//! A small head publishes immutable, individually bounded collection shards.
use crate::state::persistence::{Persistence, document, status::Availability};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::Path;
pub(crate) type Collections = BTreeMap<String, Map<String, Value>>;
const SHARD_LIMIT: usize = 1 << 20;
const MAX_SHARDS: usize = 4096;
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Shard {
    pub collection: String,
    pub file: String,
    pub bytes: usize,
    pub sha256: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Native {
    pub file: String,
    pub bytes: usize,
    pub sha256: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Head {
    pub revision: u64,
    pub transaction: String,
    pub shards: Vec<Shard>,
    #[serde(default)]
    pub native: BTreeMap<String, Native>,
    #[serde(default)]
    pub legacy_extensions_present: bool,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}
pub(crate) fn document(root: &Path) -> document::Document {
    Persistence::new(root)
        .document("state/head.json", "axolotl-sync-state", 1)
        .expect("constant sync head")
}
pub(crate) async fn load(root: &Path) -> crate::Result<(Head, Collections)> {
    let read = document(root).read().await?;
    if !matches!(
        read.availability,
        Availability::Ready | Availability::Recovered
    ) {
        return Err(crate::ErrorKind::FSError(
            "Shared sync state unavailable or newer; original files retained"
                .into(),
        )
        .into());
    }
    let raw = read.snapshot.expect("head");
    let data = raw.data().expect("head payload");
    if data
        .get("schemaVersion")
        .is_some_and(|value| value.as_u64() != Some(1))
        || data
            .get("schema_version")
            .is_some_and(|value| value.as_u64() != Some(1))
        || data.get("protocol").is_some()
    {
        return Err(crate::ErrorKind::InputError(
            "Unsupported sync head data retained".into(),
        )
        .into());
    }
    let head: Head = raw.decode()?;
    let collections = validate(root, &head).await?;
    Ok((head, collections))
}
/// Validate a staged generation before its head becomes visible.
pub(crate) async fn validate(
    root: &Path,
    head: &Head,
) -> crate::Result<Collections> {
    uuid::Uuid::parse_str(&head.transaction)?;
    if head.shards.len() > MAX_SHARDS {
        return Err(crate::ErrorKind::InputError(
            "Sync shard count exceeds limit".into(),
        )
        .into());
    }
    let mut collections = Collections::new();
    let mut expanded = 0usize;
    for shard in &head.shards {
        if shard.bytes > SHARD_LIMIT
            || !shard.file.starts_with("state/generations/")
            || !shard.file.ends_with(".json")
        {
            return Err(crate::ErrorKind::InputError(
                "Invalid sync shard".into(),
            )
            .into());
        }
        let path = crate::util::io::safe_relative_join(root, &shard.file)?;
        let base = tokio::fs::canonicalize(root).await?;
        let canonical = tokio::fs::canonicalize(&path).await?;
        if !canonical.starts_with(base) {
            return Err(crate::ErrorKind::InputError(
                "Sync shard escaped authority".into(),
            )
            .into());
        }
        let body = read_bounded(&path, SHARD_LIMIT).await?;
        if body.len() != shard.bytes
            || format!("{:x}", Sha256::digest(&body)) != shard.sha256
        {
            return Err(crate::ErrorKind::FSError(
                "Sync shard integrity failed".into(),
            )
            .into());
        }
        let mut entries: Map<String, Value> = serde_json::from_slice(&body)?;
        expanded = expanded
            .checked_add(body.len())
            .filter(|size| *size <= 128 << 20)
            .ok_or_else(|| {
                crate::ErrorKind::InputError(
                    "Sync state exceeds aggregate bound".into(),
                )
            })?;
        for value in entries.values_mut() {
            if shard.collection.starts_with("$legacy/") {
                // Raw import attachments are never typed runtime records.
                // Validate their blobs without expanding them into JSON bytes.
                validate_raw_blobs(root, value).await?;
            } else {
                expand_blobs(root, value, &mut expanded).await?;
            }
        }
        let target = collections.entry(shard.collection.clone()).or_default();
        for (key, value) in entries {
            if target.insert(key, value).is_some() {
                return Err(crate::ErrorKind::InputError(
                    "Duplicate sync identity".into(),
                )
                .into());
            }
        }
    }
    for native in head.native.values() {
        read_native_ref(root, native).await?;
    }
    Ok(collections)
}
async fn validate_raw_blobs(root: &Path, value: &Value) -> crate::Result<()> {
    if value.get("axolotl_blob").is_some() {
        let mut expanded = 0;
        let mut raw = value.clone();
        expand_blobs(root, &mut raw, &mut expanded).await?;
    } else if let Some(object) = value.as_object() {
        for child in object.values() {
            Box::pin(validate_raw_blobs(root, child)).await?;
        }
    } else if let Some(array) = value.as_array() {
        for child in array {
            Box::pin(validate_raw_blobs(root, child)).await?;
        }
    }
    Ok(())
}
pub(crate) async fn prepare(
    root: &Path,
    revision: u64,
    transaction: String,
    collections: &Collections,
    opaque: bool,
) -> crate::Result<Head> {
    let mut shards = Vec::new();
    for (collection, entries) in collections {
        let mut chunk = Map::new();
        let mut size = 2usize;
        for (key, original) in entries {
            let mut value = original.clone();
            externalize_blobs(root, &mut value).await?;
            let length = serde_json::to_vec(&(key, &value))?.len();
            if length > SHARD_LIMIT / 2 {
                return Err(crate::ErrorKind::InputError(
                    "Sync record exceeds bounded shard limit".into(),
                )
                .into());
            }
            if size + length > SHARD_LIMIT && !chunk.is_empty() {
                write_chunk(
                    root,
                    &transaction,
                    collection,
                    &mut shards,
                    &chunk,
                )
                .await?;
                chunk.clear();
                size = 2;
            }
            chunk.insert(key.clone(), value);
            size += length + 1;
        }
        if !chunk.is_empty() {
            write_chunk(root, &transaction, collection, &mut shards, &chunk)
                .await?;
        }
    }
    Ok(Head {
        revision,
        transaction: transaction.clone(),
        native: prepare_native(root, &transaction, collections).await?,
        shards,
        legacy_extensions_present: opaque,
        extra: Map::new(),
    })
}
async fn write_chunk(
    root: &Path,
    token: &str,
    collection: &str,
    shards: &mut Vec<Shard>,
    entries: &Map<String, Value>,
) -> crate::Result<()> {
    if shards.len() >= MAX_SHARDS {
        return Err(crate::ErrorKind::InputError(
            "Sync shard count exceeds limit".into(),
        )
        .into());
    }
    let body = serde_json::to_vec(entries)?;
    let file = format!("state/generations/{token}/{}.json", shards.len());
    document::write_json(&root.join(&file), body.clone()).await?;
    shards.push(Shard {
        collection: collection.into(),
        file,
        bytes: body.len(),
        sha256: format!("{:x}", Sha256::digest(body)),
        extra: Map::new(),
    });
    Ok(())
}
pub(crate) async fn publish(
    root: &Path,
    desired: Head,
    expected: u64,
) -> crate::Result<()> {
    validate(root, &desired).await?;
    document(root)
        .update(move |data| {
            if !data.is_empty() {
                let current: Head =
                    serde_json::from_value(Value::Object(data.clone()))?;
                if current.transaction == desired.transaction
                    && current.revision == desired.revision
                {
                    if serde_json::to_value(&current)?
                        == serde_json::to_value(&desired)?
                    {
                        return Ok(());
                    }
                    return Err(crate::ErrorKind::InputError(
                        "Sync decision identity reused with different payload"
                            .into(),
                    )
                    .into());
                }
                if !current.extra.is_empty()
                    || current
                        .shards
                        .iter()
                        .any(|shard| !shard.extra.is_empty())
                    || current
                        .native
                        .values()
                        .any(|native| !native.extra.is_empty())
                {
                    return Err(crate::ErrorKind::InputError(
                        "Newer sync generation metadata retained".into(),
                    )
                    .into());
                }
                if current.revision != expected {
                    return Err(crate::ErrorKind::InputError(
                        "Shared sync revision changed; staged intent retained"
                            .into(),
                    )
                    .into());
                }
            }
            for (key, value) in
                serde_json::to_value(desired)?.as_object().expect("head")
            {
                data.insert(key.clone(), value.clone());
            }
            Ok(())
        })
        .await?;
    Ok(())
}
pub(crate) async fn initial(
    root: &Path,
    collections: Collections,
    opaque: bool,
) -> crate::Result<Head> {
    let _lock = super::journal::initial_lock(root).await?;
    if document(root).read().await?.availability != Availability::Missing {
        return Ok(load(root).await?.0);
    }
    let token = uuid::Uuid::new_v4().to_string();
    let head = prepare(root, 0, token, &collections, opaque).await?;
    publish(root, head.clone(), 0).await?;
    Ok(head)
}

pub(crate) async fn read_bounded(
    path: &Path,
    limit: usize,
) -> crate::Result<Vec<u8>> {
    use tokio::io::AsyncReadExt;
    let file = tokio::fs::File::open(path).await?;
    if !file.metadata().await?.is_file()
        || file.metadata().await?.len() > limit as u64
    {
        return Err(crate::ErrorKind::InputError(
            "Sync object exceeds bound".into(),
        )
        .into());
    }
    let mut body = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut body).await?;
    if body.len() > limit {
        return Err(crate::ErrorKind::InputError(
            "Sync object grew beyond bound".into(),
        )
        .into());
    }
    Ok(body)
}
async fn externalize_blobs(
    root: &Path,
    value: &mut Value,
) -> crate::Result<()> {
    // Depth-first boxed recursion remains bounded by serde's input depth limit.
    if let Some(object) = value.as_object_mut() {
        for (name, child) in object.iter_mut() {
            if matches!(name.as_str(), "nbt" | "document" | "merge_base")
                && child.is_array()
            {
                let bytes: Vec<u8> = serde_json::from_value(child.clone())?;
                let digest = format!("{:x}", Sha256::digest(&bytes));
                let file = format!("state/objects/{digest}.bin");
                document::write_blob(&root.join(&file), bytes.clone()).await?;
                *child = json!({"axolotl_blob":{"file":file,"bytes":bytes.len(),"sha256":digest}});
            } else {
                Box::pin(externalize_blobs(root, child)).await?;
            }
        }
    } else if let Some(array) = value.as_array_mut() {
        for child in array {
            Box::pin(externalize_blobs(root, child)).await?;
        }
    }
    Ok(())
}
async fn expand_blobs(
    root: &Path,
    value: &mut Value,
    expanded: &mut usize,
) -> crate::Result<()> {
    if let Some(blob) = value
        .as_object()
        .filter(|object| object.len() == 1)
        .and_then(|object| object.get("axolotl_blob"))
    {
        let file =
            blob.get("file").and_then(Value::as_str).ok_or_else(|| {
                crate::ErrorKind::InputError("Invalid sync blob".into())
            })?;
        let size = blob
            .get("bytes")
            .and_then(Value::as_u64)
            .filter(|size| *size <= 16 << 20)
            .ok_or_else(|| {
                crate::ErrorKind::InputError("Invalid sync blob size".into())
            })?;
        let hash =
            blob.get("sha256").and_then(Value::as_str).ok_or_else(|| {
                crate::ErrorKind::InputError("Invalid sync blob hash".into())
            })?;
        // serde_json stores each integer in a Value. Account for that storage
        // before reading or expanding the referenced binary object.
        *expanded = expanded
            .checked_add(
                (size as usize).saturating_mul(std::mem::size_of::<Value>()),
            )
            .filter(|size| *size <= 128 << 20)
            .ok_or_else(|| {
                crate::ErrorKind::InputError(
                    "Sync state exceeds aggregate bound".into(),
                )
            })?;
        if file != format!("state/objects/{hash}.bin")
            || hash.len() != 64
            || !hash.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(crate::ErrorKind::InputError(
                "Invalid sync blob path".into(),
            )
            .into());
        }
        let path = crate::util::io::safe_relative_join(root, file)?;
        let canonical = tokio::fs::canonicalize(&path).await?;
        if !canonical.starts_with(tokio::fs::canonicalize(root).await?) {
            return Err(crate::ErrorKind::InputError(
                "Sync blob escaped authority".into(),
            )
            .into());
        }
        let bytes = read_bounded(&path, size as usize).await?;
        if bytes.len() != size as usize
            || format!("{:x}", Sha256::digest(&bytes)) != hash
        {
            return Err(crate::ErrorKind::FSError(
                "Sync blob integrity failed".into(),
            )
            .into());
        }
        *value = serde_json::to_value(bytes)?;
    } else if let Some(object) = value.as_object_mut() {
        for child in object.values_mut() {
            Box::pin(expand_blobs(root, child, expanded)).await?;
        }
    } else if let Some(array) = value.as_array_mut() {
        for child in array {
            Box::pin(expand_blobs(root, child, expanded)).await?;
        }
    }
    Ok(())
}

async fn prepare_native(
    root: &Path,
    token: &str,
    collections: &Collections,
) -> crate::Result<BTreeMap<String, Native>> {
    let mut output = BTreeMap::new();
    if let Some(servers) = collections.get("synced_servers") {
        if servers.values().any(|raw| {
            !super::supported(raw) || !super::supported(&raw["record"])
        }) {
            return Err(crate::ErrorKind::InputError("Shared servers contain newer records; native publication paused".into()).into());
        }
        let mut records = servers
            .values()
            .map(|raw| {
                let record = &raw["record"];
                Ok((
                    record.get("position").and_then(Value::as_i64).ok_or_else(
                        || {
                            crate::ErrorKind::InputError(
                                "Unreadable shared server position".into(),
                            )
                        },
                    )?,
                    serde_json::from_value::<Vec<u8>>(record["nbt"].clone())?,
                ))
            })
            .collect::<crate::Result<Vec<_>>>()?;
        records.sort_by_key(|(position, _)| *position);
        let previous = document(root).read().await?;
        let template = match previous.availability {
            Availability::Ready | Availability::Recovered => {
                let head: Head = previous.snapshot.expect("head").decode()?;
                match head.native.get("servers") {
                    Some(native) => {
                        let body = read_native_ref(root, native).await?;
                        Some(body)
                    }
                    None => None,
                }
            }
            Availability::Missing => {
                let path = root.join("servers.dat");
                if path.is_file() {
                    Some(read_bounded(&path, 16 << 20).await?)
                } else {
                    None
                }
            }
            _ => {
                return Err(crate::ErrorKind::FSError(
                    "Previous shared native source is unreadable; retained"
                        .into(),
                )
                .into());
            }
        };
        let (mut template, root_name) = match template {
            Some(bytes) => quartz_nbt::io::read_nbt(
                &mut std::io::Cursor::new(bytes),
                quartz_nbt::io::Flavor::Uncompressed,
            )?,
            None => (quartz_nbt::NbtCompound::new(), String::new()),
        };
        let mut list = quartz_nbt::NbtList::new();
        for (_, bytes) in records {
            let (server, _) = quartz_nbt::io::read_nbt(
                &mut std::io::Cursor::new(bytes),
                quartz_nbt::io::Flavor::Uncompressed,
            )?;
            list.push(server);
        }
        template.insert("servers", list);
        let mut bytes = Vec::new();
        quartz_nbt::io::write_nbt(
            &mut bytes,
            Some(&root_name),
            &template,
            quartz_nbt::io::Flavor::Uncompressed,
        )?;
        write_native(root, token, "servers", "servers.dat", bytes, &mut output)
            .await?;
    }
    if let Some(hotbars) = collections.get("synced_hotbar_state") {
        if hotbars.values().any(|raw| {
            !super::supported(raw)
                || !matches!(
                    raw["record"]["schema_version"].as_u64(),
                    Some(1 | 2)
                )
        }) {
            return Err(crate::ErrorKind::InputError(
                "Unknown hotbar source retained".into(),
            )
            .into());
        }
        if let Some(raw) = hotbars.values().find(|raw| super::supported(raw)) {
            let bytes: Vec<u8> =
                serde_json::from_value(raw["record"]["nbt"].clone())?;
            write_native(
                root,
                token,
                "hotbars",
                "hotbars.nbt",
                bytes,
                &mut output,
            )
            .await?;
        }
    }
    if let Some(raw) = collections
        .get("native_text")
        .and_then(|entries| entries.get("command_history"))
    {
        if !super::supported(raw) {
            return Err(crate::ErrorKind::InputError(
                "Unknown native text source retained".into(),
            )
            .into());
        }
        write_native(
            root,
            token,
            "command_history",
            "command_history.txt",
            serde_json::from_value(raw["record"]["document"].clone())?,
            &mut output,
        )
        .await?;
    }
    Ok(output)
}
async fn write_native(
    root: &Path,
    token: &str,
    name: &str,
    filename: &str,
    bytes: Vec<u8>,
    output: &mut BTreeMap<String, Native>,
) -> crate::Result<()> {
    let file = format!("state/generations/{token}/native/{filename}");
    let native = Native {
        file: file.clone(),
        bytes: bytes.len(),
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        extra: Map::new(),
    };
    document::write_blob(&root.join(file), bytes).await?;
    output.insert(name.into(), native);
    Ok(())
}
pub(crate) async fn native(
    root: &Path,
    name: &str,
) -> crate::Result<Option<Vec<u8>>> {
    let (head, _) = load(root).await?;
    let Some(native) = head.native.get(name) else {
        return Ok(None);
    };
    Ok(Some(read_native_ref(root, native).await?))
}
pub(crate) async fn read_native_ref(
    root: &Path,
    native: &Native,
) -> crate::Result<Vec<u8>> {
    if native.bytes > 16 << 20 || !native.file.starts_with("state/generations/")
    {
        return Err(crate::ErrorKind::InputError(
            "Invalid native sync reference".into(),
        )
        .into());
    }
    let path = crate::util::io::safe_relative_join(root, &native.file)?;
    let canonical = tokio::fs::canonicalize(&path).await?;
    if !canonical.starts_with(tokio::fs::canonicalize(root).await?) {
        return Err(crate::ErrorKind::InputError(
            "Native sync reference escaped authority".into(),
        )
        .into());
    }
    let body = read_bounded(&path, native.bytes).await?;
    if body.len() != native.bytes
        || format!("{:x}", Sha256::digest(&body)) != native.sha256
    {
        return Err(crate::ErrorKind::FSError(
            "Native sync integrity failed".into(),
        )
        .into());
    }
    Ok(body)
}

/// Snapshot the captured head and only its reachable immutable objects. The
/// caller owns the same transaction lock as publication, so no live-file copy.
pub(crate) async fn backup(
    root: &Path,
    destination: &Path,
    committed: Option<&str>,
) -> crate::Result<()> {
    let (head, _) = load(root).await?;
    let head_path = root.join("state/head.json");
    let head_bytes = read_bounded(&head_path, 16 << 20).await?;
    let mut files = std::collections::HashSet::new();
    for shard in &head.shards {
        let path = crate::util::io::safe_relative_join(root, &shard.file)?;
        let body = read_bounded(&path, SHARD_LIMIT).await?;
        let raw: Value = serde_json::from_slice(&body)?;
        collect_blob_paths(&raw, &mut files)?;
        document::write_json(&destination.join(&shard.file), body).await?;
    }
    for native in head.native.values() {
        let body = read_native_ref(root, native).await?;
        document::write_blob(&destination.join(&native.file), body).await?;
    }
    for relative in files {
        let path = crate::util::io::safe_relative_join(root, &relative)?;
        let canonical = tokio::fs::canonicalize(&path).await?;
        if !canonical.starts_with(tokio::fs::canonicalize(root).await?) {
            return Err(crate::ErrorKind::InputError(
                "Sync backup object escaped authority".into(),
            )
            .into());
        }
        let body = read_bounded(&path, 16 << 20).await?;
        document::write_blob(&destination.join(relative), body).await?;
    }
    if let Some(token) = committed {
        uuid::Uuid::parse_str(token)?;
        let relative = format!("state/journals/{token}.json");
        let bytes = read_bounded(&root.join(&relative), 1 << 20).await?;
        document::write_json(&destination.join(relative), bytes).await?;
    }
    let pending = root.join("pending-changes.json");
    if pending.is_file() {
        let bytes = read_bounded(&pending, 4 << 20).await?;
        let _: Value = serde_json::from_slice(&bytes)?;
        document::write_json(&destination.join("pending-changes.json"), bytes)
            .await?;
    }
    document::write_json(&destination.join("state/head.json"), head_bytes)
        .await?;
    Ok(())
}
fn collect_blob_paths(
    value: &Value,
    files: &mut std::collections::HashSet<String>,
) -> crate::Result<()> {
    if let Some(blob) = value.get("axolotl_blob") {
        let file = blob
            .get("file")
            .and_then(Value::as_str)
            .filter(|file| {
                file.starts_with("state/objects/") && file.ends_with(".bin")
            })
            .ok_or_else(|| {
                crate::ErrorKind::InputError(
                    "Unknown sync blob reference".into(),
                )
            })?;
        files.insert(file.into());
    }
    if let Some(object) = value.as_object() {
        for child in object.values() {
            collect_blob_paths(child, files)?;
        }
    }
    if let Some(array) = value.as_array() {
        for child in array {
            collect_blob_paths(child, files)?;
        }
    }
    Ok(())
}
