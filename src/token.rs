use crate::parser::RepoInfo;
use jsonwebtoken::EncodingKey;
use octocrab::models::InstallationId;
// use octocrab::{from_response, Octocrab};
use octocrab::Octocrab;
// use reqwest;
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use std::collections::HashMap;
use std::error::Error;
use tokio::sync::RwLock;

#[derive(Clone, Deserialize, Debug)]
pub struct InstallationToken {
    pub token: String,
    pub expires_at: DateTime<Utc>,
}

pub struct TokenService {
    github_client: Octocrab,
    installations: RwLock<HashMap<RepoInfo, InstallationId>>,
    cache: RwLock<HashMap<InstallationId, InstallationToken>>,
}

impl TokenService {
    pub fn new(
        app_id: u64,
        app_key: EncodingKey,
        github_url: String,
    ) -> Result<TokenService, Box<dyn Error>> {
        let octocrab = Octocrab::builder()
            .base_url(github_url)?
            .app(app_id.into(), app_key)
            .build()?;
        Ok(TokenService {
            github_client: octocrab,
            installations: RwLock::new(HashMap::new()),
            cache: RwLock::new(HashMap::new()),
        })
    }

    pub async fn get_token(&self, repo_info: RepoInfo) -> Result<String, Box<dyn Error>> {
        let installations = self.installations.read().await;
        let installation_id = if let Some(installation_id) = installations.get(&repo_info) {
            let tokens = self.cache.read().await;
            if let Some(install_token) = tokens.get(installation_id) {
                if install_token.expires_at > Utc::now() - Duration::minutes(5) {
                    return Ok(install_token.token.clone());
                }
            }
            *installation_id
        } else {
            let installation = self
                .github_client
                .apps()
                .get_repository_installation(repo_info.organization.clone(), repo_info.name.clone())
                .await?;
            installation.id
        };
        drop(installations);
        eprintln!("Installation ID: {installation_id}");

        let url = self
            .github_client
            .absolute_url(format!("app/installations/{installation_id}/access_tokens"))?;
        let request_builder = self
            .github_client
            .request_builder(url, reqwest::Method::POST);
        let response = self.github_client.execute(request_builder).await?;
        let installation_token: InstallationToken = serde_json::from_str(&response.text().await?)?;

        let mut debug_token = installation_token.token.clone();
        debug_token.truncate(10);
        eprintln!(
            "Installation token: {}... (expires: {})",
            debug_token, installation_token.expires_at
        );

        {
            let mut installations = self.installations.write().await;
            installations.insert(repo_info, installation_id);
        }
        {
            let mut tokens = self.cache.write().await;
            tokens.insert(installation_id, installation_token.clone());
        }
        Ok(installation_token.token)
    }
}
