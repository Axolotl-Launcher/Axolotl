use sha2::{Digest, Sha256};
use std::fs;
use std::io::{self, Read};
use std::path::Path;

pub const INPUTS: &[&str] = &[
    "java/src",
    "java/gradle",
    "java/build.gradle.kts",
    "java/settings.gradle.kts",
    "java/gradle.properties",
    "java/gradlew",
    "java/gradlew.bat",
];
pub const JARS: &[&str] =
    &["java/libs/theseus.jar", "java/libs/authlib-injector.jar"];
const MANIFEST: &str = "java-build-fingerprint";

fn hash_file(path: &Path) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 8192];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn hash_tree(
    root: &Path,
    relative: &Path,
    hash: &mut Sha256,
) -> io::Result<()> {
    let path = root.join(relative);
    let metadata = fs::metadata(&path)?;
    let name = relative.to_string_lossy();
    hash.update((name.len() as u64).to_le_bytes());
    hash.update(name.as_bytes());
    if metadata.is_dir() {
        hash.update(b"directory");
        let mut entries = fs::read_dir(path)?
            .map(|entry| entry.map(|entry| entry.file_name()))
            .collect::<io::Result<Vec<_>>>()?;
        entries.sort();
        hash.update((entries.len() as u64).to_le_bytes());
        for entry in entries {
            hash_tree(root, &relative.join(entry), hash)?;
        }
    } else if metadata.is_file() {
        hash.update(b"file");
        hash.update(hash_file(&path)?.as_bytes());
    } else {
        return Err(io::Error::other("Unsupported Java build input"));
    }
    Ok(())
}

pub fn input_fingerprint(root: &Path) -> io::Result<String> {
    let mut hash = Sha256::new();
    for input in INPUTS {
        hash_tree(root, Path::new(input), &mut hash)?;
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn manifest(out_dir: &Path, inputs: &str) -> io::Result<String> {
    let mut contents = format!("java-build-v1\n{inputs}\n");
    for jar in JARS {
        contents.push_str(&hash_file(&out_dir.join(jar))?);
        contents.push('\n');
    }
    Ok(contents)
}

pub fn is_fresh(out_dir: &Path, inputs: &str) -> bool {
    match (
        fs::read_to_string(out_dir.join(MANIFEST)),
        manifest(out_dir, inputs),
    ) {
        (Ok(recorded), Ok(current)) => recorded == current,
        _ => false,
    }
}

pub fn invalidate(out_dir: &Path) -> io::Result<()> {
    match fs::remove_file(out_dir.join(MANIFEST)) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

pub fn record_success(out_dir: &Path, inputs: &str) -> io::Result<()> {
    fs::write(out_dir.join(MANIFEST), manifest(out_dir, inputs)?)
}
