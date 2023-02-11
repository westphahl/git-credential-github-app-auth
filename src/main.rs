use std::error::Error;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::result::Result;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use jsonwebtoken::EncodingKey;
use tokio::io::AsyncWriteExt;
use tokio::net::UnixListener;
use tokio::net::UnixStream;
use tokio::{io, signal};

use crate::token::TokenService;

pub mod parser;
pub mod token;

/// A git-credential helper that provides HTTPS credentials via Github app authentication.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    match &args.command {
        Some(Commands::Agent {
            app_id,
            key_path,
            github_url,
        }) => {
            let pem_key = fs::read_to_string(key_path)?;
            let app_key = EncodingKey::from_rsa_pem(pem_key.as_bytes())?;

            let token_service = Arc::new(token::TokenService::new(
                *app_id,
                app_key,
                github_url.to_owned(),
            )?);
            let socket_path = args.socket_path.to_owned();
            // Cleanup socket path on shutdown
            tokio::spawn(async move {
                agent(token_service, socket_path).await.unwrap();
            });
            match signal::ctrl_c().await {
                Ok(()) => {
                    fs::remove_file(&args.socket_path)?;
                }
                Err(err) => {
                    eprintln!("Unable to listen for shutdown signal: {err}");
                    // we also shut down in case of error
                }
            }
        }
        Some(Commands::Client { operation }) => match operation {
            CredentialOp::Get => {
                let stream = UnixStream::connect(args.socket_path.to_owned()).await?;
                let (mut read_stream, mut write_stream) = stream.into_split();
                let mut stdin = io::stdin();
                let input_task = tokio::spawn(async move {
                    if let Err(e) = io::copy(&mut stdin, &mut write_stream).await {
                        eprintln!("Error coping input on stdin to socket: {e:?}");
                    };
                });
                let mut stdout = io::stdout();
                let output_task = tokio::spawn(async move {
                    if let Err(e) = io::copy(&mut read_stream, &mut stdout).await {
                        eprintln!("Error coping output from socket to on stdout: {e:?}");
                    }
                });
                input_task.await?;
                output_task.await?;
            }
            _ => {
                eprintln!("Operation '{operation:?}' not supported");
            }
        },
        None => {}
    }

    Ok(())
}

pub async fn agent(
    token_service: Arc<TokenService>,
    socket_path: PathBuf,
) -> Result<(), Box<dyn Error>> {
    let listener = UnixListener::bind(&socket_path)?;
    fs::set_permissions(socket_path, fs::Permissions::from_mode(0o600))?;
    loop {
        match listener.accept().await {
            Ok((mut stream, _)) => {
                eprintln!("New auth client!");
                let service = token_service.clone();
                tokio::spawn(async move {
                    let (read_stream, write_stream) = stream.split();
                    let repo_info = if let Ok(r) = parser::parse_input(read_stream).await {
                        r
                    } else {
                        eprintln!("Error parsing input");
                        return;
                    };
                    eprintln!("Got repo info: {repo_info:?}");

                    let token = match service.get_token(repo_info).await {
                        Ok(t) => t,
                        Err(e) => {
                            eprintln!("Error getting token: {e:?}");
                            return;
                        }
                    };
                    if let Err(e) = write_stream
                        .try_write(format!("username=x-access-token\npassword={token}").as_bytes())
                    {
                        eprintln!("Error writing response: {e:?}");
                    };
                    if let Err(e) = stream.shutdown().await {
                        eprintln!("Error on stream shutdown: {e:?}");
                    };
                });
            }
            Err(e) => {
                eprintln!("Error accepting client: {e}");
            }
        }
    }
}
