use futures_util::stream::StreamExt;
use octocrab::Octocrab;
use secrecy::ExposeSecret;
use std::error::Error;
use std::fs;
use std::result::Result;
use tokio::io;
use tokio_util::codec::{FramedRead, LinesCodec};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let app_id = read_env_var("GITHUB_APP_ID").parse::<u64>().unwrap().into();
    let app_private_key = fs::read_to_string(read_env_var("GITHUB_APP_PRIVATE_KEY"))?;
    // TODO: check permissions of private key
    let github_url =
        std::env::var("GITHUB_URL").unwrap_or_else(|_| "https://api.github.com".to_string());
    let key = jsonwebtoken::EncodingKey::from_rsa_pem(app_private_key.as_bytes()).unwrap();

    let mut repo_path = String::new();
    let mut protocol = String::new();
    let mut reader = FramedRead::new(io::stdin(), LinesCodec::new());
    loop {
        let line = match reader.next().await {
            Some(l) => l?,
            None => break,
        };
        // eprintln!("Got line {:?}", line);
        if line.is_empty() {
            break;
        }
        let (attr, value) = line
            .split_once('=')
            .ok_or("Invalid input format (must be 'attr=value')")?;
        match attr {
            "path" => repo_path.push_str(value),
            "protocol" => protocol.push_str(value),
            _ => {}
        }
        // eprintln!("Got attribute {:?} => {:?}", attr, value);
    }

    if protocol != "https" {
        Err("Cannot handle non-https protocols")?;
    }

    if repo_path.is_empty() {
        Err("No repo path provided")?;
    }

    let (organization, repo_path) = repo_path
        .split_once('/')
        .ok_or("Failed to extract organization and repo name (expected format 'org/repo')")?;
    // Remove .git suffix that can be part of the clone URL
    let repository = repo_path.strip_suffix(".git").unwrap_or(repo_path);

    let octocrab = Octocrab::builder()
        .base_url(github_url)?
        .app(app_id, key)
        .build()?;
    let installation = octocrab
        .apps()
        .get_repository_installation(organization, repository)
        .await?;

    // eprintln!("Installation: {:?}", installation);

    let (_, secret_token) = octocrab.installation_and_token(installation.id).await?;
    let token = secret_token.expose_secret();

    // eprintln!("Token {}", token);
    println!("username=x-access-token");
    println!("password={}", token);

    Ok(())
}

fn read_env_var(var_name: &str) -> String {
    let err = format!("Missing environment variable: {}", var_name);
    std::env::var(var_name).expect(&err)
}
