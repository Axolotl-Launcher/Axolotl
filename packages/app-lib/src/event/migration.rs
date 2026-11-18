//! First storage takeover is awaited before the runtime becomes available.
use super::{LoadingBarId, LoadingBarType};

pub(crate) struct MigrationProgress {
    key: Option<LoadingBarId>,
    finished: bool,
}

impl MigrationProgress {
    pub(crate) async fn new() -> Self {
        let key = super::emit::init_loading(
            LoadingBarType::DatabaseMigration {
                stage: "preparing".into(),
                completed: 0,
                total: 0,
            },
            1.0,
            "Migrating launcher data",
        )
        .await
        .ok();
        Self {
            key,
            finished: false,
        }
    }

    pub(crate) fn update(
        &mut self,
        stage: &str,
        completed: u64,
        total: u64,
        fraction: f64,
    ) {
        let Some(key) = &self.key else { return };
        let Ok(events) = super::EventState::get() else {
            return;
        };
        {
            let Some(mut bar) = events.loading_bars.get_mut(&key.0) else {
                return;
            };
            bar.bar_type = LoadingBarType::DatabaseMigration {
                stage: stage.into(),
                completed,
                total,
            };
            bar.current =
                fraction.clamp(0.0, if self.finished { 1.0 } else { 0.99 });
            bar.message = "Migrating launcher data".into();
            // Stage and count changes matter even when the overall fraction
            // moves by less than the generic loading event throttle.
            bar.last_sent = -1.0;
        }
        if let Err(error) = super::emit::emit_loading(key, 0.0, None) {
            tracing::debug!(%error, "Could not report storage migration progress");
        }
    }

    pub(crate) fn recovery(&mut self, completed: u64, total: u64) {
        let fraction = if total == 0 {
            0.2
        } else {
            0.2 + 0.78 * (completed.min(total) as f64 / total as f64)
        };
        self.update("recovery", completed, total, fraction);
    }

    pub(crate) fn finish(&mut self) {
        self.finished = true;
        self.update("finishing", 1, 1, 1.0);
    }
}

impl Drop for MigrationProgress {
    fn drop(&mut self) {
        if !self.finished {
            // LoadingBarId normally announces completion on Drop. A failed
            // or cancelled takeover must not be displayed as successful.
            if let Some(key) = &self.key
                && let Ok(events) = super::EventState::get()
            {
                events.loading_bars.remove(&key.0);
            }
        }
    }
}

#[cfg(all(test, not(feature = "tauri")))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn snapshots_include_sub_percent_record_updates_and_only_finish_at_receipt()
     {
        let events = super::super::EventState::init().await.unwrap();
        let mut progress = MigrationProgress::new().await;
        progress.recovery(1, 1000);
        let id = progress.key.as_ref().unwrap().0;
        let bar = events.loading_bars.get(&id).unwrap();
        assert!(
            matches!(&bar.bar_type, LoadingBarType::DatabaseMigration { stage, completed: 1, total: 1000 } if stage == "recovery")
        );
        assert!(bar.current < 1.0);
        drop(bar);
        progress.recovery(1000, 1000);
        assert!(events.loading_bars.get(&id).unwrap().current < 1.0);
        progress.finish();
        assert_eq!(events.loading_bars.get(&id).unwrap().current, 1.0);
    }

    #[tokio::test]
    async fn aborted_takeover_removes_progress_without_a_completion() {
        let events = super::super::EventState::init().await.unwrap();
        let progress = MigrationProgress::new().await;
        let id = progress.key.as_ref().unwrap().0;
        drop(progress);
        assert!(!events.loading_bars.contains_key(&id));
    }
}
