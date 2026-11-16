use super::super::synced_options::{nbt_from_bytes, nbt_to_bytes};
use super::limits::{MAX_SERVER_BYTES, validate_size};
use crate::ErrorKind;
use quartz_nbt::{NbtCompound, NbtList, NbtTag};
use std::path::Path;
use tokio::io::AsyncReadExt;

pub(crate) async fn read_servers(
    path: &Path,
) -> crate::Result<Vec<NbtCompound>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    servers_from_bytes(read_server_bytes(path).await?)
}

pub(super) async fn read_server_bytes(path: &Path) -> crate::Result<Vec<u8>> {
    let file = tokio::fs::File::open(path).await?;
    validate_size(
        0,
        file.metadata()
            .await?
            .len()
            .try_into()
            .unwrap_or(usize::MAX),
    )?;
    let mut bytes = Vec::new();
    file.take(MAX_SERVER_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .await?;
    validate_size(0, bytes.len())?;
    Ok(bytes)
}

pub(super) fn servers_from_bytes(
    bytes: Vec<u8>,
) -> crate::Result<Vec<NbtCompound>> {
    validate_size(0, bytes.len())?;
    servers_from_root(nbt_from_bytes(bytes)?)
}

fn servers_from_root(root: NbtCompound) -> crate::Result<Vec<NbtCompound>> {
    let list = root.get::<_, &NbtList>("servers").map_err(|_| {
        ErrorKind::InputError(
            "servers.dat does not contain a valid servers list".to_string(),
        )
    })?;
    validate_size(list.len(), 0)?;
    list.iter()
        .map(|tag| match tag {
            NbtTag::Compound(compound) => Ok(compound.clone()),
            _ => Err(ErrorKind::InputError(
                "servers.dat contains an invalid server entry".to_string(),
            )
            .into()),
        })
        .collect()
}

pub(crate) async fn write_servers(
    path: &Path,
    servers: &[NbtCompound],
) -> crate::Result<()> {
    if let Some(parent) = path.parent() {
        crate::util::io::create_dir_all(parent).await?;
    }
    let template = match read_server_bytes(path).await {
        Ok(bytes) => Some(bytes),
        Err(error) if matches!(error.raw.as_ref(),crate::ErrorKind::StdIOError(error) if error.kind()==std::io::ErrorKind::NotFound) => {
            None
        }
        Err(error) => return Err(error),
    };
    let bytes = servers_to_bytes_with_template(servers, template.as_deref())?;
    super::super::synced_options::publish_detached_copy(path, bytes).await?;
    Ok(())
}

pub(super) fn servers_to_bytes(
    servers: &[NbtCompound],
) -> crate::Result<Vec<u8>> {
    servers_to_bytes_with_template(servers, None)
}

pub(super) fn servers_to_bytes_with_root(
    servers: &[NbtCompound],
    root: NbtCompound,
) -> crate::Result<Vec<u8>> {
    let template = nbt_to_bytes(&root)?;
    servers_to_bytes_with_template(servers, Some(&template))
}

/// Retain the complete native document and change only its server list.
/// Templates are bytes so the root name survives too.
pub(super) fn servers_to_bytes_with_template(
    servers: &[NbtCompound],
    template: Option<&[u8]>,
) -> crate::Result<Vec<u8>> {
    validate_size(servers.len(), 0)?;
    let (mut root, name) = match template {
        Some(bytes) => {
            validate_size(0, bytes.len())?;
            quartz_nbt::io::read_nbt(
                &mut std::io::Cursor::new(bytes),
                quartz_nbt::io::Flavor::Uncompressed,
            )?
        }
        None => (NbtCompound::new(), String::new()),
    };
    let mut size = 0;
    let mut list = NbtList::new();
    for server in servers {
        size += nbt_to_bytes(server)?.len();
        validate_size(servers.len(), size)?;
        list.push(server.clone());
    }
    root.insert("servers", list);
    let mut bytes = Vec::new();
    quartz_nbt::io::write_nbt(
        &mut bytes,
        Some(&name),
        &root,
        quartz_nbt::io::Flavor::Uncompressed,
    )?;
    validate_size(servers.len(), bytes.len())?;
    Ok(bytes)
}

pub(crate) fn server_data(
    name: String,
    address: String,
    accept_textures: Option<bool>,
) -> NbtCompound {
    let mut server = NbtCompound::new();
    server.insert("name", name);
    server.insert("ip", address);
    if let Some(accept_textures) = accept_textures {
        server.insert("acceptTextures", i8::from(accept_textures));
    }
    server.insert("hidden", 0_i8);
    server
}

pub(super) fn update_server_data(
    server: &mut NbtCompound,
    name: String,
    address: String,
    accept_textures: Option<bool>,
) {
    server.insert("name", name);
    server.insert("ip", address);
    match accept_textures {
        Some(value) => server.insert("acceptTextures", i8::from(value)),
        None => {
            server.inner_mut().remove("acceptTextures");
        }
    }
}

pub(super) fn server_hidden(server: &NbtCompound) -> bool {
    server.get::<_, i8>("hidden").unwrap_or(0) != 0
}

pub(super) fn server_address(server: &NbtCompound) -> String {
    server.get::<_, &str>("ip").unwrap_or_default().to_string()
}

pub(super) fn server_identity_address(server: &NbtCompound) -> String {
    server_address(server).trim().to_ascii_lowercase()
}

pub(super) fn server_identity_name(server: &NbtCompound) -> String {
    server
        .get::<_, &str>("name")
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
}

#[cfg(test)]
mod preservation_tests {
    use super::*;
    #[test]
    fn replacing_server_list_keeps_unknown_root_and_entry_fields_and_root_name()
    {
        let mut root = NbtCompound::new();
        root.insert("future_root", 42_i32);
        root.insert("servers", NbtList::new());
        let mut template = Vec::new();
        quartz_nbt::io::write_nbt(
            &mut template,
            Some("NamedRoot"),
            &root,
            quartz_nbt::io::Flavor::Uncompressed,
        )
        .unwrap();
        let mut server =
            server_data("A".into(), "example.invalid".into(), None);
        server.insert("future_server", "preserve");
        let bytes =
            servers_to_bytes_with_template(&[server], Some(&template)).unwrap();
        let (root, name) = quartz_nbt::io::read_nbt(
            &mut std::io::Cursor::new(bytes),
            quartz_nbt::io::Flavor::Uncompressed,
        )
        .unwrap();
        assert_eq!(name, "NamedRoot");
        assert_eq!(root.get::<_, i32>("future_root").unwrap(), 42);
        let entries = servers_from_root(root).unwrap();
        assert_eq!(
            entries[0].get::<_, &str>("future_server").unwrap(),
            "preserve"
        );
    }
}
