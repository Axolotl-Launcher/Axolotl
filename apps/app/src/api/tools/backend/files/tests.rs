use super::*;
use std::io::{Cursor, Write};
fn command(name: &str, args: Value) -> Command {
    serde_json::from_value(json!({"name":name,"arguments":args})).unwrap()
}
fn args(path: &str) -> Value {
    json!({"target_kind":"instance","target_id":"test","path":path})
}

#[test]
fn paths_and_conditional_writes_reject_traversal_stale_hashes_and_overwrites() {
    let temp = tempfile::tempdir().unwrap();
    let root = open_root(temp.path()).unwrap();
    for path in [
        "../outside",
        "a/../b",
        "/root",
        "a\\b",
        "C:/bad",
        "file:stream",
        "a//b",
        "trailing.",
        "NUL.txt",
        "COM1",
    ] {
        assert!(valid(path, false).is_err(), "{path}");
    }
    write(&root, "config.txt", b"old", None, true).unwrap();
    assert!(write(&root, "config.txt", b"lost", None, true).is_err());
    assert!(
        write(
            &root,
            "config.txt",
            b"lost",
            Some(&hash(b"different")),
            false
        )
        .is_err()
    );
    assert_eq!(read(&root, "config.txt", 10).unwrap(), b"old");
    write(&root, "config.txt", b"new", Some(&hash(b"old")), false).unwrap();
    assert_eq!(read(&root, "config.txt", 10).unwrap(), b"new");
    assert!(write(&root, "missing/child", b"x", None, true).is_err());
    assert!(read(&root, "config.txt", 2).is_err());
}

#[test]
fn text_pages_and_patches_preserve_unicode_and_reject_ambiguous_matches() {
    let temp = tempfile::tempdir().unwrap();
    let root = open_root(temp.path()).unwrap();
    write(&root, "config.txt", "旧值\r\n你好".as_bytes(), None, true).unwrap();
    let mut input = args("config.txt");
    input["offset"] = json!(1);
    input["max_bytes"] = json!(4);
    let result = run(
        root.try_clone().unwrap(),
        command("file_read_text", input),
        temp.path().into(),
    )
    .unwrap();
    assert!(result["text"].as_str().is_some());
    assert_eq!(result["sha256"], hash("旧值\r\n你好".as_bytes()));
    let mut patch = args("config.txt");
    patch["find"] = json!("旧值");
    patch["replace"] = json!("新值");
    patch["expected_sha256"] = json!(hash("旧值\r\n你好".as_bytes()));
    run(
        root.try_clone().unwrap(),
        command("file_apply_patch", patch.clone()),
        temp.path().into(),
    )
    .unwrap();
    assert!(
        run(
            root.try_clone().unwrap(),
            command("file_apply_patch", patch),
            temp.path().into()
        )
        .is_err()
    );
    assert_eq!(
        read(&root, "config.txt", 100).unwrap(),
        "新值\r\n你好".as_bytes()
    );
    write(&root, "twice.txt", b"same same", None, true).unwrap();
    let mut patch = args("twice.txt");
    patch["find"] = json!("same");
    patch["replace"] = json!("new");
    patch["expected_sha256"] = json!(hash(b"same same"));
    assert!(
        run(root, command("file_apply_patch", patch), temp.path().into())
            .is_err()
    );
}

#[test]
fn archive_roundtrip_and_zip_slip_are_bounded_and_do_not_overwrite() {
    let temp = tempfile::tempdir().unwrap();
    let root = open_root(temp.path()).unwrap();
    root.create_dir("source").unwrap();
    write(&root, "source/a.txt", b"hello", None, true).unwrap();
    let result=archives::execute(&root,command("archive_create",json!({"target_kind":"instance","target_id":"test","paths":["source/a.txt"],"destination_path":"bundle.zip"}))).unwrap();
    let mut extract = args("bundle.zip");
    extract["destination_path"] = json!("output");
    extract["expected_sha256"] = result["sha256"].clone();
    archives::execute(&root, command("archive_extract", extract.clone()))
        .unwrap();
    assert_eq!(read(&root, "output/source/a.txt", 50).unwrap(), b"hello");
    assert!(
        archives::execute(&root, command("archive_extract", extract)).is_err()
    );
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file("../escape.txt", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(b"bad").unwrap();
    let bytes = zip.finish().unwrap().into_inner();
    write(&root, "bad.zip", &bytes, None, true).unwrap();
    let mut extract = args("bad.zip");
    extract["destination_path"] = json!("bad-output");
    extract["expected_sha256"] = json!(hash(&bytes));
    assert!(
        archives::execute(&root, command("archive_extract", extract)).is_err()
    );
    assert!(!temp.path().join("bad-output").exists());
}

#[test]
fn nbt_roundtrips_types_compression_lists_and_long_precision() {
    use quartz_nbt::{
        NbtCompound,
        io::{Flavor, read_nbt, write_nbt},
    };
    let temp = tempfile::tempdir().unwrap();
    let root = open_root(temp.path()).unwrap();
    for (i, flavor) in [
        Flavor::Uncompressed,
        Flavor::GzCompressed,
        Flavor::ZlibCompressed,
    ]
    .into_iter()
    .enumerate()
    {
        let name = format!("test-{i}.dat");
        let compound=NbtCompound::from_snbt("{Data:{counter:9223372036854775806L,flag:1b,list:[1,2],array:[I;4,5]}}").unwrap();
        let mut bytes = Vec::new();
        write_nbt(&mut bytes, Some("World"), &compound, flavor).unwrap();
        write(&root, &name, &bytes, None, true).unwrap();
        let mut query = args(&name);
        query["tag_path"] = json!(["Data", "counter"]);
        let result = nbt::execute(&root, command("nbt_read", query)).unwrap();
        assert_eq!(result["value"], "9223372036854775806");
        assert_eq!(result["type"], "long");
        assert_eq!(result["root_name"], "World");
        let mut patch = args(&name);
        patch["tag_path"] = json!(["Data", "list", "1"]);
        patch["remove"] = json!(false);
        patch["value_snbt"] = json!("3");
        patch["expected_sha256"] = json!(hash(&bytes));
        nbt::execute(&root, command("nbt_patch", patch)).unwrap();
        let updated = read(&root, &name, MAX).unwrap();
        let (c, root_name) =
            read_nbt(&mut Cursor::new(updated), flavor).unwrap();
        assert_eq!(root_name, "World");
        assert!(c.to_snbt().contains("[1,3]"));
        let mut patch = args(&name);
        patch["tag_path"] = json!(["Data", "flag"]);
        patch["remove"] = json!(false);
        patch["value_snbt"] = json!("2");
        patch["expected_sha256"] =
            json!(hash(&read(&root, &name, MAX).unwrap()));
        assert!(nbt::execute(&root, command("nbt_patch", patch)).is_err());
    }
}

#[test]
fn malformed_nbt_lengths_and_deep_structures_fail_before_parser_allocation() {
    let temp = tempfile::tempdir().unwrap();
    let root = open_root(temp.path()).unwrap();
    let mut cases = vec![
        vec![10, 0, 0, 7, 0, 1, b'a', 0xff, 0xff, 0xff, 0xff, 0],
        vec![10, 0, 0, 9, 0, 1, b'a', 1, 0x7f, 0xff, 0xff, 0xff, 0],
    ];
    let mut deep = vec![10, 0, 0];
    for _ in 0..35 {
        deep.extend_from_slice(&[10, 0, 0]);
    }
    deep.extend_from_slice(&[0; 36]);
    cases.push(deep);
    for (i, bytes) in cases.into_iter().enumerate() {
        let name = format!("bad{i}.dat");
        write(&root, &name, &bytes, None, true).unwrap();
        assert!(nbt::execute(&root, command("nbt_read", args(&name))).is_err());
    }
}

#[test]
fn directory_handles_do_not_switch_targets_when_path_is_replaced() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir(temp.path().join("dir")).unwrap();
    std::fs::write(temp.path().join("dir/file"), b"original").unwrap();
    let root = open_root(temp.path()).unwrap();
    let held = directory(&root, "dir").unwrap();
    if std::fs::rename(temp.path().join("dir"), temp.path().join("old"))
        .is_err()
    {
        return;
    }
    std::fs::create_dir(temp.path().join("dir")).unwrap();
    std::fs::write(temp.path().join("dir/file"), b"replacement").unwrap();
    assert_eq!(read(&held, "file", 100).unwrap(), b"original");
    write(&held, "new", b"safe", None, true).unwrap();
    assert!(!temp.path().join("dir/new").exists());
    assert_eq!(std::fs::read(temp.path().join("old/new")).unwrap(), b"safe");
}

#[cfg(unix)]
#[test]
fn links_cannot_read_write_or_traverse_outside_target() {
    let temp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret"), b"private").unwrap();
    std::os::unix::fs::symlink(outside.path(), temp.path().join("link"))
        .unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("secret"),
        temp.path().join("file"),
    )
    .unwrap();
    let root = open_root(temp.path()).unwrap();
    assert!(read(&root, "link/secret", 100).is_err());
    assert!(read(&root, "file", 100).is_err());
    assert!(write(&root, "link/new", b"bad", None, true).is_err());
}
