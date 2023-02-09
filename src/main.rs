use clap::{Parser, Subcommand};
use jsonwebtoken::EncodingKey;
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::result::Result;
use tokio::net::UnixStream;
use tokio::{io, signal};

pub mod agent;
pub mod parser;
pub mod token;

/// Simple program to greet a person
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
    /// Runs the Github app authentication deamon
    Agent {
        /// lists test values
        #[arg(long)]
        app_id: u64,

        /// Sets a custom config file
        #[arg(long, value_name = "FILE")]
        key_path: PathBuf,

        /// URL of the Github API
        #[arg(long, default_value = "https://api.github.com")]
        github_url: String,
    },
    Client {
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
            let token_service = token::TokenService::new(*app_id, app_key, github_url.to_owned())?;
            let agent = agent::AuthAgent::new(token_service);
            // agent.listen(args.socket_path.to_owned()).await?;

            let socket_path = args.socket_path.to_owned();
            // Cleanup socket path on shutdown
            tokio::spawn(async move {
                agent.listen(socket_path).await.unwrap();
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
                    io::copy(&mut stdin, &mut write_stream).await.unwrap();
                });
                let mut stdout = io::stdout();
                let output_task = tokio::spawn(async move {
                    io::copy(&mut read_stream, &mut stdout).await.unwrap();
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
