use std::cmp::{Eq, PartialEq};
use std::hash::Hash;
use std::io::BufRead;

use anyhow::{anyhow, bail, Result};
use log::trace;

#[derive(Debug, Eq, Hash, PartialEq)]
pub struct RepoInfo {
    pub organization: String,
    pub name: String,
}

pub fn parse_input<T>(stream: T) -> Result<RepoInfo>
where
    T: BufRead + Unpin,
{
    let mut repo_path = String::new();
    let mut protocol = String::new();
    for input in stream.lines() {
        trace!("Got input: {input:?}");
        let line = input?;
        trace!("Got line {:?}", line);
        if line.is_empty() {
            break;
        }
        let (attr, value) = line
            .split_once('=')
            .ok_or(anyhow!("Invalid input format (must be 'attr=value')"))?;
        match attr {
            "path" => repo_path.push_str(value),
            "protocol" => protocol.push_str(value),
            _ => {}
        }
        trace!("Got attribute {attr:?} => {value:?}");
        if !(repo_path.is_empty() || protocol.is_empty()) {
            // Got all necessary information
            break;
        }
    }

    if protocol != "https" {
        bail!("Cannot handle non-https protocols");
    }

    if repo_path.is_empty() {
        bail!("No repo path provided");
    }

    let (organization, repo_path) = repo_path.split_once('/').ok_or(anyhow!(
        "Failed to extract organization and repo name (expected format 'org/repo')"
    ))?;
    // Remove .git suffix that can be part of the clone URL
    let repository = repo_path.strip_suffix(".git").unwrap_or(repo_path);

    Ok(RepoInfo {
        organization: organization.to_owned(),
        name: repository.to_owned(),
    })
}
