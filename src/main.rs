use std::fs;
use std::io;
use std::io::prelude::*;
use std::net::Shutdown;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use clap_verbosity_flag::Verbosity;
use jsonwebtoken::EncodingKey;
use log::{debug, error, info};

use crate::token::TokenService;

mod parser;
mod token;

/// A git-credential helper that provides HTTPS credentials via Github app authentication.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[clap(flatten)]
    verbose: Verbosity,

    /// Path of the Unix auth socket
    socket_path: PathBuf,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Clone, Debug, clap::ValueEnum)]
enum CredentialOp {
    Get,
    Store,
    Erase,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Runs the Github app authentication agent
    Agent {
        /// The Github app ID
        #[arg(long)]
        app_id: u64,

        /// Path to the app's private key
        #[arg(long, value_name = "PRIVATE_KEY")]
        key_path: PathBuf,

        /// URL of the Github API
        #[arg(long, default_value = "https://api.github.com")]
        github_url: String,
    },
    /// Runs the git-credential helper client
    Client {
        /// The git-credential helper operation (only 'get' is implemented)
        #[arg(value_enum)]
        operation: CredentialOp,
    },
}

fn main() -> Result<()> {
    let args = Args::parse();
    env_logger::Builder::new()
        .filter_level(args.verbose.log_level_filter())
        .init();

    match &args.command {
        Some(Commands::Agent {
            app_id,
            key_path,
            github_url,
        }) => {
            let running = Arc::new(AtomicBool::new(true));
            let condition = Arc::new((Mutex::new(false), Condvar::new()));

            let r = running.clone();
            let c = condition.clone();
            ctrlc::set_handler(move || {
                r.store(false, Ordering::SeqCst);
                let (_, c) = &*c;
                c.notify_all();
            })
            .expect("Error setting Ctrl-C handler");

            let pem_key = fs::read_to_string(key_path)?;
            let app_key = EncodingKey::from_rsa_pem(pem_key.as_bytes())?;

            let token_service = Arc::new(token::TokenService::new(
                *app_id,
                app_key,
                github_url.to_owned(),
            )?);
            let socket_path = args.socket_path.to_owned();
            let r = running.clone();
            // Cleanup socket path on shutdown
            let agent_thread = thread::spawn(move || {
                agent(token_service, socket_path, r).unwrap();
            });

            let (mutex, stop) = &*condition;
            let mut guard = mutex.lock().or_else(|_| bail!("Mutext was poisoned"))?;
            while running.load(Ordering::SeqCst) {
                guard = stop.wait(guard).or_else(|_| bail!("Mutex was poisoned"))?;
            }
            info!("Terminating ...");

            // Wake up listener socket
            let _ = UnixStream::connect(&args.socket_path);

            if let Err(e) = agent_thread.join() {
                bail!("Failed to join agent thread: {e:?}")
            };
            fs::remove_file(&args.socket_path)?;
        }
        Some(Commands::Client { operation }) => match operation {
            CredentialOp::Get => {
                let mut read_stream = UnixStream::connect(&args.socket_path)?;
                let mut write_stream = read_stream.try_clone()?;
                let mut stdin = io::stdin();
                let input_task = thread::spawn(move || {
                    if let Err(e) = io::copy(&mut stdin, &mut write_stream) {
                        error!("Error coping input on stdin to socket: {e:?}");
                    };
                });
                let mut stdout = io::stdout();
                let output_task = thread::spawn(move || {
                    if let Err(e) = io::copy(&mut read_stream, &mut stdout) {
                        error!("Error coping output from socket to on stdout: {e:?}");
                    }
                });
                if let Err(e) = input_task.join() {
                    bail!("Failed to join input thread: {e:?}");
                };
                if let Err(e) = output_task.join() {
                    bail!("Failed to join output thread: {e:?}");
                };
            }
            _ => {
                info!("Operation '{operation:?}' not supported");
            }
        },
        None => {}
    }

    Ok(())
}

fn agent(
    token_service: Arc<TokenService>,
    socket_path: PathBuf,
    running: Arc<AtomicBool>,
) -> Result<()> {
    let listener = UnixListener::bind(&socket_path)?;
    fs::set_permissions(socket_path, fs::Permissions::from_mode(0o600))?;
    for stream in listener.incoming() {
        if !running.load(Ordering::SeqCst) {
            break;
        }
        match stream {
            Ok(stream) => {
                debug!("New auth client!");
                let service = token_service.clone();
                thread::spawn(move || {
                    if let Err(e) = handle_client(stream, &service) {
                        error!("Error handling client: {e}");
                    }
                });
            }
            Err(e) => {
                error!("Error accepting client: {e}");
            }
        }
    }
    Ok(())
}

fn handle_client(mut stream: UnixStream, service: &Arc<TokenService>) -> Result<()> {
    let repo_info = parser::parse_input(io::BufReader::new(stream.try_clone()?))?;
    info!("Got repo info: {repo_info:?}");
    let token = service.get_token(repo_info)?;

    stream.write_all(format!("username=x-access-token\npassword={token}\n").as_bytes())?;
    stream.shutdown(Shutdown::Both)?;
    Ok(())
}
