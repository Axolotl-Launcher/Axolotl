use std::borrow::Cow;
use std::fmt;
use std::path::{Path, PathBuf};

use encoding_rs::{UTF_16BE, UTF_16LE};
use serde::de::{
    self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor,
};
use serde_json::{Map, Number, Value};

pub(crate) fn discover(version_dir: &Path) -> crate::Result<PathBuf> {
    let folder_name = version_dir.file_name().ok_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "PCL-CE version directory has no name: {}",
            version_dir.display()
        ))
    })?;
    let mut filename = folder_name.to_os_string();
    filename.push(".json");
    let conventional = version_dir.join(filename);
    if conventional.is_file() {
        return Ok(conventional);
    }

    let entries = std::fs::read_dir(version_dir).map_err(|error| {
        crate::ErrorKind::FSError(format!(
            "Failed to read PCL-CE version directory {}: {error}",
            version_dir.display()
        ))
    })?;
    let mut candidates = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
                && path.is_file()
        });
    match (candidates.next(), candidates.next()) {
        (Some(path), None) => Ok(path),
        (None, _) => Err(crate::ErrorKind::InputError(format!(
            "No version JSON found in {}",
            version_dir.display()
        ))
        .into()),
        (Some(_), Some(_)) => Err(crate::ErrorKind::InputError(format!(
            "Multiple version JSON files found in {}; expected {} or one unique fallback",
            version_dir.display(),
            conventional.display()
        ))
        .into()),
    }
}

pub(crate) fn read(path: &Path) -> crate::Result<Value> {
    let bytes = std::fs::read(path).map_err(|error| {
        crate::ErrorKind::FSError(format!(
            "Failed to read PCL-CE version JSON {}: {error}",
            path.display()
        ))
    })?;
    let mut document = parse_raw(path, &bytes)?;
    canonicalize_document(&mut document).map_err(|key| {
        invalid_document(path, &format!("multiple spellings of `{key}`"))
    })?;
    Ok(document)
}

pub(super) fn parse_raw(path: &Path, bytes: &[u8]) -> crate::Result<Value> {
    let text = decode(bytes)
        .ok_or_else(|| invalid_document(path, "invalid text encoding"))?;
    let UniqueValue(document) =
        serde_json_lenient::from_str(&text).map_err(|error| {
            let detail = if error.to_string().starts_with(DUPLICATE_PROPERTY) {
                DUPLICATE_PROPERTY
            } else {
                "invalid JSON"
            };
            invalid_document(
                path,
                &format!(
                    "{detail} (line {}, column {})",
                    error.line(),
                    error.column()
                ),
            )
        })?;
    if !document.is_object() {
        return Err(invalid_document(path, "expected a JSON object"));
    }
    Ok(document)
}

const DUPLICATE_PROPERTY: &str = "duplicate JSON property";

struct UniqueValue(Value);

impl<'de> Deserialize<'de> for UniqueValue {
    fn deserialize<D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Self, D::Error> {
        deserializer.deserialize_any(UniqueValueVisitor).map(Self)
    }
}

struct UniqueValueVisitor;

impl<'de> Visitor<'de> for UniqueValueVisitor {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("JSON without duplicate object properties")
    }

    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Value, E> {
        Ok(Value::from(value))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Value, E> {
        Ok(Value::from(value))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Value, E> {
        Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("non-finite JSON number"))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Value, E> {
        Ok(Value::from(value))
    }

    fn visit_string<E: de::Error>(self, value: String) -> Result<Value, E> {
        Ok(Value::String(value))
    }

    fn visit_seq<A: SeqAccess<'de>>(
        self,
        mut entries: A,
    ) -> Result<Value, A::Error> {
        let mut values = Vec::new();
        while let Some(UniqueValue(value)) = entries.next_element()? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }

    fn visit_map<A: MapAccess<'de>>(
        self,
        mut entries: A,
    ) -> Result<Value, A::Error> {
        let mut object = Map::new();
        while let Some(key) = entries.next_key::<String>()? {
            if object.contains_key(&key) {
                return Err(de::Error::custom(DUPLICATE_PROPERTY));
            }
            let UniqueValue(value) = entries.next_value()?;
            object.insert(key, value);
        }
        Ok(Value::Object(object))
    }
}

fn decode(bytes: &[u8]) -> Option<Cow<'_, str>> {
    match bytes {
        [0x00, 0x00, 0xfe, 0xff, tail @ ..] => {
            decode_utf32(tail, u32::from_be_bytes).map(Cow::Owned)
        }
        [0xff, 0xfe, 0x00, 0x00, tail @ ..] => {
            decode_utf32(tail, u32::from_le_bytes).map(Cow::Owned)
        }
        [0xfe, 0xff, tail @ ..] => {
            UTF_16BE.decode_without_bom_handling_and_without_replacement(tail)
        }
        [0xff, 0xfe, tail @ ..] => {
            UTF_16LE.decode_without_bom_handling_and_without_replacement(tail)
        }
        _ => std::str::from_utf8(
            bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes),
        )
        .ok()
        .map(Cow::Borrowed),
    }
}

fn decode_utf32(bytes: &[u8], decode: fn([u8; 4]) -> u32) -> Option<String> {
    let (words, remainder) = bytes.as_chunks::<4>();
    if !remainder.is_empty() {
        return None;
    }
    words
        .iter()
        .map(|word| char::from_u32(decode(*word)))
        .collect()
}

fn invalid_document(path: &Path, detail: &str) -> crate::Error {
    crate::ErrorKind::JSONError(serde::de::Error::custom(format!(
        "Failed to parse PCL-CE version JSON {}: {detail}",
        path.display()
    )))
    .into()
}

type KeyResult = Result<(), &'static str>;

fn canonicalize_fields(value: &mut Value, keys: &[&'static str]) -> KeyResult {
    let Some(object) = value.as_object_mut() else {
        return Ok(());
    };
    let renamed = object
        .keys()
        .filter_map(|key| {
            keys.iter()
                .copied()
                .find(|known| *known != key && known.eq_ignore_ascii_case(key))
                .map(|known| (key.clone(), known))
        })
        .collect::<Vec<_>>();
    for (original, known) in renamed {
        if object.contains_key(known) {
            return Err(known);
        }
        if let Some(value) = object.remove(&original) {
            object.insert(known.to_string(), value);
        }
    }
    Ok(())
}

fn canonicalize_document(document: &mut Value) -> KeyResult {
    canonicalize_fields(
        document,
        &[
            "arguments",
            "assetIndex",
            "assets",
            "clientVersion",
            "complianceLevel",
            "downloads",
            "id",
            "inheritsFrom",
            "jar",
            "javaVersion",
            "labymod_data",
            "libraries",
            "logging",
            "mainClass",
            "minecraftArguments",
            "minimumLauncherVersion",
            "patches",
            "priority",
            "releaseTime",
            "root",
            "time",
            "type",
            "version",
        ],
    )?;
    if let Some(patches) =
        document.get_mut("patches").and_then(Value::as_array_mut)
    {
        for patch in patches {
            canonicalize_document(patch)?;
        }
    }
    for (member, fields) in [
        (
            "assetIndex",
            &["id", "sha1", "size", "totalSize", "url"][..],
        ),
        ("javaVersion", &["component", "majorVersion"][..]),
        ("labymod_data", &["version"][..]),
    ] {
        if let Some(value) = document.get_mut(member) {
            canonicalize_fields(value, fields)?;
        }
    }
    if let Some(arguments) = document.get_mut("arguments") {
        canonicalize_fields(arguments, &["game", "jvm"])?;
        for group in ["game", "jvm"] {
            if let Some(arguments) =
                arguments.get_mut(group).and_then(Value::as_array_mut)
            {
                for argument in arguments {
                    canonicalize_fields(argument, &["rules", "value"])?;
                    canonicalize_rules(argument)?;
                }
            }
        }
    }
    if let Some(downloads) = document.get_mut("downloads") {
        canonicalize_fields(
            downloads,
            &[
                "client",
                "client_mappings",
                "server",
                "server_mappings",
                "windows_server",
            ],
        )?;
        if let Some(downloads) = downloads.as_object_mut() {
            for download in downloads.values_mut() {
                canonicalize_fields(download, &["sha1", "size", "url"])?;
            }
        }
    }
    if let Some(logging) = document.get_mut("logging") {
        canonicalize_fields(logging, &["client"])?;
        if let Some(client) = logging.get_mut("client") {
            canonicalize_fields(client, &["argument", "file", "type"])?;
            if let Some(file) = client.get_mut("file") {
                canonicalize_fields(file, &["id", "sha1", "size", "url"])?;
            }
        }
    }
    if let Some(libraries) =
        document.get_mut("libraries").and_then(Value::as_array_mut)
    {
        for library in libraries {
            canonicalize_library(library)?;
        }
    }
    Ok(())
}

fn canonicalize_rules(container: &mut Value) -> KeyResult {
    if let Some(rules) =
        container.get_mut("rules").and_then(Value::as_array_mut)
    {
        for rule in rules {
            canonicalize_fields(rule, &["action", "features", "os"])?;
            if let Some(os) = rule.get_mut("os") {
                canonicalize_fields(os, &["arch", "name", "version"])?;
            }
        }
    }
    Ok(())
}

fn canonicalize_library(library: &mut Value) -> KeyResult {
    canonicalize_fields(
        library,
        &[
            "checksums",
            "downloadable",
            "downloads",
            "extract",
            "filename",
            "hint",
            "include_in_classpath",
            "MMC-filename",
            "MMC-hint",
            "name",
            "natives",
            "rules",
            "url",
        ],
    )?;
    canonicalize_rules(library)?;
    if let Some(extract) = library.get_mut("extract") {
        canonicalize_fields(extract, &["exclude"])?;
    }
    if let Some(natives) = library.get_mut("natives") {
        canonicalize_fields(
            natives,
            &[
                "freebsd",
                "linux",
                "linux-arm32",
                "linux-arm64",
                "linux-ppc64le",
                "osx",
                "osx-arm64",
                "universal",
                "unknown",
                "windows",
                "windows-arm64",
            ],
        )?;
    }
    if let Some(downloads) = library.get_mut("downloads") {
        const DOWNLOAD_FIELDS: &[&str] = &["path", "sha1", "size", "url"];
        canonicalize_fields(downloads, &["artifact", "classifiers"])?;
        if let Some(artifact) = downloads.get_mut("artifact") {
            canonicalize_fields(artifact, DOWNLOAD_FIELDS)?;
        }
        if let Some(classifiers) = downloads
            .get_mut("classifiers")
            .and_then(Value::as_object_mut)
        {
            for download in classifiers.values_mut() {
                canonicalize_fields(download, DOWNLOAD_FIELDS)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    #[test]
    fn discovery_prefers_same_name_before_validating_the_document() {
        let dir = tempfile::tempdir().unwrap();
        let version_dir = dir.path().join("demo");
        fs::create_dir(&version_dir).unwrap();
        let conventional = version_dir.join("demo.json");
        let fallback = version_dir.join("alternative.JSON");
        fs::write(&conventional, "invalid").unwrap();
        fs::write(&fallback, "{}").unwrap();

        let selected = discover(&version_dir).unwrap();
        assert_eq!(selected, conventional);
        assert!(read(&selected).is_err());

        fs::remove_file(conventional).unwrap();
        fs::create_dir(version_dir.join("directory.json")).unwrap();
        assert_eq!(discover(&version_dir).unwrap(), fallback);
        fs::write(version_dir.join("another.json"), "{}").unwrap();
        assert!(
            discover(&version_dir)
                .unwrap_err()
                .to_string()
                .contains("Multiple version JSON files")
        );
    }

    #[test]
    fn reads_comments_trailing_commas_and_known_keys_without_folding_opaque_keys()
     {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("demo.json");
        fs::write(&path, r#"{
            // comments are syntax, not part of the data
            "ID": "1.21.1",
            "mainCLASS": "sample.Main",
            "ARGUMENTS": { "GAME": [{"RULES": [{"OS": {"NAME": "windows"}, "FEATURES": {"QUICK_PLAY": false}, "ACTION": "allow"}], "VALUE": ["https://example.com/a/*b*/",],}], },
            "PATCHES": [{"ID":"game", "VERSION":"1.21.1"}],
            "LIBRARIES": [{"DOWNLOADS": {"CLASSIFIERS": {"Native-X": {"URL": "https://example.com/"}}}}],
            "plugin": {"ID": "opaque"},
        }"#).unwrap();
        let document = read(&path).unwrap();
        assert_eq!(document["id"], "1.21.1");
        assert_eq!(document["mainClass"], "sample.Main");
        let argument = &document["arguments"]["game"][0];
        assert_eq!(
            argument["rules"][0],
            json!({"os":{"name":"windows"}, "features":{"QUICK_PLAY":false}, "action":"allow"})
        );
        assert_eq!(argument["value"][0], "https://example.com/a/*b*/");
        assert_eq!(document["patches"][0]["version"], "1.21.1");
        assert_eq!(
            document["libraries"][0]["downloads"]["classifiers"]["Native-X"]["url"],
            "https://example.com/"
        );
        assert_eq!(document["plugin"], json!({"ID":"opaque"}));
    }

    #[test]
    fn malformed_or_ambiguous_documents_do_not_leak_values() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("demo.json");
        for text in [
            r#"{id: "private-value"}"#,
            r#"{"id": 'private-value'}"#,
            r#"{"id": "private-value" "time": 0}"#,
            r#"{"id": "private-value", "time": 01}"#,
            r#"{"id": "private-value\x21"}"#,
            r#"{"id": "private-value"} []"#,
            r#"{"id": "private-value", "ID": "other"}"#,
            r#"{"ID": "private-value", "Id": "other"}"#,
            r#"["private-value"]"#,
        ] {
            fs::write(&path, text).unwrap();
            let message = read(&path).unwrap_err().to_string();
            assert!(message.contains("demo.json"), "{message}");
            assert!(!message.contains("private-value"), "{message}");
        }
    }

    #[test]
    fn exact_duplicate_properties_are_rejected_before_values_can_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("demo.json");
        for text in [
            r#"{"id":"private-first","id":"private-second"}"#,
            r#"{"arguments":{"game":[{"value":"private-first","\u0076alue":"private-second"}]}}"#,
        ] {
            fs::write(&path, text).unwrap();
            let message = read(&path).unwrap_err().to_string();
            assert!(message.contains("duplicate JSON property"), "{message}");
            assert!(!message.contains("private-"), "{message}");
        }
    }

    #[test]
    fn supports_bom_marked_text_and_rejects_invalid_encoding() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("demo.json");
        let text = r#"{"id":"示例"}"#;
        let mut bytes = b"\xef\xbb\xbf".to_vec();
        bytes.extend_from_slice(text.as_bytes());
        fs::write(&path, bytes).unwrap();
        assert_eq!(read(&path).unwrap()["id"], "示例");

        let bytes = [0xff, 0xfe]
            .into_iter()
            .chain(text.encode_utf16().flat_map(u16::to_le_bytes))
            .collect::<Vec<_>>();
        fs::write(&path, bytes).unwrap();
        assert_eq!(read(&path).unwrap()["id"], "示例");

        let bytes = [0x00, 0x00, 0xfe, 0xff]
            .into_iter()
            .chain(
                text.chars()
                    .flat_map(|character| u32::from(character).to_be_bytes()),
            )
            .collect::<Vec<_>>();
        fs::write(&path, bytes).unwrap();
        assert_eq!(read(&path).unwrap()["id"], "示例");

        fs::write(&path, [0xff, 0xfe, 0x7b]).unwrap();
        assert!(
            read(&path)
                .unwrap_err()
                .to_string()
                .contains("invalid text encoding")
        );
    }
}
