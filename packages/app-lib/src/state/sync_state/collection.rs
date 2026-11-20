//! Typed collection operations participate in the explicit file transaction.
use super::manifest::Collections;
use super::{key, supported};
use crate::state::store::{Atom, Handle, atom};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Map, Value, json};
pub(crate) trait Collection {
    type Key: PartialEq + Clone + Serialize + DeserializeOwned;
    type Scope;
    type Record: Clone + Serialize + DeserializeOwned;
    const NAME: &'static str;
    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool;
    fn record_supported(raw: &Value) -> bool {
        !raw.is_null()
            && raw.get("protocol").is_none()
            && raw
                .get("schemaVersion")
                .is_none_or(|value| value.as_u64() == Some(1))
            && raw
                .get("schema_version")
                .is_none_or(|value| value.as_u64() == Some(1))
    }
    fn ordered(rows: &mut Vec<(Self::Key, Self::Record)>) {
        let _ = rows;
    }
    async fn get(
        handle: &Handle,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let collections =
            crate::state::store::sync_documents::read_collections(handle)
                .await?;
        Self::read(&collections, key)
    }
    async fn get_all(
        handle: &Handle,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let collections =
            crate::state::store::sync_documents::read_collections(handle)
                .await?;
        Self::read_all(&collections)
    }
    async fn get_scoped(
        handle: &Handle,
        scope: &Self::Scope,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let collections =
            crate::state::store::sync_documents::read_collections(handle)
                .await?;
        Self::read_scoped(&collections, scope)
    }
    async fn set(
        handle: &Handle,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let mut unit = atom(handle).await?;
        Self::atom_set(&mut unit, key, record).await?;
        unit.commit().await
    }
    async fn set_all(
        handle: &Handle,
        rows: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        let mut unit = atom(handle).await?;
        Self::atom_set_all(&mut unit, rows).await?;
        unit.commit().await
    }
    async fn set_scoped(
        handle: &Handle,
        scope: &Self::Scope,
        rows: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        let mut unit = atom(handle).await?;
        Self::atom_set_scoped(&mut unit, scope, rows).await?;
        unit.commit().await
    }
    async fn atom_get(
        unit: &mut Atom,
        wanted: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        Self::read(&unit.sync().await?.collections, wanted)
    }
    fn read(
        collections: &Collections,
        wanted: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let id = key(&serde_json::to_value(wanted)?)?;
        let Some(raw) =
            collections.get(Self::NAME).and_then(|rows| rows.get(&id))
        else {
            return Ok(None);
        };
        if !supported(raw) || !Self::record_supported(&raw["record"]) {
            return Err(crate::ErrorKind::InputError(
                "Sync record uses newer protocol; retained".into(),
            )
            .into());
        }
        let stored: Self::Key = serde_json::from_value(raw["key"].clone())?;
        if stored != *wanted {
            return Err(crate::ErrorKind::InputError(
                "Sync identity mismatch".into(),
            )
            .into());
        }
        Ok(Some(serde_json::from_value(raw["record"].clone())?))
    }
    async fn atom_get_all(
        unit: &mut Atom,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        Self::read_all(&unit.sync().await?.collections)
    }
    fn read_all(
        collections: &Collections,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let mut result = Vec::new();
        if let Some(rows) = collections.get(Self::NAME) {
            for (identity, raw) in rows {
                if !supported(raw) || !Self::record_supported(&raw["record"]) {
                    continue;
                }
                if key(&raw["key"])? != *identity {
                    tracing::warn!(
                        collection = Self::NAME,
                        "Sync record identity mismatch retained"
                    );
                    continue;
                }
                match (
                    serde_json::from_value(raw["key"].clone()),
                    serde_json::from_value(raw["record"].clone()),
                ) {
                    (Ok(key), Ok(record)) => result.push((key, record)),
                    _ => tracing::warn!(
                        collection = Self::NAME,
                        "Unreadable sync record retained"
                    ),
                }
            }
        }
        Self::ordered(&mut result);
        Ok(result)
    }
    async fn atom_get_scoped(
        unit: &mut Atom,
        scope: &Self::Scope,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        Self::read_scoped(&unit.sync().await?.collections, scope)
    }
    fn read_scoped(
        collections: &Collections,
        scope: &Self::Scope,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        Ok(Self::read_all(collections)?
            .into_iter()
            .filter(|(key, _)| Self::in_scope(scope, key))
            .collect())
    }
    async fn atom_set(
        unit: &mut Atom,
        wanted: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let encoded_key = serde_json::to_value(wanted)?;
        if record.is_some() {
            unit.validate_sync_parents(Self::NAME, &encoded_key).await?;
        }
        let id = key(&encoded_key)?;
        let transaction = unit.sync().await?;
        if transaction.head.legacy_extensions_present {
            return Err(crate::ErrorKind::InputError("Opaque legacy sync state prevents mutation; original records retained".into()).into());
        }
        if record.is_none()
            && matches!(Self::NAME, "synced_servers" | "synced_pack_catalog")
        {
            let dependent = if Self::NAME == "synced_servers" {
                "instance_servers"
            } else {
                "synced_pack_instances"
            };
            if transaction.collections.get(dependent).is_some_and(|rows| {
                rows.values()
                    .any(|raw| !supported(raw) || !supported(&raw["record"]))
            }) {
                return Err(crate::ErrorKind::InputError(
                    "Unknown dependent sync record prevents removal".into(),
                )
                .into());
            }
        }
        let rows = transaction
            .collections
            .entry(Self::NAME.into())
            .or_default();
        if let Some(raw) = rows.get(&id) {
            if !supported(raw) || !Self::record_supported(&raw["record"]) {
                return Err(crate::ErrorKind::InputError(
                    "Sync record uses newer protocol; retained".into(),
                )
                .into());
            }
            let _: Self::Record =
                serde_json::from_value(raw["record"].clone())?;
            let stored: Self::Key = serde_json::from_value(raw["key"].clone())?;
            if stored != *wanted {
                return Err(crate::ErrorKind::InputError(
                    "Sync identity mismatch retained".into(),
                )
                .into());
            }
        }
        if let Some(record) = record {
            let raw = rows.entry(id).or_insert_with(|| json!({}));
            raw["key"] = serde_json::to_value(wanted)?;
            let desired = serde_json::to_value(record)?;
            if let (Some(previous), Some(desired)) = (
                raw.get_mut("record").and_then(Value::as_object_mut),
                desired.as_object(),
            ) {
                for (key, value) in desired {
                    previous.insert(key.clone(), value.clone());
                }
            } else {
                raw["record"] = desired;
            }
        } else {
            rows.remove(&id);
        }
        if record.is_none()
            && matches!(Self::NAME, "synced_servers" | "synced_pack_catalog")
        {
            let target = serde_json::to_value(wanted)?;
            let dependent = if Self::NAME == "synced_servers" {
                "instance_servers"
            } else {
                "synced_pack_instances"
            };
            if let Some(rows) = transaction.collections.get_mut(dependent) {
                if Self::NAME == "synced_servers" {
                    for raw in rows.values_mut() {
                        if raw["record"]["excluded_synced_server_id"] == target
                        {
                            raw["record"]["excluded_synced_server_id"] =
                                Value::Null;
                        }
                    }
                } else {
                    rows.retain(|_, raw| raw["key"]["pack_id"] != target);
                }
            }
        }
        transaction.changed = true;
        Ok(())
    }
    async fn atom_set_all(
        unit: &mut Atom,
        rows: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        for (index, (key, _)) in rows.iter().enumerate() {
            if rows[..index].iter().any(|(other, _)| other == key) {
                return Err(crate::ErrorKind::InputError(
                    "Duplicate sync identity".into(),
                )
                .into());
            }
        }
        let previous = Self::atom_get_all(unit).await?;
        for (key, _) in previous {
            if !rows.iter().any(|(desired, _)| desired == &key) {
                Self::atom_set(unit, &key, None).await?;
            }
        }
        for (key, record) in rows {
            Self::atom_set(unit, key, Some(record)).await?;
        }
        Ok(())
    }
    async fn atom_set_scoped(
        unit: &mut Atom,
        scope: &Self::Scope,
        rows: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        if rows.iter().any(|(key, _)| !Self::in_scope(scope, key)) {
            return Err(crate::ErrorKind::InputError(
                "Sync record outside scope".into(),
            )
            .into());
        }
        for (index, (key, _)) in rows.iter().enumerate() {
            if rows[..index].iter().any(|(other, _)| other == key) {
                return Err(crate::ErrorKind::InputError(
                    "Duplicate sync identity".into(),
                )
                .into());
            }
        }
        let previous = Self::atom_get_scoped(unit, scope).await?;
        for (key, _) in previous {
            if !rows.iter().any(|(desired, _)| desired == &key) {
                Self::atom_set(unit, &key, None).await?;
            }
        }
        for (key, record) in rows {
            Self::atom_set(unit, key, Some(record)).await?;
        }
        Ok(())
    }
}
macro_rules! verbs{
 ($collection:ty,$key:ty,$scope:ty,$record:ty)=>{impl $collection{
  pub(crate) async fn get(handle:&$crate::state::store::Handle,key:&$key)->$crate::Result<Option<$record>>{<$collection as $crate::state::sync_state::collection::Collection>::get(handle,key).await}
  pub(crate) async fn get_all(handle:&$crate::state::store::Handle)->$crate::Result<Vec<($key,$record)>>{<$collection as $crate::state::sync_state::collection::Collection>::get_all(handle).await}
  pub(crate) async fn get_scoped(handle:&$crate::state::store::Handle,scope:&$scope)->$crate::Result<Vec<($key,$record)>>{<$collection as $crate::state::sync_state::collection::Collection>::get_scoped(handle,scope).await}
  pub(crate) async fn set(handle:&$crate::state::store::Handle,key:&$key,record:Option<&$record>)->$crate::Result<()>{<$collection as $crate::state::sync_state::collection::Collection>::set(handle,key,record).await}
  pub(crate) async fn set_all(handle:&$crate::state::store::Handle,rows:&[($key,$record)])->$crate::Result<()>{<$collection as $crate::state::sync_state::collection::Collection>::set_all(handle,rows).await}
  pub(crate) async fn set_scoped(handle:&$crate::state::store::Handle,scope:&$scope,rows:&[($key,$record)])->$crate::Result<()>{<$collection as $crate::state::sync_state::collection::Collection>::set_scoped(handle,scope,rows).await}
  pub(crate) async fn atom_get(unit:&mut $crate::state::store::Atom,key:&$key)->$crate::Result<Option<$record>>{<$collection as $crate::state::sync_state::collection::Collection>::atom_get(unit,key).await}
  pub(crate) async fn atom_get_all(unit:&mut $crate::state::store::Atom)->$crate::Result<Vec<($key,$record)>>{<$collection as $crate::state::sync_state::collection::Collection>::atom_get_all(unit).await}
  pub(crate) async fn atom_get_scoped(unit:&mut $crate::state::store::Atom,scope:&$scope)->$crate::Result<Vec<($key,$record)>>{<$collection as $crate::state::sync_state::collection::Collection>::atom_get_scoped(unit,scope).await}
  pub(crate) async fn atom_set(unit:&mut $crate::state::store::Atom,key:&$key,record:Option<&$record>)->$crate::Result<()>{<$collection as $crate::state::sync_state::collection::Collection>::atom_set(unit,key,record).await}
  pub(crate) async fn atom_set_all(unit:&mut $crate::state::store::Atom,rows:&[($key,$record)])->$crate::Result<()>{<$collection as $crate::state::sync_state::collection::Collection>::atom_set_all(unit,rows).await}
  pub(crate) async fn atom_set_scoped(unit:&mut $crate::state::store::Atom,scope:&$scope,rows:&[($key,$record)])->$crate::Result<()>{<$collection as $crate::state::sync_state::collection::Collection>::atom_set_scoped(unit,scope,rows).await}
 }};
}
pub(crate) use verbs;
