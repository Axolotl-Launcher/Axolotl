use serde::{Deserialize, Serialize};

use crate::State;
use crate::state::documents;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CrashAnalysisAiSettings {
    pub enabled: bool,
    pub provider_id: String,
    pub model_id: String,
    pub ai_source: String,
}

pub async fn get_crash_analysis_ai_settings()
-> crate::Result<CrashAnalysisAiSettings> {
    let state = State::get().await?;
    let document =
        documents::log_share::read(&documents::Documents::new(&state)).await?;
    Ok(CrashAnalysisAiSettings {
        enabled: document.crash_analysis.enabled,
        provider_id: document.crash_analysis.provider_id,
        model_id: document.crash_analysis.model_id,
        ai_source: document.ai_source,
    })
}

pub async fn update_crash_analysis_ai_settings(
    settings: CrashAnalysisAiSettings,
) -> crate::Result<()> {
    let state = State::get().await?;
    documents::log_share::update_crash_analysis(
        &documents::Documents::new(&state),
        documents::log_share::CrashAnalysisSettings {
            enabled: settings.enabled,
            provider_id: settings.provider_id.trim().to_string(),
            model_id: settings.model_id.trim().to_string(),
            ai_source: normalize_ai_source(&settings.ai_source).to_string(),
        },
    )
    .await
}

fn normalize_ai_source(ai_source: &str) -> &str {
    if ai_source == "custom" {
        "custom"
    } else {
        "logshare"
    }
}
