use super::paths::*;
use cap_std::fs::Dir;
use launcher_tools::{Command, ToolError};
use quartz_nbt::{NbtCompound, NbtTag, io::Flavor};
use serde_json::{Value, json};
use std::io::{Cursor, Read};
const LIMIT: usize = 2 * 1024 * 1024;

pub(in super::super) fn validate_document(
    bytes: &[u8],
) -> Result<(), ToolError> {
    decode(bytes).map(|_| ())
}

// quartz_nbt allocates declared lengths and recurses while reading. Preflight
// the decompressed wire format before allowing its parser to allocate.
struct Wire<'a> {
    bytes: &'a [u8],
    at: usize,
    nodes: usize,
}
impl Wire<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], ToolError> {
        let end = self
            .at
            .checked_add(n)
            .filter(|end| *end <= self.bytes.len())
            .ok_or_else(|| error("invalid NBT length"))?;
        let slice = &self.bytes[self.at..end];
        self.at = end;
        Ok(slice)
    }
    fn byte(&mut self) -> Result<u8, ToolError> {
        Ok(self.take(1)?[0])
    }
    fn string(&mut self) -> Result<(), ToolError> {
        let n = u16::from_be_bytes(self.take(2)?.try_into().unwrap()) as usize;
        self.take(n)?;
        Ok(())
    }
    fn length(&mut self) -> Result<usize, ToolError> {
        let n = i32::from_be_bytes(self.take(4)?.try_into().unwrap());
        usize::try_from(n)
            .ok()
            .filter(|n| *n <= LIMIT)
            .ok_or_else(|| error("invalid NBT length"))
    }
    fn tag(&mut self, id: u8, depth: usize) -> Result<(), ToolError> {
        self.nodes += 1;
        if depth > 32 || self.nodes > 100000 {
            return Err(error("NBT complexity"));
        }
        match id {
            1 => {
                self.take(1)?;
            }
            2 => {
                self.take(2)?;
            }
            3 | 5 => {
                self.take(4)?;
            }
            4 | 6 => {
                self.take(8)?;
            }
            7 | 11 | 12 => {
                let n = self.length()?;
                let width = match id {
                    7 => 1,
                    11 => 4,
                    _ => 8,
                };
                self.take(n.checked_mul(width).ok_or_else(|| error("size"))?)?;
            }
            8 => self.string()?,
            9 => {
                let kind = self.byte()?;
                let n = self.length()?;
                if kind > 12 || (kind == 0 && n > 0) || n > 100000 {
                    return Err(error("NBT list"));
                }
                for _ in 0..n {
                    self.tag(kind, depth + 1)?;
                }
            }
            10 => loop {
                let kind = self.byte()?;
                if kind == 0 {
                    break;
                }
                self.string()?;
                self.tag(kind, depth + 1)?;
            },
            _ => return Err(error("NBT tag")),
        }
        Ok(())
    }
}
fn decode(
    bytes: &[u8],
) -> Result<(NbtTag, String, Flavor, &'static str), ToolError> {
    let (flavor, name, raw) = if bytes.starts_with(&[0x1f, 0x8b]) {
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(bytes)
            .take((LIMIT + 1) as u64)
            .read_to_end(&mut raw)
            .map_err(error)?;
        (Flavor::GzCompressed, "gzip", raw)
    } else if bytes.first() == Some(&0x78) {
        let mut raw = Vec::new();
        flate2::read::ZlibDecoder::new(bytes)
            .take((LIMIT + 1) as u64)
            .read_to_end(&mut raw)
            .map_err(error)?;
        (Flavor::ZlibCompressed, "zlib", raw)
    } else {
        (Flavor::Uncompressed, "raw", bytes.to_vec())
    };
    if raw.len() > LIMIT {
        return Err(error("NBT size"));
    }
    let mut wire = Wire {
        bytes: &raw,
        at: 0,
        nodes: 0,
    };
    if wire.byte()? != 10 {
        return Err(error("root"));
    }
    wire.string()?;
    wire.tag(10, 0)?;
    if wire.at != raw.len() {
        return Err(error("trailing NBT"));
    }
    let (compound, root) =
        quartz_nbt::io::read_nbt(&mut Cursor::new(raw), Flavor::Uncompressed)
            .map_err(error)?;
    Ok((NbtTag::Compound(compound), root, flavor, name))
}
fn node<'a>(
    root: &'a NbtTag,
    path: &[String],
) -> Result<&'a NbtTag, ToolError> {
    let mut cur = root;
    for key in path {
        cur = match cur {
            NbtTag::Compound(c) => c.inner().get(key),
            NbtTag::List(l) => {
                key.parse::<usize>().ok().and_then(|i| l.as_ref().get(i))
            }
            _ => None,
        }
        .ok_or_else(|| {
            ToolError::new("nbt_path_not_found", "NBT path segment not found")
        })?;
    }
    Ok(cur)
}
fn node_mut<'a>(
    root: &'a mut NbtTag,
    path: &[String],
) -> Result<&'a mut NbtTag, ToolError> {
    if path.is_empty() {
        return Ok(root);
    }
    match root {
        NbtTag::Compound(c) => node_mut(
            c.inner_mut()
                .get_mut(&path[0])
                .ok_or_else(|| error("path"))?,
            &path[1..],
        ),
        NbtTag::List(l) => node_mut(
            l.inner_mut()
                .get_mut(path[0].parse::<usize>().map_err(error)?)
                .ok_or_else(|| error("path"))?,
            &path[1..],
        ),
        _ => Err(error("path")),
    }
}
fn tag_type(t: &NbtTag) -> &'static str {
    match t {
        NbtTag::Byte(_) => "byte",
        NbtTag::Short(_) => "short",
        NbtTag::Int(_) => "int",
        NbtTag::Long(_) => "long",
        NbtTag::Float(_) => "float",
        NbtTag::Double(_) => "double",
        NbtTag::ByteArray(_) => "byte_array",
        NbtTag::String(_) => "string",
        NbtTag::List(_) => "list",
        NbtTag::Compound(_) => "compound",
        NbtTag::IntArray(_) => "int_array",
        NbtTag::LongArray(_) => "long_array",
    }
}
fn summary(t: &NbtTag) -> Value {
    let mut v = json!({"type":tag_type(t)});
    match t {
        NbtTag::Compound(c) => v["length"] = json!(c.inner().len()),
        NbtTag::List(l) => v["length"] = json!(l.len()),
        NbtTag::ByteArray(a) => v["length"] = json!(a.len()),
        NbtTag::IntArray(a) => v["length"] = json!(a.len()),
        NbtTag::LongArray(a) => v["length"] = json!(a.len()),
        NbtTag::String(s) => {
            v["value"] = json!(s.chars().take(4096).collect::<String>());
            v["truncated"] = json!(s.chars().count() > 4096);
        }
        NbtTag::Long(n) => v["value"] = json!(n.to_string()),
        _ => v["snbt"] = json!(t.to_snbt()),
    };
    v
}
pub(super) fn execute(
    root: &Dir,
    command: Command,
) -> Result<Value, ToolError> {
    match command {
        Command::NbtRead {
            path,
            tag_path,
            offset,
            limit,
            ..
        } => {
            let bytes = read(root, &path, LIMIT as u64)?;
            let (tag, root_name, _, compression) = decode(&bytes)?;
            let target = node(&tag, &tag_path.unwrap_or_default())?;
            let offset = offset.unwrap_or(0);
            let limit = limit.unwrap_or(50);
            let mut result = summary(target);
            result["sha256"] = json!(hash(&bytes));
            result["root_name"] = json!(root_name);
            result["compression"] = json!(compression);
            let values = match target {
                NbtTag::Compound(c) => {
                    let mut keys = c.inner().keys().collect::<Vec<_>>();
                    keys.sort();
                    keys.into_iter()
                        .map(|k| json!({"key":k,"node":summary(&c.inner()[k])}))
                        .collect::<Vec<_>>()
                }
                NbtTag::List(l) => l
                    .iter()
                    .enumerate()
                    .map(|(i, t)| json!({"index":i,"node":summary(t)}))
                    .collect(),
                NbtTag::ByteArray(a) => a
                    .iter()
                    .enumerate()
                    .skip(offset)
                    .take(limit)
                    .map(|(i, v)| json!({"index":i,"value":v}))
                    .collect(),
                NbtTag::IntArray(a) => a
                    .iter()
                    .enumerate()
                    .skip(offset)
                    .take(limit)
                    .map(|(i, v)| json!({"index":i,"value":v}))
                    .collect(),
                NbtTag::LongArray(a) => a
                    .iter()
                    .enumerate()
                    .skip(offset)
                    .take(limit)
                    .map(|(i, v)| json!({"index":i,"value":v.to_string()}))
                    .collect(),
                _ => vec![],
            };
            let array = matches!(
                target,
                NbtTag::ByteArray(_)
                    | NbtTag::IntArray(_)
                    | NbtTag::LongArray(_)
            );
            let total = result["length"].as_u64().unwrap_or(values.len() as u64)
                as usize;
            let children = if array {
                values
            } else {
                values.into_iter().skip(offset).take(limit).collect()
            };
            let end = offset.saturating_add(children.len());
            result["children"] = json!(children);
            result["next_offset"] = json!((end < total).then_some(end));
            Ok(result)
        }
        Command::NbtPatch {
            path,
            tag_path,
            value_snbt,
            remove,
            expected_sha256,
            ..
        } => {
            let bytes = read(root, &path, LIMIT as u64)?;
            check(&bytes, &expected_sha256)?;
            let (mut tag, name, flavor, _) = decode(&bytes)?;
            if tag_path.is_empty() {
                return Err(error("root patch"));
            }
            let replacement = if remove {
                if value_snbt.is_some() {
                    return Err(error("remove with value"));
                }
                None
            } else {
                let text = value_snbt.ok_or_else(|| error("missing value"))?;
                if text.matches(['{', '[']).count() > 32 {
                    return Err(error("SNBT depth"));
                }
                let compound =
                    NbtCompound::from_snbt(&format!("{{value:{text}}}"))
                        .map_err(error)?;
                if compound.inner().len() != 1 {
                    return Err(error("SNBT value"));
                }
                Some(
                    compound
                        .inner()
                        .get("value")
                        .ok_or_else(|| error("SNBT value"))?
                        .clone(),
                )
            };
            let parent = node_mut(&mut tag, &tag_path[..tag_path.len() - 1])?;
            let key = &tag_path[tag_path.len() - 1];
            match parent {
                NbtTag::Compound(c) => {
                    if let Some(value) = replacement {
                        if c.inner().get(key).is_some_and(|old| {
                            tag_type(old) != tag_type(&value)
                        }) {
                            return Err(error("type change"));
                        }
                        c.insert(key.clone(), value);
                    } else if c.inner_mut().remove(key).is_none() {
                        return Err(error("missing key"));
                    }
                }
                NbtTag::List(l) => {
                    let i = key.parse::<usize>().map_err(error)?;
                    let old =
                        l.as_ref().get(i).ok_or_else(|| error("index"))?;
                    if let Some(value) = replacement {
                        if tag_type(old) != tag_type(&value) {
                            return Err(error("list type"));
                        }
                        l.inner_mut()[i] = value;
                    } else {
                        l.inner_mut().remove(i);
                    }
                }
                _ => return Err(error("parent")),
            };
            let NbtTag::Compound(compound) = tag else {
                unreachable!()
            };
            let mut output = Vec::new();
            quartz_nbt::io::write_nbt(
                &mut output,
                Some(&name),
                &compound,
                flavor,
            )
            .map_err(error)?;
            if output.len() > LIMIT {
                return Err(error("output size"));
            }
            decode(&output)?;
            write(root, &path, &output, Some(&expected_sha256), false)?;
            Ok(json!({"path":path,"sha256":hash(&output),"bytes":output.len()}))
        }
        _ => Err(error("command")),
    }
}
