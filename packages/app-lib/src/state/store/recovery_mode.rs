//! Explicit read-only recovery mode. No damaged database is rewritten.
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, Weak};
struct RecoveryRoot {
    owner: Weak<sqlx::sqlite::SqliteConnectOptions>,
    lexical: PathBuf,
    canonical: PathBuf,
}
fn roots() -> &'static Mutex<HashMap<usize, RecoveryRoot>> {
    static ROOTS: OnceLock<Mutex<HashMap<usize, RecoveryRoot>>> =
        OnceLock::new();
    ROOTS.get_or_init(Mutex::default)
}
pub(crate) fn register(pool: &SqlitePool, root: &Path) {
    let options = pool.connect_options();
    let lexical_root = lexical_path(root);
    let normalized_root = normalized(root);
    let mut roots = roots().lock().expect("recovery roots");
    roots.retain(|_, root| root.owner.strong_count() != 0);
    roots.insert(
        Arc::as_ptr(&options) as usize,
        RecoveryRoot {
            owner: Arc::downgrade(&options),
            lexical: lexical_root,
            canonical: normalized_root,
        },
    );
}
pub(crate) fn root(pool: &SqlitePool) -> Option<PathBuf> {
    let options = pool.connect_options();
    roots()
        .lock()
        .expect("recovery roots")
        .get(&(Arc::as_ptr(&options) as usize))
        .filter(|root| root.owner.strong_count() != 0)
        .map(|root| root.canonical.clone())
}
pub(crate) fn ensure_writable(pool: &SqlitePool) -> crate::Result<()> {
    if root(pool).is_some() {
        Err(crate::ErrorKind::InputError("Instance database is unavailable; recovery copies allow browsing and launching only".into()).into())
    } else {
        Ok(())
    }
}

/// Unsupported business data is not permission to bypass its guards with a
/// stale copy. Only storage unavailability enters recovery mode.
pub(crate) fn can_recover(error: &crate::Error) -> bool {
    matches!(
        error.raw.as_ref(),
        crate::ErrorKind::Sqlx(_)
            | crate::ErrorKind::SqlxMigrate(_)
            | crate::ErrorKind::StdIOError(_)
            | crate::ErrorKind::IOError(_)
            | crate::ErrorKind::FSError(_)
    )
}

/// Independent authorities share the original root with the recovered core.
/// Recovery permits reads, never publication or credential side effects.
fn lexical_path(path: &Path) -> PathBuf {
    let path = dunce::simplified(path);
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}
fn normalized(path: &Path) -> PathBuf {
    let path = lexical_path(path);
    let path = path.as_path();
    if let Ok(path) = std::fs::canonicalize(path) {
        return lexical_path(&path);
    }
    let mut ancestor = path;
    let mut missing = Vec::new();
    while let Some(parent) = ancestor.parent() {
        if let Some(name) = ancestor.file_name() {
            missing.push(name.to_owned());
        }
        ancestor = parent;
        if let Ok(mut resolved) = std::fs::canonicalize(ancestor) {
            for name in missing.into_iter().rev() {
                resolved.push(name);
            }
            return lexical_path(&resolved);
        }
    }
    path.to_owned()
}
pub(crate) fn path_is_read_only(path: &Path) -> bool {
    let roots = {
        let mut roots = roots().lock().expect("recovery roots");
        roots.retain(|_, root| root.owner.strong_count() != 0);
        roots
            .values()
            .map(|root| (root.lexical.clone(), root.canonical.clone()))
            .collect::<Vec<_>>()
    };
    if roots.is_empty() {
        return false;
    }
    let lexical = lexical_path(path);
    let resolved = normalized(path);
    roots.iter().any(|(registered, canonical)| {
        lexical.starts_with(registered)
            || lexical.starts_with(canonical)
            || resolved.starts_with(canonical)
    })
}

pub(crate) fn ensure_path_writable(path: &Path) -> crate::Result<()> {
    if path_is_read_only(path) {
        Err(crate::ErrorKind::InputError("Instance database is unavailable; recovery copies allow browsing and launching only".into()).into())
    } else {
        Ok(())
    }
}
pub(crate) fn ensure_state_writable() -> crate::Result<()> {
    if let Some(state) = crate::State::get_if_initialized() {
        ensure_writable(&state.pool)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(any(unix, windows))]
    #[tokio::test]
    async fn recovery_root_alias_cannot_escape_through_an_inner_directory_link()
    {
        let temporary = tempfile::tempdir().unwrap();
        let original = temporary.path().join("original");
        let outside = temporary.path().join("outside");
        let alias = temporary.path().join("alias");
        tokio::fs::create_dir(&original).await.unwrap();
        tokio::fs::create_dir(&outside).await.unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&original, &alias).unwrap();
            std::os::unix::fs::symlink(&outside, original.join("escape"))
                .unwrap();
        }
        #[cfg(windows)]
        {
            junction::create(&original, &alias).unwrap();
            junction::create(&outside, original.join("escape")).unwrap();
        }
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        register(&pool, &alias);
        assert_eq!(root(&pool), Some(normalized(&original)));
        for path in [
            alias.join("escape/new.json"),
            alias.join("./escape/missing/../new.json"),
            original.join("escape/new.json"),
        ] {
            assert!(
                ensure_path_writable(&path).is_err(),
                "an internal link must not bypass read-only recovery: {}",
                path.display()
            );
        }
        assert!(ensure_path_writable(&outside.join("new.json")).is_ok());
        assert!(!outside.join("new.json").exists());
        drop(pool);
        assert!(ensure_path_writable(&alias.join("escape/new.json")).is_ok());
    }

    #[cfg(not(feature = "tauri"))]
    #[tokio::test]
    async fn recovery_group_commands_refuse_before_mutating_files_or_memory() {
        let root = tempfile::tempdir().unwrap().keep();
        crate::event::EventState::init().await.unwrap();
        let state =
            crate::State::init_for_test(root.to_string_lossy().to_string())
                .await
                .unwrap();
        let instance =
            crate::api::instance::create_group("Before recovery".into())
                .await
                .unwrap();
        let favorite =
            crate::resources::favorite_groups::create("Before recovery".into())
                .await
                .unwrap();
        let original_instances =
            tokio::fs::read(root.join("instance_groups.json"))
                .await
                .unwrap();
        let original_favorites =
            tokio::fs::read(root.join("favorite_groups.json"))
                .await
                .unwrap();
        register(&state.pool, &root);
        assert!(
            crate::state::instance_groups::forget_instance("absent").is_err()
        );
        assert!(crate::state::favorite_groups::forget("absent").is_err());
        assert!(
            crate::api::instance::create_group("Blocked".into())
                .await
                .is_err()
        );
        assert!(
            crate::api::instance::rename_group(
                instance.id.clone(),
                "Blocked".into()
            )
            .await
            .is_err()
        );
        assert!(
            crate::api::instance::set_group_order(vec![instance.id.clone()])
                .await
                .is_err()
        );
        assert!(
            crate::resources::favorite_groups::create("Blocked".into())
                .await
                .is_err()
        );
        assert!(
            crate::resources::favorite_groups::rename(
                favorite.id.clone(),
                "Blocked".into()
            )
            .await
            .is_err()
        );
        assert_eq!(
            tokio::fs::read(root.join("instance_groups.json"))
                .await
                .unwrap(),
            original_instances
        );
        assert_eq!(
            tokio::fs::read(root.join("favorite_groups.json"))
                .await
                .unwrap(),
            original_favorites
        );
        assert_eq!(
            crate::state::instance_groups::list()
                .iter()
                .find(|group| group.id == instance.id)
                .unwrap()
                .name,
            "Before recovery"
        );
        assert_eq!(
            crate::state::favorite_groups::list()
                .iter()
                .find(|group| group.id == favorite.id)
                .unwrap()
                .name,
            "Before recovery"
        );
    }

    #[tokio::test]
    async fn recovery_documents_refuse_mutations_and_read_backups_without_repair()
     {
        let root = tempfile::tempdir().unwrap();
        let persistence =
            crate::state::persistence::Persistence::new(root.path());
        let document = persistence
            .document("state/item.json", "test-state", 1)
            .unwrap();
        document
            .update(|data| {
                data.insert("value".into(), serde_json::json!(1));
                Ok(())
            })
            .await
            .unwrap();
        document
            .update(|data| {
                data.insert("value".into(), serde_json::json!(2));
                Ok(())
            })
            .await
            .unwrap();
        tokio::fs::write(document.path(), b"broken").await.unwrap();
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        register(&pool, root.path());
        assert!(
            document
                .update(|_| panic!(
                    "recovery must refuse before executing the mutation"
                ))
                .await
                .is_err()
        );
        assert!(
            crate::state::persistence::document::write_blob(
                &root.path().join("new/subdir.bin"),
                vec![1]
            )
            .await
            .is_err()
        );
        assert!(!root.path().join("new").exists());
        let loaded = document.read().await.unwrap().snapshot.unwrap();
        assert_eq!(loaded.data().unwrap()["value"], 1);
        assert_eq!(tokio::fs::read(document.path()).await.unwrap(), b"broken");
        assert_eq!(
            std::fs::read_dir(root.path().join("state"))
                .unwrap()
                .count(),
            3,
            "reading must not archive corrupt bytes or create new lock files"
        );
        assert!(
            ensure_path_writable(&root.path().join("state/./next.json"))
                .is_err()
        );
        drop(pool);
        assert!(
            ensure_path_writable(document.path()).is_ok(),
            "a closed recovery pool must not poison a later normal session"
        );
        document
            .update(|data| {
                data.insert("value".into(), serde_json::json!(3));
                Ok(())
            })
            .await
            .unwrap();
    }
}
