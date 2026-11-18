//! Complete, bounded recovery copies. Healthy SQL always remains authoritative.
use crate::state::{
    DirectoryInfo,
    instances::{InstanceMetadata, adapters::sqlite::manifest_rows},
    persistence::{Persistence, document, status::Availability},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::io::{Read, Write};
use std::path::Path;

const PROTOCOL: &str = "axolotl-instance-recovery";
const SHARD_BYTES: usize = 1024 * 1024;
const MAX_TOTAL_BYTES: usize = 64 * 1024 * 1024;
const MAX_SHARDS: usize = 4096;
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Shard {
    file: String,
    sha256: String,
    bytes: usize,
    #[serde(default)]
    table: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Head {
    instance_id: String,
    database_generation: String,
    database_revision: i64,
    deleted: bool,
    #[serde(default)]
    shards: Vec<Shard>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct RecoverySnapshot {
    pub metadata: InstanceMetadata,
    pub relationships: serde_json::Map<String, Value>,
}
pub(crate) fn id_key(id: &str) -> String {
    format!("{:x}", Sha256::digest(id.as_bytes()))
}
fn authority(root: &Path, id: &str) -> Persistence {
    Persistence::new(
        root.join(crate::state::layout::INSTANCE_RECOVERY)
            .join(id_key(id)),
    )
}
fn head(
    root: &Path,
    id: &str,
) -> crate::state::persistence::document::Document {
    authority(root, id)
        .document("recovery.json", PROTOCOL, 1)
        .expect("constant recovery head")
}

pub(crate) async fn head_present(root: &Path, id: &str) -> crate::Result<bool> {
    Ok(tokio::fs::try_exists(head(root, id).path()).await?)
}

pub(crate) async fn publish(
    dirs: &DirectoryInfo,
    pool: &SqlitePool,
    id: &str,
) -> crate::Result<()> {
    let persistence = Persistence::new(&dirs.settings_dir);
    let requested = manifest_rows::pending(pool)
        .await?
        .into_iter()
        .find(|(candidate, _, _)| candidate == id);
    let Some((_, revision, deleted)) = requested else {
        return Ok(());
    };
    let target = head(persistence.root(), id);
    let database_generation = manifest_rows::generation(pool).await?;
    let mut shards = Vec::new();
    let mut mirror = None;
    if !deleted {
        let snapshot = match manifest_rows::snapshot(pool, id, revision).await {
            Ok(snapshot) => snapshot,
            Err(error) => {
                if matches!(error.raw.as_ref(), crate::ErrorKind::InputError(_))
                {
                    // A newer core record must not leave a launchable stale
                    // copy behind when this reader cannot represent it.
                    mark_unrepresentable(&target, id, revision).await?;
                    if let Some(path)=super::adapters::sqlite::instance_rows::get_instance_path_by_id(id,pool).await? {
                        let profile=dirs.instances_dir().join(path);
                        let mirror=profile.join(".axolotl/recovery");
                        if let Ok(canonical)=tokio::fs::canonicalize(&profile).await
                            && canonical.starts_with(tokio::fs::canonicalize(dirs.instances_dir()).await?)
                            && managed_destination(&canonical,&mirror).await?
                            && tokio::fs::try_exists(mirror.join("recovery.json")).await? {
                            let copy=Persistence::new(mirror).document("recovery.json",PROTOCOL,1)?;
                            mark_unrepresentable(&copy,id,revision).await?;
                        }
                    }
                }
                return Err(error);
            }
        };
        let Some((metadata, relationships)) = snapshot else {
            return Ok(());
        };
        if !metadata.instance.is_direct_linked()
            && metadata.instance.game_dir_override.is_none()
            && metadata.instance.symlink_target.is_none()
        {
            mirror = Some(
                dirs.instances_dir()
                    .join(&metadata.instance.path)
                    .join(".axolotl/recovery"),
            );
        }
        let generation = uuid::Uuid::new_v4().to_string();
        let mut payloads = vec![(None, serde_json::to_vec(&metadata)?)];
        for (table, rows) in relationships {
            let rows = rows.as_array().ok_or_else(|| {
                crate::ErrorKind::InputError(
                    "Recovery table is not an array".into(),
                )
            })?;
            let mut chunk = Vec::new();
            let mut size = 2usize;
            for row in rows {
                let length = serde_json::to_vec(row)?.len();
                if length > SHARD_BYTES / 2 {
                    return Err(crate::ErrorKind::InputError(
                        "Recovery relationship record exceeds shard limit"
                            .into(),
                    )
                    .into());
                }
                if size + length > SHARD_BYTES && !chunk.is_empty() {
                    payloads.push((
                        Some(table.clone()),
                        serde_json::to_vec(&chunk)?,
                    ));
                    chunk.clear();
                    size = 2;
                }
                chunk.push(row.clone());
                size += length + 1;
            }
            if !chunk.is_empty() {
                payloads.push((Some(table), serde_json::to_vec(&chunk)?));
            }
        }
        if payloads.len() > MAX_SHARDS {
            return Err(crate::ErrorKind::InputError(
                "Instance recovery exceeds supported shard count".into(),
            )
            .into());
        }
        let total: usize = payloads.iter().map(|(_, bytes)| bytes.len()).sum();
        if total > MAX_TOTAL_BYTES
            || payloads.iter().any(|(_, bytes)| bytes.len() > SHARD_BYTES)
        {
            return Err(crate::ErrorKind::InputError(
                "Instance recovery exceeds supported byte limits".into(),
            )
            .into());
        }
        for (index, (table, bytes)) in payloads.into_iter().enumerate() {
            let file = format!("generations/{generation}/{index}.json");
            document::write_json(
                &authority(persistence.root(), id).root().join(&file),
                bytes.clone(),
            )
            .await?;
            shards.push(Shard {
                file,
                sha256: format!("{:x}", Sha256::digest(&bytes)),
                bytes: bytes.len(),
                table,
            });
        }
    }
    let desired = Head {
        instance_id: id.to_owned(),
        database_generation,
        database_revision: revision,
        deleted,
        shards,
    };
    let copy_root = authority(persistence.root(), id);
    manifest_rows::publish(pool,id,revision,||async move{
        if let Some(mirror)=mirror {
            let profile=mirror.parent().and_then(Path::parent).expect("managed profile");
            let canonical=tokio::fs::canonicalize(profile).await.ok();
            if let Some(canonical)=canonical && canonical.starts_with(tokio::fs::canonicalize(dirs.instances_dir()).await?) && managed_destination(&canonical, &mirror).await? {
                for shard in &desired.shards {let body=read_shard(copy_root.root(),shard).await?;document::write_json(&mirror.join(&shard.file),body).await?;}
                let document=Persistence::new(&mirror).document("recovery.json",PROTOCOL,1)?;
                let copy=serde_json::to_value(&desired)?;
                document.update(move|data|{ensure_supported(data)?;
                    if data.get("deletion_intent")==Some(&json!(true)) && copy.get("deleted")!=Some(&json!(true)) {return Err(crate::ErrorKind::InputError("Pending instance deletion retains its mirror fence".into()).into());}
                    if data.get("database_generation")==copy.get("database_generation") && data.get("database_revision").and_then(Value::as_i64).is_some_and(|existing|existing>revision) {return Err(crate::ErrorKind::InputError("Newer mirror publication retained".into()).into());}
                    data.remove("unsupported_core_revision");
                    for(key,value)in copy.as_object().expect("head"){data.insert(key.clone(),value.clone());}Ok(())}).await?;
            }
        }
        target.update(move|data|{
            ensure_supported(data)?;
            if data.get("deletion_intent")==Some(&json!(true)) && !desired.deleted {return Err(crate::ErrorKind::InputError("Pending instance deletion retains its recovery fence".into()).into());}
            if data.get("database_generation")==Some(&json!(desired.database_generation)) && let Some(existing)=data.get("database_revision").and_then(Value::as_i64) && existing>revision {return Err(crate::ErrorKind::InputError("Newer recovery publication retained".into()).into());}
            data.remove("unsupported_core_revision");
            for(key,value)in serde_json::to_value(desired)?.as_object().expect("head object"){data.insert(key.clone(),value.clone());}Ok(())
        }).await?;Ok(())
    }).await?;
    Ok(())
}

async fn mark_unrepresentable(
    document: &crate::state::persistence::document::Document,
    id: &str,
    revision: i64,
) -> crate::Result<()> {
    let id = id.to_owned();
    document
        .update(move |data| {
            ensure_supported(data)?;
            data.insert("instance_id".into(), json!(id));
            data.insert("unsupported_core_revision".into(), json!(revision));
            Ok(())
        })
        .await?;
    Ok(())
}

pub(crate) async fn read(
    root: &Path,
    id: &str,
) -> crate::Result<Option<RecoverySnapshot>> {
    let read = head(root, id).read().await?;
    if !matches!(
        read.availability,
        Availability::Ready | Availability::Recovered
    ) {
        return Err(crate::ErrorKind::FSError(
            "Instance recovery copy is unavailable or newer".into(),
        )
        .into());
    }
    let snapshot = read.snapshot.expect("readable head");
    if snapshot
        .data()
        .is_some_and(|data| data.contains_key("unsupported_core_revision"))
    {
        return Err(crate::ErrorKind::InputError("The last core revision cannot be represented by this recovery reader".into()).into());
    }
    ensure_supported(snapshot.data().expect("payload"))?;
    let head: Head = snapshot.decode()?;
    if head.instance_id != id {
        return Err(crate::ErrorKind::InputError(
            "Recovery identity mismatch".into(),
        )
        .into());
    }
    if head.deleted {
        return Ok(None);
    }
    if head.shards.len() > MAX_SHARDS
        || head
            .shards
            .iter()
            .try_fold(0usize, |sum, shard| sum.checked_add(shard.bytes))
            .is_none_or(|total| total > MAX_TOTAL_BYTES)
    {
        return Err(crate::ErrorKind::InputError(
            "Recovery head exceeds shard limit".into(),
        )
        .into());
    }
    let mut metadata = None;
    let mut relationships = serde_json::Map::new();
    for shard in head.shards {
        let chunk = read_shard(authority(root, id).root(), &shard).await?;
        if let Some(table) = shard.table {
            let rows: Vec<Value> = serde_json::from_slice(&chunk)?;
            relationships
                .entry(table)
                .or_insert_with(|| json!([]))
                .as_array_mut()
                .expect("rows")
                .extend(rows);
        } else {
            if metadata.is_some() {
                return Err(crate::ErrorKind::InputError(
                    "Duplicate recovery descriptor".into(),
                )
                .into());
            }
            let value: Value = serde_json::from_slice(&chunk)?;
            ensure_supported(value.as_object().ok_or_else(|| {
                crate::ErrorKind::InputError(
                    "Recovery descriptor is not an object".into(),
                )
            })?)?;
            metadata = Some(serde_json::from_value::<InstanceMetadata>(value)?);
        }
    }
    let snapshot = RecoverySnapshot {
        metadata: metadata.ok_or_else(|| {
            crate::ErrorKind::InputError("Recovery descriptor missing".into())
        })?,
        relationships,
    };
    if snapshot.metadata.instance.id != id {
        return Err(crate::ErrorKind::InputError(
            "Recovery payload identity mismatch".into(),
        )
        .into());
    }
    Ok(Some(snapshot))
}

pub(crate) async fn list(root: &Path) -> crate::Result<Vec<RecoverySnapshot>> {
    let mut output = Vec::new();
    let mut dirs = match tokio::fs::read_dir(
        root.join(crate::state::layout::INSTANCE_RECOVERY),
    )
    .await
    {
        Ok(dirs) => dirs,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(output);
        }
        Err(error) => return Err(error.into()),
    };
    while let Some(entry) = dirs.next_entry().await? {
        if !entry.file_type().await?.is_dir() {
            continue;
        }
        let p = Persistence::new(entry.path());
        let read = match p.document("recovery.json", PROTOCOL, 1)?.read().await
        {
            Ok(read) => read,
            Err(error) => {
                tracing::warn!(%error,"Unreadable recovery head retained");
                continue;
            }
        };
        let id = read
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.data())
            .and_then(|data| data.get("instance_id"))
            .and_then(Value::as_str)
            .map(str::to_owned);
        if let Some(id) = id
            && id_key(&id) == entry.file_name().to_string_lossy()
        {
            match self::read(root, &id).await {
                Ok(Some(snapshot)) => output.push(snapshot),
                Ok(None) => {}
                Err(error) => {
                    tracing::warn!(%error,"Unreadable instance recovery copy retained")
                }
            }
        }
    }
    Ok(output)
}

/// Persist deletion intent before touching any instance files. Keep the last
/// complete shard references so a failed deletion remains inspectable.
pub(crate) async fn tombstone(
    dirs: &DirectoryInfo,
    pool: &SqlitePool,
    id: &str,
    revision: i64,
) -> crate::Result<()> {
    let database_generation = manifest_rows::generation(pool).await?;
    let mut targets = vec![head(&dirs.settings_dir, id)];
    if let Some(metadata) = manifest_rows::metadata(pool, id).await?
        && !metadata.instance.is_direct_linked()
        && metadata.instance.game_dir_override.is_none()
        && metadata.instance.symlink_target.is_none()
    {
        let profile = dirs.instances_dir().join(&metadata.instance.path);
        if let Ok(canonical) = tokio::fs::canonicalize(&profile).await {
            let mirror = profile.join(".axolotl/recovery");
            if canonical.starts_with(
                tokio::fs::canonicalize(dirs.instances_dir()).await?,
            ) && managed_destination(&canonical, &mirror).await?
            {
                targets.push(Persistence::new(mirror).document(
                    "recovery.json",
                    PROTOCOL,
                    1,
                )?);
            }
        }
    }
    for document in targets {
        let id = id.to_owned();
        let generation = database_generation.clone();
        document
            .update(move |data| {
                ensure_supported(data)?;
                data.insert("instance_id".into(), json!(id));
                data.insert("database_generation".into(), json!(generation));
                data.insert("deleted".into(), json!(true));
                data.insert("deletion_intent".into(), json!(true));
                data.insert("database_revision".into(), json!(revision));
                data.entry("shards").or_insert_with(|| json!([]));
                Ok(())
            })
            .await?;
    }
    Ok(())
}

/// Existing symlinks below a managed profile must not redirect recovery writes
/// into another installation. Check every existing ancestor before publication.
async fn managed_destination(
    profile: &Path,
    destination: &Path,
) -> crate::Result<bool> {
    let profile = tokio::fs::canonicalize(profile).await?;
    let mut current = destination.to_owned();
    loop {
        match tokio::fs::symlink_metadata(&current).await {
            Ok(_) => {
                let resolved = tokio::fs::canonicalize(&current).await?;
                if !resolved.starts_with(&profile) {
                    return Ok(false);
                }
                if resolved == profile {
                    return Ok(true);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let Some(parent) = current.parent() else {
            return Ok(false);
        };
        current = parent.to_owned();
    }
}

async fn read_shard(base: &Path, shard: &Shard) -> crate::Result<Vec<u8>> {
    validate_shard(shard)?;
    let path = crate::util::io::safe_relative_join(base, &shard.file)?;
    if !tokio::fs::canonicalize(&path)
        .await?
        .starts_with(tokio::fs::canonicalize(base).await?)
    {
        return Err(crate::ErrorKind::InputError(
            "Recovery shard escaped its authority".into(),
        )
        .into());
    }
    let file = tokio::fs::File::open(path).await?;
    let metadata = file.metadata().await?;
    if !metadata.is_file() || metadata.len() != shard.bytes as u64 {
        return Err(crate::ErrorKind::FSError(
            "Recovery shard size mismatch".into(),
        )
        .into());
    }
    let mut bytes = Vec::with_capacity(shard.bytes);
    use tokio::io::AsyncReadExt;
    file.take((SHARD_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .await?;
    verify_shard(shard, &bytes)?;
    Ok(bytes)
}
fn validate_shard(shard: &Shard) -> crate::Result<()> {
    let parts = shard.file.split('/').collect::<Vec<_>>();
    if shard.bytes > SHARD_BYTES
        || parts.len() != 3
        || parts[0] != "generations"
        || uuid::Uuid::parse_str(parts[1]).is_err()
        || !parts[2]
            .strip_suffix(".json")
            .is_some_and(|number| number.parse::<usize>().is_ok())
        || shard.sha256.len() != 64
    {
        return Err(crate::ErrorKind::InputError(
            "Invalid recovery shard descriptor".into(),
        )
        .into());
    }
    Ok(())
}
fn verify_shard(shard: &Shard, bytes: &[u8]) -> crate::Result<()> {
    if bytes.len() != shard.bytes
        || format!("{:x}", Sha256::digest(bytes)) != shard.sha256
    {
        return Err(crate::ErrorKind::FSError(
            "Recovery shard integrity failed".into(),
        )
        .into());
    }
    Ok(())
}

fn ensure_supported(
    data: &serde_json::Map<String, Value>,
) -> crate::Result<()> {
    if data.get("protocol").is_some()
        || ["schema_version", "schemaVersion"]
            .into_iter()
            .any(|key| data.get(key).is_some_and(|v| v.as_u64() != Some(1)))
    {
        return Err(crate::ErrorKind::InputError(
            "Unsupported instance recovery record preserved".into(),
        )
        .into());
    }
    Ok(())
}

pub(crate) async fn backup(
    root: &Path,
    destination: &Path,
) -> crate::Result<()> {
    let mut directories = match tokio::fs::read_dir(
        root.join(crate::state::layout::INSTANCE_RECOVERY),
    )
    .await
    {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };
    while let Some(entry) = directories.next_entry().await? {
        if !entry.file_type().await?.is_dir() {
            continue;
        }
        let base = entry.path();
        // Stage7 also creates operation-only instance directories.
        if !tokio::fs::try_exists(base.join("recovery.json")).await? {
            continue;
        }
        let document =
            Persistence::new(&base).document("recovery.json", PROTOCOL, 1)?;
        let target = destination
            .join(crate::state::layout::INSTANCE_RECOVERY)
            .join(entry.file_name());
        document
            .inspect(move |read| {
                if !matches!(
                    read.availability,
                    Availability::Ready | Availability::Recovered
                ) {
                    return Err(crate::ErrorKind::FSError(
                        "Cannot back up unavailable instance recovery copy"
                            .into(),
                    )
                    .into());
                }
                let snapshot = read.snapshot.expect("readable");
                ensure_supported(snapshot.data().expect("payload"))?;
                let descriptor: Head = snapshot.decode()?;
                if id_key(&descriptor.instance_id)
                    != base
                        .file_name()
                        .expect("instance folder")
                        .to_string_lossy()
                    || descriptor.shards.len() > MAX_SHARDS
                    || descriptor
                        .shards
                        .iter()
                        .try_fold(0usize, |sum, shard| {
                            sum.checked_add(shard.bytes)
                        })
                        .is_none_or(|total| total > MAX_TOTAL_BYTES)
                {
                    return Err(crate::ErrorKind::InputError(
                        "Invalid recovery backup descriptor".into(),
                    )
                    .into());
                }
                let canonical = std::fs::canonicalize(&base)?;
                for shard in &descriptor.shards {
                    validate_shard(shard)?;
                    let path = crate::util::io::safe_relative_join(
                        &base,
                        &shard.file,
                    )?;
                    if !std::fs::canonicalize(&path)?.starts_with(&canonical) {
                        return Err(crate::ErrorKind::InputError(
                            "Recovery backup shard escaped authority".into(),
                        )
                        .into());
                    }
                    let file = std::fs::File::open(path)?;
                    if !file.metadata()?.is_file()
                        || file.metadata()?.len() != shard.bytes as u64
                    {
                        return Err(crate::ErrorKind::FSError(
                            "Recovery backup shard size mismatch".into(),
                        )
                        .into());
                    }
                    let mut bytes = Vec::with_capacity(shard.bytes);
                    file.take((SHARD_BYTES + 1) as u64)
                        .read_to_end(&mut bytes)?;
                    verify_shard(shard, &bytes)?;
                    durable_copy(
                        &crate::util::io::safe_relative_join(
                            &target,
                            &shard.file,
                        )?,
                        &bytes,
                    )?;
                }
                // The source head lock remains held. Publish this exact envelope
                // last, retaining unknown fields and only complete immutable shards.
                let file = std::fs::File::open(base.join("recovery.json"))?;
                if file.metadata()?.len() > 16 * 1024 * 1024 {
                    return Err(crate::ErrorKind::InputError(
                        "Recovery backup head exceeds bound".into(),
                    )
                    .into());
                }
                let mut bytes = Vec::new();
                file.take(16 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
                if bytes.len() > 16 * 1024 * 1024 {
                    return Err(crate::ErrorKind::InputError(
                        "Recovery backup head grew beyond bound".into(),
                    )
                    .into());
                }
                durable_copy(&target.join("recovery.json"), &bytes)?;
                Ok(())
            })
            .await?;
    }
    Ok(())
}
fn durable_copy(path: &Path, bytes: &[u8]) -> crate::Result<()> {
    let parent = path.parent().expect("recovery destination parent");
    std::fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    crate::util::io::atomically_replace_file(temporary.path(), path)?;
    #[cfg(unix)]
    std::fs::File::open(parent)?.sync_all()?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::store::{databases, recovery_mode};

    async fn fixture() -> (tempfile::TempDir, DirectoryInfo, SqlitePool) {
        let root = tempfile::tempdir().unwrap();
        let dirs = DirectoryInfo {
            settings_dir: root.path().to_owned(),
            config_dir: root.path().to_owned(),
            app_identifier: "recovery-test".into(),
        };
        tokio::fs::create_dir_all(dirs.instances_dir())
            .await
            .unwrap();
        let pool = databases::open(root.path()).await.unwrap();
        (root, dirs, pool)
    }
    async fn instance(dirs: &DirectoryInfo, pool: &SqlitePool, id: &str) {
        tokio::fs::create_dir_all(dirs.instances_dir().join(id))
            .await
            .unwrap();
        let mut tx = pool.begin().await.unwrap();
        sqlx::query("INSERT INTO instances(id,path,name,install_stage,launcher_feature_version,update_channel,created,modified) VALUES(?,?,?,'installed','none','release',0,0)").bind(id).bind(id).bind(id).execute(&mut *tx).await.unwrap();
        let set = format!("set:{id}");
        sqlx::query("INSERT INTO instance_content_sets(id,instance_id,name,source_kind,status,game_version,loader,created,modified) VALUES(?,?,'local','local','available','1.21','vanilla',0,0)").bind(&set).bind(id).execute(&mut *tx).await.unwrap();
        sqlx::query("UPDATE instances SET applied_content_set_id=? WHERE id=?")
            .bind(set)
            .bind(id)
            .execute(&mut *tx)
            .await
            .unwrap();
        tx.commit().await.unwrap();
    }
    #[test]
    fn ids_and_shard_paths_cannot_alias_or_escape() {
        assert_ne!(id_key("local:a/b"), id_key("local:a?b"));
        for name in [
            "generations/../../outside.json",
            "generations/not-a-uuid/0.json",
            "/absolute.json",
        ] {
            assert!(
                validate_shard(&Shard {
                    file: name.into(),
                    sha256: "a".repeat(64),
                    bytes: 0,
                    table: None
                })
                .is_err()
            );
        }
        assert!(
            ensure_supported(json!({"schema_version":2}).as_object().unwrap())
                .is_err()
        );
    }
    #[tokio::test]
    async fn complete_copies_reopen_readonly_without_changing_the_core() {
        let (root, dirs, pool) = fixture().await;
        instance(&dirs, &pool, "one").await;
        publish(&dirs, &pool, "one").await.unwrap();
        let saved = read(root.path(), "one").await.unwrap().unwrap();
        assert_eq!(saved.metadata.instance.id, "one");
        assert!(
            dirs.instances_dir()
                .join("one/.axolotl/recovery/recovery.json")
                .is_file()
        );
        let recovery = databases::open_recovery(root.path()).await.unwrap();
        assert!(recovery_mode::ensure_writable(&recovery).is_err());
        let launch = super::super::commands::get_instance_launch_context(
            "one", &recovery,
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(launch.applied_content_set.game_version, "1.21");
        super::super::commands::set_instance_last_played(
            "one",
            chrono::Utc::now(),
            &recovery,
        )
        .await
        .unwrap();
        assert!(
            sqlx::query("UPDATE instances SET name='changed'")
                .execute(&recovery)
                .await
                .is_err()
        );
        let name: String =
            sqlx::query_scalar("SELECT name FROM instances WHERE id='one'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(name, "one");
    }
    #[tokio::test]
    async fn corrupt_one_copy_is_isolated_and_unknown_head_is_not_rewritten() {
        let (root, dirs, pool) = fixture().await;
        for id in ["one", "two"] {
            instance(&dirs, &pool, id).await;
            publish(&dirs, &pool, id).await.unwrap();
        }
        let read = head(root.path(), "one").read().await.unwrap();
        let descriptor: Head = read.snapshot.unwrap().decode().unwrap();
        let path = authority(root.path(), "one")
            .root()
            .join(&descriptor.shards[0].file);
        tokio::fs::write(path, b"broken").await.unwrap();
        let copies = list(root.path()).await.unwrap();
        assert_eq!(copies.len(), 1);
        assert_eq!(copies[0].metadata.instance.id, "two");
        head(root.path(), "two")
            .update(|data| {
                data.insert("schema_version".into(), json!(2));
                Ok(())
            })
            .await
            .unwrap();
        let before = tokio::fs::read(head(root.path(), "two").path())
            .await
            .unwrap();
        manifest_rows::force_dirty(&pool, "two").await.unwrap();
        assert!(publish(&dirs, &pool, "two").await.is_err());
        assert_eq!(
            before,
            tokio::fs::read(head(root.path(), "two").path())
                .await
                .unwrap()
        );
    }
    #[tokio::test]
    async fn deletion_fence_survives_cascades_and_blocks_republication() {
        let (root, dirs, pool) = fixture().await;
        instance(&dirs, &pool, "one").await;
        publish(&dirs, &pool, "one").await.unwrap();
        let revision = manifest_rows::revision(&pool, "one").await.unwrap();
        tombstone(&dirs, &pool, "one", revision).await.unwrap();
        manifest_rows::force_dirty(&pool, "one").await.unwrap();
        assert!(publish(&dirs, &pool, "one").await.is_err());
        assert!(read(root.path(), "one").await.unwrap().is_none());
        sqlx::query("DELETE FROM instances WHERE id='one'")
            .execute(&pool)
            .await
            .unwrap();
        let deleted:bool=sqlx::query_scalar("SELECT deleted FROM instance_recovery_publications WHERE instance_id='one'").fetch_one(&pool).await.unwrap();
        assert!(deleted);
        publish(&dirs, &pool, "one").await.unwrap();
        assert!(read(root.path(), "one").await.unwrap().is_none());
    }
    #[tokio::test]
    async fn external_profiles_receive_no_recovery_writes() {
        let (root, dirs, pool) = fixture().await;
        let external = tempfile::tempdir().unwrap();
        instance(&dirs, &pool, "one").await;
        sqlx::query("UPDATE instances SET game_dir_override=? WHERE id='one'")
            .bind(external.path().to_string_lossy().as_ref())
            .execute(&pool)
            .await
            .unwrap();
        publish(&dirs, &pool, "one").await.unwrap();
        assert!(read(root.path(), "one").await.unwrap().is_some());
        assert!(!dirs.instances_dir().join("one/.axolotl").exists());
        assert!(std::fs::read_dir(external.path()).unwrap().next().is_none());
    }
    #[tokio::test]
    async fn relationship_owner_changes_dirty_both_instances_atomically() {
        let (_root, dirs, pool) = fixture().await;
        for id in ["one", "two"] {
            instance(&dirs, &pool, id).await;
            publish(&dirs, &pool, id).await.unwrap();
        }
        sqlx::query("INSERT INTO instance_files(id,instance_id,relative_path,file_name,enabled,sha1,size,missing,added_at,modified_at) VALUES('file','one','mods/local.jar','local.jar',1,'',0,0,0,0)").execute(&pool).await.unwrap();
        for id in ["one", "two"] {
            publish(&dirs, &pool, id).await.unwrap();
        }
        let before_one = manifest_rows::revision(&pool, "one").await.unwrap();
        let before_two = manifest_rows::revision(&pool, "two").await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        sqlx::query(
            "UPDATE instance_files SET instance_id='two' WHERE id='file'",
        )
        .execute(&mut *tx)
        .await
        .unwrap();
        tx.rollback().await.unwrap();
        assert_eq!(
            manifest_rows::revision(&pool, "one").await.unwrap(),
            before_one
        );
        assert_eq!(
            manifest_rows::revision(&pool, "two").await.unwrap(),
            before_two
        );
        sqlx::query(
            "UPDATE instance_files SET instance_id='two' WHERE id='file'",
        )
        .execute(&pool)
        .await
        .unwrap();
        assert!(
            manifest_rows::revision(&pool, "one").await.unwrap() > before_one
        );
        assert!(
            manifest_rows::revision(&pool, "two").await.unwrap() > before_two
        );
    }
    #[tokio::test]
    async fn backup_copies_reachable_shards_and_tombstone_without_new_database()
    {
        let (root, dirs, pool) = fixture().await;
        instance(&dirs, &pool, "one").await;
        publish(&dirs, &pool, "one").await.unwrap();
        let destination = tempfile::tempdir().unwrap();
        backup(root.path(), destination.path()).await.unwrap();
        assert_eq!(
            read(destination.path(), "one")
                .await
                .unwrap()
                .unwrap()
                .metadata
                .instance
                .id,
            "one"
        );
        assert!(!destination.path().join("databases").exists());
    }
    #[tokio::test]
    async fn unmaterialized_pack_projects_and_identity_evidence_are_recoverable()
     {
        let (root, dirs, pool) = fixture().await;
        instance(&dirs, &pool, "one").await;
        sqlx::query("INSERT INTO instance_pack_members(id,content_set_id,member_key,project_type,expected_relative_path,provider,provider_project_id,created_at,modified_at) VALUES('member','set:one','key','mod','mods/pending.jar','modrinth','abcdefgh',0,0)").execute(&pool).await.unwrap();
        // The resource bootstrap may have already created this identity.
        let resource:String=sqlx::query_scalar("SELECT resource_id FROM resource_source_projects WHERE source='modrinth' AND project_id='abcdefgh'").fetch_one(&pool).await.unwrap();
        sqlx::query("INSERT INTO resource_identity_evidence VALUES('evidence',?,'{}',0)").bind(&resource).execute(&pool).await.unwrap();
        publish(&dirs, &pool, "one").await.unwrap();
        let snapshot = read(root.path(), "one").await.unwrap().unwrap();
        assert_eq!(
            snapshot.relationships["resource_source_projects"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            snapshot.relationships["resource_identity_evidence"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        let before = manifest_rows::revision(&pool, "one").await.unwrap();
        sqlx::query("UPDATE resource_identity_evidence SET confirmed_at=1 WHERE evidence_id='evidence'").execute(&pool).await.unwrap();
        assert!(manifest_rows::revision(&pool, "one").await.unwrap() > before);
    }
}
