//! Document transactions coordinated by a durable core decision, without a SQL projection.
use super::Handle;
use fs4::tokio::AsyncFileExt;
use serde_json::{Map, Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

pub(crate) fn runtime(pool: &sqlx::SqlitePool) -> bool {
    pool.connect_options()
        .get_filename()
        .file_name()
        .is_some_and(|name| name == "system.db")
        || super::recovery_mode::root(pool).is_some()
}

pub(crate) struct Pending {
    root: PathBuf,
    _guard: tokio::sync::MutexGuard<'static, ()>,
    _file: tokio::fs::File,
    documents: BTreeMap<String, Value>,
    pub(crate) settings: Option<super::settings::SettingsRecord>,
}
impl Pending {
    pub(crate) async fn begin(handle: &Handle) -> crate::Result<Self> {
        let root = handle.persistence().root().to_owned();
        // sched-order: theseus::state::documents::WRITES
        let guard = crate::state::documents::WRITES.lock().await;
        let file = lock_files(&root).await?;
        Ok(Self {
            root,
            _guard: guard,
            _file: file,
            documents: BTreeMap::new(),
            settings: None,
        })
    }
    pub(crate) async fn document(
        &mut self,
        name: &str,
    ) -> crate::Result<&mut Value> {
        if !self.documents.contains_key(name) {
            let value = read_local(&self.root, name, true).await?;
            self.documents.insert(name.to_owned(), value);
        }
        Ok(self.documents.get_mut(name).expect("loaded document"))
    }
    pub(crate) fn view(&self, name: &str) -> Option<&Value> {
        self.documents.get(name)
    }
    pub(crate) async fn snapshot(
        &self,
        handle: &Handle,
        changed: Option<&std::collections::BTreeSet<String>>,
    ) -> crate::Result<Vec<(String, Value)>> {
        let mut documents = self.documents.clone();
        if let Some(record) = &self.settings {
            let source = crate::state::documents::Documents::at(
                self.root.join("settings"),
                handle.pool(),
            )
            .with_config_root(crate::state::documents::config_root());
            for (name, value) in
                crate::state::documents::settings::staged_row_documents(
                    &source,
                    crate::state::Settings::from_record(record.clone()),
                    changed,
                )
                .await?
            {
                documents.insert(name, value);
            }
            match read_local(&self.root, "internal", true).await {
                Ok(mut internal) => {
                    let old = internal["data"]["personalized_ads"]
                        .as_bool()
                        .unwrap_or(false);
                    if old != record.personalized_ads {
                        payload(&mut internal)?.insert(
                            "personalized_ads".into(),
                            json!(record.personalized_ads),
                        );
                        documents.insert("internal".into(), internal);
                    }
                }
                Err(error) if record.personalized_ads => return Err(error),
                Err(_) => {}
            }
        }
        Ok(documents.into_iter().collect())
    }
}
pub(crate) fn payload(
    value: &mut Value,
) -> crate::Result<&mut Map<String, Value>> {
    value
        .get_mut("data")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| {
            crate::ErrorKind::InputError("Invalid configuration payload".into())
                .into()
        })
}
pub(crate) async fn read_local(
    root: &Path,
    name: &str,
    writable: bool,
) -> crate::Result<Value> {
    let path = root.join("settings").join(format!("{name}.json"));
    let value = match tokio::fs::metadata(&path).await {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(json!({"schema_version":1,"data":{}}));
        }
        Err(error) => return Err(error.into()),
        Ok(metadata) if !metadata.is_file() || metadata.len() > 1 << 20 => {
            return Err(crate::ErrorKind::InputError(
                "Configuration document exceeds its bound".into(),
            )
            .into());
        }
        Ok(_) => {
            serde_json::from_slice::<Value>(&tokio::fs::read(&path).await?)?
        }
    };
    let version = value.get("schema_version").and_then(Value::as_u64);
    if !value.get("data").is_some_and(Value::is_object)
        || version.is_none()
        || (writable && (version != Some(1) || value.get("protocol").is_some()))
    {
        return Err(crate::ErrorKind::InputError(format!(
            "The {name} configuration cannot be safely {} by this build",
            if writable { "updated" } else { "read" }
        ))
        .into());
    }
    Ok(value)
}
async fn provided(
    handle: &Handle,
    name: &str,
) -> crate::Result<Map<String, Value>> {
    let source = crate::state::documents::Documents::at(
        handle.persistence().root().join("settings"),
        handle.pool(),
    )
    .with_config_root(crate::state::documents::config_root());
    match source.shipped_keys(name, 1).await {
        crate::state::documents::Shipped::Keys(values) => Ok(values),
        crate::state::documents::Shipped::Missing => Ok(Map::new()),
        crate::state::documents::Shipped::Unreadable => {
            Err(crate::ErrorKind::InputError(format!(
                "The deployment's {name} configuration is unavailable"
            ))
            .into())
        }
    }
}
pub(crate) async fn composed(
    handle: &Handle,
    name: &str,
) -> crate::Result<Map<String, Value>> {
    let mut result = provided(handle, name).await?;
    let local = read_local(handle.persistence().root(), name, false).await?;
    merge(
        &mut result,
        local["data"].as_object().expect("validated payload"),
    );
    Ok(result)
}
pub(crate) async fn preferred_source(handle: &Handle) -> crate::Result<String> {
    Ok(composed(handle, "resources")
        .await?
        .get("preferred_source")
        .and_then(Value::as_str)
        .unwrap_or("modrinth")
        .to_owned())
}
pub(crate) async fn set_preferred_source(
    handle: &Handle,
    source: &str,
) -> crate::Result<()> {
    let mut unit = super::atom(handle).await?;
    payload(unit.configuration().await?.document("resources").await?)?
        .insert("preferred_source".into(), json!(source));
    unit.commit().await
}

pub(crate) trait Preference: super::Collection {
    fn rows(
        data: &Map<String, Value>,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>>;
    fn put(
        data: &mut Map<String, Value>,
        key: &Self::Key,
        value: Option<&Self::Record>,
    ) -> crate::Result<()>;
    async fn guard(
        _unit: &mut super::Atom,
        _key: &Self::Key,
        _value: Option<&Self::Record>,
    ) -> crate::Result<()> {
        Ok(())
    }
}
pub(crate) async fn preference_rows<C: Preference>(
    unit: &mut super::Atom,
) -> crate::Result<Vec<(C::Key, C::Record)>> {
    let pending = unit
        .configuration()
        .await?
        .view("sync")
        .and_then(|value| value.get("data"))
        .and_then(Value::as_object)
        .cloned();
    let values = if let Some(pending) = pending {
        let mut values = provided(&unit.handle, "sync").await?;
        merge(&mut values, &pending);
        values
    } else {
        composed(&unit.handle, "sync").await?
    };
    C::rows(&values)
}
macro_rules! preference_verbs {
    ($collection:ty) => {
        impl $collection {
            pub(crate) async fn get(
                handle: &super::Handle,
                key: &<Self as super::Collection>::Key,
            ) -> crate::Result<Option<<Self as super::Collection>::Record>>
            {
                if !super::configuration_documents::runtime(handle.pool()) {
                    return <Self as super::Collection>::get(handle, key).await;
                }
                Ok(<Self as super::configuration_documents::Preference>::rows(
                    &super::configuration_documents::composed(handle, "sync")
                        .await?,
                )?
                .into_iter()
                .find(|(candidate, _)| candidate == key)
                .map(|(_, record)| record))
            }
            pub(crate) async fn get_all(
                handle: &super::Handle,
            ) -> crate::Result<
                Vec<(
                    <Self as super::Collection>::Key,
                    <Self as super::Collection>::Record,
                )>,
            > {
                if !super::configuration_documents::runtime(handle.pool()) {
                    return <Self as super::Collection>::get_all(handle).await;
                }
                <Self as super::configuration_documents::Preference>::rows(
                    &super::configuration_documents::composed(handle, "sync")
                        .await?,
                )
            }
            pub(crate) async fn get_scoped(
                handle: &super::Handle,
                scope: &<Self as super::Collection>::Scope,
            ) -> crate::Result<
                Vec<(
                    <Self as super::Collection>::Key,
                    <Self as super::Collection>::Record,
                )>,
            > {
                Ok(Self::get_all(handle)
                    .await?
                    .into_iter()
                    .filter(|(key, _)| {
                        <Self as super::Collection>::in_scope(scope, key)
                    })
                    .collect())
            }
            pub(crate) async fn atom_get(
                unit: &mut super::Atom,
                key: &<Self as super::Collection>::Key,
            ) -> crate::Result<Option<<Self as super::Collection>::Record>>
            {
                if !super::configuration_documents::runtime(unit.handle.pool())
                {
                    return <Self as super::Collection>::atom_get(unit, key)
                        .await;
                }
                Ok(super::configuration_documents::preference_rows::<Self>(
                    unit,
                )
                .await?
                .into_iter()
                .find(|(candidate, _)| candidate == key)
                .map(|(_, record)| record))
            }
            pub(crate) async fn atom_get_all(
                unit: &mut super::Atom,
            ) -> crate::Result<
                Vec<(
                    <Self as super::Collection>::Key,
                    <Self as super::Collection>::Record,
                )>,
            > {
                if !super::configuration_documents::runtime(unit.handle.pool())
                {
                    return <Self as super::Collection>::atom_get_all(unit)
                        .await;
                }
                super::configuration_documents::preference_rows::<Self>(unit)
                    .await
            }
            pub(crate) async fn atom_get_scoped(
                unit: &mut super::Atom,
                scope: &<Self as super::Collection>::Scope,
            ) -> crate::Result<
                Vec<(
                    <Self as super::Collection>::Key,
                    <Self as super::Collection>::Record,
                )>,
            > {
                Ok(Self::atom_get_all(unit)
                    .await?
                    .into_iter()
                    .filter(|(key, _)| {
                        <Self as super::Collection>::in_scope(scope, key)
                    })
                    .collect())
            }
            pub(crate) async fn atom_set(
                unit: &mut super::Atom,
                key: &<Self as super::Collection>::Key,
                record: Option<&<Self as super::Collection>::Record>,
            ) -> crate::Result<()> {
                if !super::configuration_documents::runtime(unit.handle.pool())
                {
                    return <Self as super::Collection>::atom_set(
                        unit, key, record,
                    )
                    .await;
                }
                <Self as super::configuration_documents::Preference>::guard(
                    unit, key, record,
                )
                .await?;
                <Self as super::configuration_documents::Preference>::put(
                    super::configuration_documents::payload(
                        unit.configuration().await?.document("sync").await?,
                    )?,
                    key,
                    record,
                )
            }
            pub(crate) async fn set(
                handle: &super::Handle,
                key: &<Self as super::Collection>::Key,
                record: Option<&<Self as super::Collection>::Record>,
            ) -> crate::Result<()> {
                let mut unit = super::atom(handle).await?;
                Self::atom_set(&mut unit, key, record).await?;
                unit.commit().await
            }
            pub(crate) async fn atom_set_all(
                unit: &mut super::Atom,
                records: &[(
                    <Self as super::Collection>::Key,
                    <Self as super::Collection>::Record,
                )],
            ) -> crate::Result<()> {
                super::ensure_unique::<Self>(records)?;
                for (key, _) in Self::atom_get_all(unit).await? {
                    if !records.iter().any(|(kept, _)| kept == &key) {
                        Self::atom_set(unit, &key, None).await?;
                    }
                }
                for (key, record) in records {
                    Self::atom_set(unit, key, Some(record)).await?;
                }
                Ok(())
            }
            pub(crate) async fn set_all(
                handle: &super::Handle,
                records: &[(
                    <Self as super::Collection>::Key,
                    <Self as super::Collection>::Record,
                )],
            ) -> crate::Result<()> {
                let mut unit = super::atom(handle).await?;
                Self::atom_set_all(&mut unit, records).await?;
                unit.commit().await
            }
            pub(crate) async fn atom_set_scoped(
                unit: &mut super::Atom,
                scope: &<Self as super::Collection>::Scope,
                records: &[(
                    <Self as super::Collection>::Key,
                    <Self as super::Collection>::Record,
                )],
            ) -> crate::Result<()> {
                super::ensure_unique::<Self>(records)?;
                super::ensure_in_scope::<Self>(scope, records)?;
                for (key, _) in Self::atom_get_scoped(unit, scope).await? {
                    if !records.iter().any(|(kept, _)| kept == &key) {
                        Self::atom_set(unit, &key, None).await?;
                    }
                }
                for (key, record) in records {
                    Self::atom_set(unit, key, Some(record)).await?;
                }
                Ok(())
            }
            pub(crate) async fn set_scoped(
                handle: &super::Handle,
                scope: &<Self as super::Collection>::Scope,
                records: &[(
                    <Self as super::Collection>::Key,
                    <Self as super::Collection>::Record,
                )],
            ) -> crate::Result<()> {
                let mut unit = super::atom(handle).await?;
                Self::atom_set_scoped(&mut unit, scope, records).await?;
                unit.commit().await
            }
        }
    };
}
pub(crate) use preference_verbs;
pub(crate) fn map<'a>(
    data: &'a mut Map<String, Value>,
    key: &str,
) -> crate::Result<&'a mut Map<String, Value>> {
    let value = data.entry(key.to_owned()).or_insert_with(|| json!({}));
    value.as_object_mut().ok_or_else(|| {
        crate::ErrorKind::InputError("Unknown preference shape retained".into())
            .into()
    })
}

/// Caller owns document WRITES, after reserving core if it needs both domains.
pub(crate) async fn lock_files(root: &Path) -> crate::Result<tokio::fs::File> {
    tokio::fs::create_dir_all(root.join("settings")).await?;
    let file = tokio::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(root.join("settings/.writer-lock"))
        .await?;
    file.lock_exclusive().await?;
    Ok(file)
}

fn merge(target: &mut Map<String, Value>, source: &Map<String, Value>) {
    for (key, value) in source {
        match (target.get_mut(key), value) {
            (Some(Value::Object(target)), Value::Object(source)) => {
                merge(target, source)
            }
            _ => {
                target.insert(key.clone(), value.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::store;
    #[tokio::test]
    async fn readonly_recovery_composes_settings_without_any_settings_table_or_writes()
     {
        let root = tempfile::tempdir().unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let appearance=br#"{"schema_version":99,"data":{"locale":"zh-CN","future":{"keep":true}}}"#;
        tokio::fs::create_dir_all(root.path().join("settings"))
            .await
            .unwrap();
        tokio::fs::write(
            root.path().join("settings/appearance.json"),
            appearance,
        )
        .await
        .unwrap();
        tokio::fs::write(
            root.path().join("settings/download.json"),
            b"broken document",
        )
        .await
        .unwrap();
        store::recovery_mode::register(&pool, root.path());
        sqlx::query("PRAGMA query_only=ON")
            .execute(&pool)
            .await
            .unwrap();
        let handle = Handle::new(pool);
        let settings = crate::state::Settings::get(&handle).await.unwrap();
        assert_eq!(settings.locale, "zh-CN");
        assert_eq!(settings.version, 3);
        assert_eq!(
            settings.max_concurrent_downloads,
            crate::state::Settings::default().max_concurrent_downloads
        );
        assert!(settings.update(&handle).await.is_err());
        assert!(!root.path().join("settings/.writer-lock").exists());
        assert_eq!(
            tokio::fs::read(root.path().join("settings/appearance.json"))
                .await
                .unwrap(),
            appearance
        );
    }
    #[tokio::test]
    async fn targeted_settings_patch_preserves_envelope_and_newer_unrelated_domain()
     {
        let root = tempfile::tempdir().unwrap();
        let deployment = tempfile::tempdir().unwrap();
        crate::state::documents::test_support::deploy(&deployment);
        let shipped=br#"{"schema_version":2,"data":{"locale":"en-US","hide_nametag_skins_page":true}}"#;
        tokio::fs::write(deployment.path().join("appearance.json"), shipped)
            .await
            .unwrap();
        let pool = store::databases::open(root.path()).await.unwrap();
        let handle = Handle::new(pool);
        let local = json!({"schema_version":2,"envelope_extension":{"keep":true},"data":{"locale":"zh-CN","future":7}});
        crate::state::persistence::document::write_json(
            &root.path().join("settings/appearance.json"),
            serde_json::to_vec(&local).unwrap(),
        )
        .await
        .unwrap();
        let download = br#"{"schema_version":999,"data":{"future":true}}"#;
        crate::state::persistence::document::write_json(
            &root.path().join("settings/download.json"),
            download.to_vec(),
        )
        .await
        .unwrap();
        let before = crate::state::Settings::get(&handle).await.unwrap();
        assert_eq!(before.locale, "zh-CN");
        assert!(before.hide_nametag_skins_page);
        crate::api::settings::patch_with(&handle, json!({"locale":"zh-TW"}))
            .await
            .unwrap();
        let saved: Value = serde_json::from_slice(
            &tokio::fs::read(root.path().join("settings/appearance.json"))
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(saved["envelope_extension"]["keep"], true);
        assert_eq!(saved["data"]["future"], 7);
        assert_eq!(saved["data"]["locale"], "zh-TW");
        assert!(saved["data"].get("hide_nametag_skins_page").is_none());
        assert_eq!(
            tokio::fs::read(root.path().join("settings/download.json"))
                .await
                .unwrap(),
            download
        );
        assert_eq!(
            tokio::fs::read(deployment.path().join("appearance.json"))
                .await
                .unwrap(),
            shipped
        );
    }
    #[tokio::test]
    async fn deleting_a_preference_removes_only_its_checkpoint_in_the_same_decision()
     {
        use store::instance_sync_checkpoints::*;
        use store::instance_sync_preferences::*;
        let root = tempfile::tempdir().unwrap();
        let pool = store::databases::open(root.path()).await.unwrap();
        let handle = Handle::new(pool.clone());
        sqlx::query("INSERT INTO instances(id,path,install_stage,launcher_feature_version,name,created,modified) VALUES('first','first','installed','none','First',1,1),('second','second','installed','none','Second',1,1)").execute(&pool).await.unwrap();
        let key = |id: &str| InstancePreferenceKey {
            instance_id: id.into(),
            feature: "screenshots".into(),
        };
        let checkpoint = |id: &str| CheckpointKey {
            instance_id: id.into(),
            feature: "screenshots".into(),
            variant: "default".into(),
        };
        for id in ["first", "second"] {
            InstanceSyncPreferences::set(
                &handle,
                &key(id),
                Some(&InstancePreferenceRecord { enabled: true }),
            )
            .await
            .unwrap();
            InstanceSyncCheckpoints::set(
                &handle,
                &checkpoint(id),
                Some(&CheckpointRecord {
                    expected_sha1: "hash".into(),
                    merge_base: None,
                    source_revision: 0,
                    status: CheckpointStatus::Ready,
                    link_mode: None,
                }),
            )
            .await
            .unwrap();
        }
        {
            let mut unit = store::atom(&handle).await.unwrap();
            InstanceSyncPreferences::atom_set(&mut unit, &key("first"), None)
                .await
                .unwrap();
            assert!(
                InstanceSyncPreferences::atom_get(&mut unit, &key("first"))
                    .await
                    .unwrap()
                    .is_none()
            );
        }
        assert!(
            InstanceSyncCheckpoints::get(&handle, &checkpoint("first"))
                .await
                .unwrap()
                .is_some()
        );
        InstanceSyncPreferences::set(&handle, &key("first"), None)
            .await
            .unwrap();
        assert!(
            InstanceSyncCheckpoints::get(&handle, &checkpoint("first"))
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            InstanceSyncCheckpoints::get(&handle, &checkpoint("second"))
                .await
                .unwrap()
                .is_some()
        );
    }
}
