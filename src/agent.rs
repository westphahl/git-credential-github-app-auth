use crate::parser;
use crate::token::TokenService;
use std::error::Error;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::UnixListener;

pub struct AuthAgent {
    token_service: Arc<TokenService>,
}

impl AuthAgent {
    pub fn new(service: TokenService) -> AuthAgent {
        AuthAgent {
            token_service: Arc::new(service),
        }
    }

    pub async fn listen(&self, socket_path: PathBuf) -> Result<(), Box<dyn Error>> {
        let listener = UnixListener::bind(&socket_path)?;
        fs::set_permissions(socket_path, fs::Permissions::from_mode(0o600))?;
        loop {
            match listener.accept().await {
                Ok((mut stream, _)) => {
                    eprintln!("New auth client!");
                    let token_service = self.token_service.clone();
                    tokio::spawn(async move {
                        let (read_stream, write_stream) = stream.split();
                        let repo_info = parser::parse_input(read_stream).await.unwrap();
                        eprintln!("Got repo info: {repo_info:?}");

                        let token = token_service.get_token(repo_info).await.unwrap();
                        write_stream
                            .try_write(
                                format!("username=x-access-token\npassword={token}").as_bytes(),
                            )
                            .unwrap();
                        stream.shutdown().await.unwrap();
                    });
                }
                Err(e) => {
                    eprintln!("Error accepting client: {e}");
                }
            }
        }
    }
}
