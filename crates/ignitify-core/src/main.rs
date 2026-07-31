mod error;

use std::{env, sync::Arc};

use ignitify_auth::{AuthConfig, AuthService};
use ignitify_db::{Database, DatabaseConfig};
use tokio::net::TcpListener;

use crate::error::{CoreError, Result};

fn trusted_origins() -> Arc<[String]> {
    env_value("IGNITIFY_TRUSTED_ORIGINS")
        .map(|origins| {
            origins
                .split(',')
                .map(str::trim)
                .filter(|origin| !origin.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_else(|| AuthConfig::default().trusted_origins)
        .into()
}

fn env_value(name: &str) -> Option<String> {
    env::var(name).ok().or_else(|| match name {
        "IGNITIFY_DATABASE_URL" => option_env!("IGNITIFY_DATABASE_URL").map(str::to_owned),
        "IGNITIFY_JWT_SECRET" => option_env!("IGNITIFY_JWT_SECRET").map(str::to_owned),
        "IGNITIFY_SECURE_COOKIES" => option_env!("IGNITIFY_SECURE_COOKIES").map(str::to_owned),
        "IGNITIFY_TRUSTED_ORIGINS" => option_env!("IGNITIFY_TRUSTED_ORIGINS").map(str::to_owned),
        _ => None,
    })
}

fn required_env(name: &'static str) -> Result<String> {
    env_value(name).ok_or(CoreError::MissingEnvironment(name))
}

#[tokio::main]
async fn main() -> Result<()> {
    let database = Database::connect(&DatabaseConfig {
        url: env_value("IGNITIFY_DATABASE_URL").unwrap_or_else(|| DatabaseConfig::default().url),
    })
    .await?;
    database.ping().await?;

    let auth = AuthService::new(
        database.clone(),
        AuthConfig {
            jwt_secret: required_env("IGNITIFY_JWT_SECRET")?,
            ..AuthConfig::default()
        },
    )
    .shared();
    let app = ignitify_api::router(
        auth,
        database,
        env_value("IGNITIFY_SECURE_COOKIES").is_some_and(|value| value == "true"),
        trusted_origins(),
    );
    let listener = TcpListener::bind("127.0.0.1:5656").await?;

    println!("Ignitify API listening on {}", listener.local_addr()?);
    axum::serve(listener, app).await.map_err(CoreError::Io)
}
