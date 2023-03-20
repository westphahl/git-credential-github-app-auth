use std::collections::HashMap;

use std::sync::RwLock;
use std::time::SystemTime;

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::EncodingKey;
use log::info;
use serde::{Deserialize, Serialize};

use crate::parser::RepoInfo;

#[derive(Clone, Deserialize, Debug)]
pub struct InstallationToken {
    pub token: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone, Deserialize, Debug)]
pub struct RepositoryInstallation {
    pub id: u64,
}

struct GithubClient {
    github_url: String,
    app_id: u64,
    app_key: EncodingKey,
}

impl GithubClient {
    pub fn get_repository_installation(&self, repo_info: &RepoInfo) -> Result<u64> {
        let response: RepositoryInstallation = ureq::get(&format!(
            "{}/repos/{}/{}/installation",
            self.github_url, repo_info.organization, repo_info.name
        ))
        .set("Accept", "application/vnd.github+json")
        .set("Authorization", &format!("Bearer {}", self.create_jwt()?))
        .set("X-GitHub-Api-Version", "2022-11-28")
        .call()?
        .into_json()?;
        Ok(response.id)
    }

    pub fn get_access_token(&self, installation_id: u64) -> Result<InstallationToken> {
        let response: InstallationToken = ureq::post(&format!(
            "{}/app/installations/{installation_id}/access_tokens",
            self.github_url
        ))
        .set("Accept", "application/vnd.github+json")
        .set("Authorization", &format!("Bearer {}", self.create_jwt()?))
        .set("X-GitHub-Api-Version", "2022-11-28")
        .call()?
        .into_json()?;
        Ok(response)
    }

    /// Note: copied from octocrab
    pub fn create_jwt(&self) -> Result<String> {
        #[derive(Serialize)]
        struct Claims {
            iat: usize,
            exp: usize,
            iss: String,
        }

        let header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
        let now = SystemTime::UNIX_EPOCH.elapsed().unwrap().as_secs() as usize;
        let claims = Claims {
            iss: self.app_id.to_string(),
            iat: now - 60,
            exp: now + (9 * 60),
        };

        Ok(jsonwebtoken::encode(&header, &claims, &self.app_key)?)
    }
}

pub struct TokenService {
    github_client: GithubClient,
    installations: RwLock<HashMap<RepoInfo, u64>>,
    cache: RwLock<HashMap<u64, InstallationToken>>,
}

impl TokenService {
    pub fn new(app_id: u64, app_key: EncodingKey, github_url: String) -> Result<TokenService> {
        let client = GithubClient {
            github_url,
            app_id,
            app_key,
        };
        Ok(TokenService {
            github_client: client,
            installations: RwLock::new(HashMap::new()),
            cache: RwLock::new(HashMap::new()),
        })
    }

    pub fn get_token(&self, repo_info: RepoInfo) -> Result<String> {
        let installation_id = {
            let installations = self.installations.read().unwrap();
            if let Some(installation_id) = installations.get(&repo_info) {
                let tokens = self.cache.read().unwrap();
                if let Some(install_token) = tokens.get(installation_id) {
                    if install_token.expires_at > Utc::now() - Duration::minutes(5) {
                        return Ok(install_token.token.clone());
                    }
                }
                *installation_id
            } else {
                self.github_client.get_repository_installation(&repo_info)?
            }
        };
        // drop(installations);
        info!("Installation ID: {installation_id}");

        let installation_token = self.github_client.get_access_token(installation_id)?;

        let mut debug_token = installation_token.token.clone();
        debug_token.truncate(10);
        info!(
            "Installation token: {}... (expires: {})",
            debug_token, installation_token.expires_at
        );

        {
            let mut installations = self.installations.write().unwrap();
            installations.insert(repo_info, installation_id);
        }
        {
            let mut tokens = self.cache.write().unwrap();
            tokens.insert(installation_id, installation_token.clone());
        }
        Ok(installation_token.token)
    }
}
