//! User media organization is independent of SQL; picture bodies stay files.

pub(crate) mod manifest;
pub(crate) mod skins;
#[cfg(test)]
mod tests;

use super::persistence::{
    Persistence, document::Document, status::Availability,
};
use serde_json::{Map, Value, json};

#[derive(Clone)]
pub(crate) struct Media {
    pub(super) persistence: Persistence,
    pub(super) document: Document,
}

impl Media {
    pub(crate) fn new(persistence: &Persistence) -> Self {
        Self {
            persistence: persistence.clone(),
            document: persistence
                .document("state/media.json", "axolotl-media", 1)
                .expect("constant media document path"),
        }
    }

    pub(crate) async fn read(&self) -> crate::Result<Map<String, Value>> {
        let result = self.document.read().await?;
        if !matches!(
            result.availability,
            Availability::Ready | Availability::Recovered
        ) {
            return Err(crate::ErrorKind::FSError(
                "Media organization is unavailable or uses a newer protocol"
                    .into(),
            )
            .into());
        }
        let data = result
            .snapshot
            .expect("readable media document")
            .data()
            .expect("media payload")
            .clone();
        validate(&data)?;
        Ok(data)
    }

    pub(super) async fn update<F>(&self, change: F) -> crate::Result<()>
    where
        F: FnOnce(&mut Map<String, Value>) -> crate::Result<()>
            + Send
            + 'static,
    {
        self.document
            .update(move |data| {
                validate(data)?;
                change(data)
            })
            .await?;
        Ok(())
    }
}

pub(crate) fn empty() -> Map<String, Value> {
    Map::from_iter(
        [
            "screenshots",
            "groups",
            "saved_skins",
            "textures",
            "offline_skins",
        ]
        .into_iter()
        .map(|key| (key.into(), json!({}))),
    )
}

pub(crate) fn validate(data: &Map<String, Value>) -> crate::Result<()> {
    if [
        "screenshots",
        "groups",
        "saved_skins",
        "textures",
        "offline_skins",
    ]
    .iter()
    .any(|key| !data.get(*key).is_some_and(Value::is_object))
    {
        return Err(crate::ErrorKind::InputError(
            "Media manifest has an unreadable collection".into(),
        )
        .into());
    }
    Ok(())
}

pub(super) fn collection<'a>(
    data: &'a mut Map<String, Value>,
    name: &str,
) -> &'a mut Map<String, Value> {
    data.get_mut(name)
        .and_then(Value::as_object_mut)
        .expect("validated media collection")
}

pub(crate) fn supported(value: &Value) -> bool {
    value.is_object()
        && value.get("protocol").is_none()
        && ["schema_version", "schemaVersion"].into_iter().all(|key| {
            value
                .get(key)
                .is_none_or(|version| version.as_u64() == Some(1))
        })
}
