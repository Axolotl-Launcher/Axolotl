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
