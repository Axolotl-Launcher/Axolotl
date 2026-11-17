use crate::api::Result;
use theseus::google_ip;
use theseus::translation::{
    self, TranslationProvider, TranslationRequest, TranslationResponse,
    TranslationSettings,
};

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("translation")
        .invoke_handler(tauri::generate_handler![
            translation_get_settings,
            translation_update_settings,
            translation_test_provider,
            translation_translate,
            translation_clear_cache,
            translation_google_ip_pool_size,
        ])
        .build()
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn translation_get_settings() -> Result<TranslationSettings> {
    Ok(translation::get_settings().await?)
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn translation_update_settings(
    settings: TranslationSettings,
) -> Result<()> {
    Ok(translation::update_settings(settings).await?)
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn translation_test_provider(
    provider: TranslationProvider,
) -> Result<String> {
    Ok(translation::test_provider(provider).await?)
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn translation_translate(
    request: TranslationRequest,
) -> Result<TranslationResponse> {
    Ok(translation::translate(request).await?)
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn translation_clear_cache() -> Result<()> {
    Ok(translation::clear_cache().await?)
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn translation_google_ip_pool_size() -> usize {
    google_ip::ip_pool_size().await
}
