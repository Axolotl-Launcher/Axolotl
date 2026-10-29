//! Persistence for the user-configurable proxy settings.

use sqlx::SqlitePool;

use crate::state::store::settings::Settings;
use crate::state::store::{Handle, atom};
use crate::util::proxy::{ProxyConfig, ProxyMode};

pub async fn get(pool: &SqlitePool) -> crate::Result<ProxyConfig> {
    let columns = Settings::owned_columns(&Handle::new(pool.clone())).await?;

    Ok(ProxyConfig {
        mode: ProxyMode::from_string(&columns.proxy_mode),
        url: columns.proxy_url,
        username: columns.proxy_username,
        password: columns.proxy_password,
    })
}

pub async fn set(pool: &SqlitePool, config: &ProxyConfig) -> crate::Result<()> {
    config.validate()?;

    let handle = Handle::new(pool.clone());
    let mut unit = atom(&handle).await?;
    Settings::update_owned_in(&mut unit, |columns| {
        columns.proxy_mode = config.mode.as_str().to_string();
        columns.proxy_url = config.url.trim().to_string();
        columns.proxy_username = config.username.trim().to_string();
        columns.proxy_password = config.password.clone();
    })
    .await?;

    unit.commit().await
}
