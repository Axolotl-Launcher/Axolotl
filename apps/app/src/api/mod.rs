use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};
use thiserror::Error;

pub mod ai;
pub mod auth;
pub mod import;
pub mod install;
pub mod instance;
pub mod jre;
pub mod logs;
pub mod mcarchive;
pub mod metadata;
pub mod minecraft_skins;
pub mod mod_translation;
pub mod mr_auth;
pub mod multiplayer;
pub mod planet_minecraft;
pub mod process;
pub mod schematic_preview;
pub mod seed_map;
pub mod servers;
pub mod settings;
pub mod shortcuts;
pub mod storage;
pub mod system_accent;
pub mod tags;
pub mod telemetry;
pub mod terracotta;
pub mod translation;
pub mod utils;

pub mod cache;
pub mod content_favorites;
pub mod content_search;
pub mod curseforge;
pub mod datapacks;
pub mod drop;
pub mod files;
pub mod fonts;
pub mod friends;
pub mod resources;
pub mod worlds;

mod oauth_utils;
mod search_cancellation;

pub type Result<T> = std::result::Result<T, TheseusSerializableError>;

// // Main returnable Theseus GUI error
// // Needs to be Serializable to be returned to the JavaScript side
// #[derive(Error, Debug, Serialize)]
// pub enum TheseusGuiError {
//     #[error(transparent)]
//     Serializable(),
// }

// Serializable error intermediary, so TheseusGuiError can be Serializable (eg: so that we can return theseus::Errors in Tauri directly)
#[derive(Error, Debug)]
pub enum TheseusSerializableError {
    #[error("{0}")]
    Theseus(#[from] theseus::Error),

    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),

    #[error("Tauri error: {0}")]
    Tauri(#[from] tauri::Error),

    #[cfg(feature = "updater")]
    #[error("Updater error: {0}")]
    Updater(#[from] tauri_plugin_updater::Error),

    #[cfg(feature = "updater")]
    #[error("HTTP error: {0}")]
    Http(#[from] tauri_plugin_http::reqwest::Error),

    #[error("Search request cancelled: {0}")]
    SearchCancelled(String),

    #[error("{0}")]
    Resource(#[from] theseus::resources::ResourceError),
}

// Generic implementation of From<T> for ErrorTypeA
// impl<T> From<T> for TheseusGuiError
// where
//     TheseusSerializableError: From<T>,
// {
//     fn from(error: T) -> Self {
//         TheseusGuiError::Serializable(TheseusSerializableError::from(error))
//     }
// }

// This is a very simple macro that implements a very basic Serializable for each variant of TheseusSerializableError,
// where the field is the string. (This allows easy extension to errors without many match arms)
macro_rules! impl_serialize {
    ($($variant:ident),* $(,)?) => {
        impl Serialize for TheseusSerializableError {
            fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                match self {
                    // For the Theseus variant, we add a special display for the error,
                    // to view the spans if subscribed to them (which is information that is lost when serializing)
                    TheseusSerializableError::Theseus(theseus_error) => {
                        $crate::error::display_tracing_error(theseus_error);

                        let mut state = serializer.serialize_struct("Theseus", 2)?;
                        state.serialize_field("field_name", "Theseus")?;
                        state.serialize_field(
                            "message",
                            &theseus_error.user_facing_message(),
                        )?;
                        state.end()
                    }
                    TheseusSerializableError::Resource(error) => {
                        if !matches!(error, theseus::resources::ResourceError::Canceled) {
                            tracing::error!(error = %error, "Resource operation failed");
                        }
                        let mut state = serializer.serialize_struct("Resource", 3)?;
                        state.serialize_field("field_name", "Resource")?;
                        state.serialize_field("message", &error.to_string())?;
                        state.serialize_field("code", if matches!(error, theseus::resources::ResourceError::CursorExpired) { "cursorExpired" } else { "resourceError" })?;
                        state.end()
                    }
                    $(
                        TheseusSerializableError::$variant(message) => {
                            let mut state = serializer.serialize_struct(stringify!($variant), 2)?;
                            state.serialize_field("field_name", stringify!($variant))?;
                            state.serialize_field("message", &message.to_string())?;
                            state.end()
                        },
                    )*
                }
            }
        }
    };
}

// Use the macro to implement Serialize for TheseusSerializableError
#[cfg(not(feature = "updater"))]
impl_serialize! {
    IO,
    Tauri,
    SearchCancelled,
}

#[cfg(feature = "updater")]
impl_serialize! {
    IO,
    Tauri,
    Updater,
    Http,
    SearchCancelled,
}
