use octocrab::Octocrab;
use secrecy::ExposeSecret;

#[tokio::main]
async fn main() -> octocrab::Result<()> {
    let app_id = read_env_var("GITHUB_APP_ID").parse::<u64>().unwrap().into();
    let app_private_key = read_env_var("GITHUB_APP_PRIVATE_KEY");
    let key = jsonwebtoken::EncodingKey::from_rsa_pem(app_private_key.as_bytes()).unwrap();

    // TODO: make this env vars or cli params
    let organization = "westphahl";
    let repository = "git-credential-helper-github-app-auth";

    let octocrab = Octocrab::builder().app(app_id, key).build()?;

    let installation = octocrab
        .apps()
        .get_repository_installation(organization, repository)
        .await?;

    println!("Installation: {:?}", installation);

    let (_, secret_token) = octocrab.installation_and_token(installation.id).await?;
    let token = secret_token.expose_secret();

    println!("Token {}", token);

    Ok(())
}

fn read_env_var(var_name: &str) -> String {
    let err = format!("Missing environment variable: {}", var_name);
    std::env::var(var_name).expect(&err)
}
