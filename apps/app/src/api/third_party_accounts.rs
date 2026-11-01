use crate::api::Result;
use theseus::third_party_accounts::{self, AccountSummary, AuthorizationStart};

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("third-party-accounts")
        .invoke_handler(tauri::generate_handler![
            get_account,
            begin_authorization,
            cancel_authorization,
            disconnect,
            check_connection,
            shimu_list_schematics,
            shimu_get_schematic,
            shimu_install_schematic,
        ])
        .build()
}

#[tauri::command]
pub fn get_account() -> Result<AccountSummary> {
    Ok(third_party_accounts::get_account()?)
}

#[tauri::command]
pub fn begin_authorization() -> Result<AuthorizationStart> {
    Ok(third_party_accounts::begin_authorization()?)
}

#[tauri::command]
pub fn cancel_authorization(flow_id: String) -> Result<()> {
    Ok(third_party_accounts::cancel_authorization(&flow_id)?)
}

#[tauri::command]
pub fn disconnect() -> Result<()> {
    Ok(third_party_accounts::disconnect()?)
}

#[tauri::command]
pub async fn check_connection() -> Result<AccountSummary> {
    Ok(third_party_accounts::check_connection().await?)
}

#[tauri::command]
pub async fn shimu_list_schematics(
    page: u32,
    query: String,
    page_size: u32,
    sort: String,
) -> Result<theseus::third_party_accounts::shimu::SchematicPage> {
    Ok(
        theseus::third_party_accounts::shimu::list(
            page, query, page_size, sort,
        )
        .await?,
    )
}

#[tauri::command]
pub async fn shimu_get_schematic(
    post_id: u64,
) -> Result<theseus::third_party_accounts::shimu::SchematicDetail> {
    Ok(theseus::third_party_accounts::shimu::detail(post_id).await?)
}

#[tauri::command]
pub async fn shimu_install_schematic(
    instance_id: String,
    post_id: u64,
    file_name: String,
) -> Result<String> {
    Ok(theseus::third_party_accounts::shimu::install(
        instance_id,
        post_id,
        file_name,
    )
    .await?)
}
