use std::cmp::{Eq, PartialEq};
use std::error::Error;
use std::hash::Hash;

use futures_util::stream::StreamExt;
use tokio::io::AsyncRead;
use tokio_util::codec::{FramedRead, LinesCodec};

#[derive(Debug, Eq, Hash, PartialEq)]
pub struct RepoInfo {
    pub organization: String,
    pub name: String,
}

pub async fn parse_input<T>(stream: T) -> Result<RepoInfo, Box<dyn Error>>
where
    T: AsyncRead + Unpin,
{
    let mut repo_path = String::new();
    let mut protocol = String::new();
    let mut reader = FramedRead::new(stream, LinesCodec::new());
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
        eprintln!("Got attribute {attr:?} => {value:?}");
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

    Ok(RepoInfo {
        organization: organization.to_owned(),
        name: repository.to_owned(),
    })
}
