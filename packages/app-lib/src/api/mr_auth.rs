use crate::state::ModrinthCredentials;

#[tracing::instrument]
pub fn authenticate_begin_flow() -> &'static str {
    crate::state::get_login_url()
}

#[tracing::instrument]
pub async fn authenticate_finish_flow(
    code: &str,
) -> crate::Result<ModrinthCredentials> {
    let state = crate::State::get().await?;

    let creds =
        crate::state::finish_login_flow(code, &state.api_semaphore).await?;

    creds.upsert(&state.data).await?;
    Ok(creds)
}

#[tracing::instrument]
pub async fn logout() -> crate::Result<()> {
    let state = crate::State::get().await?;
    let current = ModrinthCredentials::get_active(&state.data).await?;

    if let Some(current) = current {
        ModrinthCredentials::remove(&current.user_id, &state.data).await?;
    }

    Ok(())
}

#[tracing::instrument]
pub async fn get_credentials() -> crate::Result<Option<ModrinthCredentials>> {
    let state = crate::State::get().await?;
    let current =
        ModrinthCredentials::get_and_refresh(&state.data, &state.api_semaphore)
            .await?;

    Ok(current)
}
