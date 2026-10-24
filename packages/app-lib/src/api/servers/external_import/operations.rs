//! Serializes directory-mutating server operations without holding a DashMap reference across awaits.
use dashmap::DashSet;
use std::sync::LazyLock;

static BUSY: LazyLock<DashSet<String>> = LazyLock::new(DashSet::new);

pub(in crate::api::servers) struct OperationGuard(String);

pub(in crate::api::servers) fn acquire(
    id: &str,
) -> crate::Result<OperationGuard> {
    if !BUSY.insert(id.to_owned()) {
        return Err(super::input(
            "Another server operation is in progress. Wait for it to finish and try again",
        ));
    }
    Ok(OperationGuard(id.to_owned()))
}

impl Drop for OperationGuard {
    fn drop(&mut self) {
        BUSY.remove(&self.0);
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn active_operation_blocks_start_delete_and_retry_before_directory_access()
     {
        let id = uuid::Uuid::new_v4().to_string();
        let _guard = super::acquire(&id).unwrap();
        for result in [
            crate::api::servers::lifecycle::start(&id, None, None, None).await,
            crate::api::servers::manage::delete(&id).await,
            super::super::install::finish_import_install(&id).await,
        ] {
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("Another server operation")
            );
        }
    }

    #[test]
    fn operations_are_exclusive_and_release_on_drop() {
        let id = uuid::Uuid::new_v4().to_string();
        let guard = super::acquire(&id).unwrap();
        assert!(super::acquire(&id).is_err());
        assert!(super::acquire("another-server").is_ok());
        drop(guard);
        assert!(super::acquire(&id).is_ok());
    }
}
