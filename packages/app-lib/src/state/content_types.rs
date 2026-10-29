//! The provider and content vocabulary a favorite is described in.
//!
//! It lives on its own so both the logic that uses a favorite and the
//! collection that stores one can depend on it, without either depending on
//! the other.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentFavoriteProvider {
    Modrinth,
    Curseforge,
    Mcarchive,
}

impl ContentFavoriteProvider {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Modrinth => "modrinth",
            Self::Curseforge => "curseforge",
            Self::Mcarchive => "mcarchive",
        }
    }

    pub(crate) fn parse(value: &str) -> crate::Result<Self> {
        match value {
            "modrinth" => Ok(Self::Modrinth),
            "curseforge" => Ok(Self::Curseforge),
            "mcarchive" => Ok(Self::Mcarchive),
            _ => Err(crate::ErrorKind::InputError(format!(
                "Unsupported content favorite provider: {value}"
            ))
            .into()),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentFavoriteType {
    Mod,
    Resourcepack,
    Datapack,
    Shader,
}

impl ContentFavoriteType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mod => "mod",
            Self::Resourcepack => "resourcepack",
            Self::Datapack => "datapack",
            Self::Shader => "shader",
        }
    }

    pub(crate) fn parse(value: &str) -> crate::Result<Self> {
        match value {
            "mod" => Ok(Self::Mod),
            "resourcepack" => Ok(Self::Resourcepack),
            "datapack" => Ok(Self::Datapack),
            "shader" => Ok(Self::Shader),
            _ => Err(crate::ErrorKind::InputError(format!(
                "Unsupported content favorite type: {value}"
            ))
            .into()),
        }
    }
}
