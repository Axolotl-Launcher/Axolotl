use crate::install::InstallProgressReporter;
use crate::util::fetch::DownloadResult;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
pub(crate) struct CurseForgeDownloadMetrics {
    pub(super) source: Mutex<Option<String>>,
    pub(super) fallback_count: AtomicU64,
    pub(super) reporter: Option<InstallProgressReporter>,
}

impl CurseForgeDownloadMetrics {
    pub(super) fn with_reporter(reporter: InstallProgressReporter) -> Self {
        Self {
            reporter: Some(reporter),
            ..Self::default()
        }
    }

    pub(super) fn record(&self, result: &DownloadResult) {
        if result.attempts > 0
            && let Ok(mut source) = self.source.lock()
        {
            *source = Some(result.source.as_str().to_string());
        }
        self.fallback_count
            .fetch_add(result.fallback_count as u64, Ordering::Relaxed);
    }

    pub(super) async fn finish(
        &self,
        reporter: &InstallProgressReporter,
    ) -> crate::Result<()> {
        let source = self.source.lock().ok().and_then(|source| source.clone());
        if let Some(source) = source {
            reporter
                .record_download_metrics(
                    source,
                    self.fallback_count.load(Ordering::Relaxed),
                )
                .await?;
        }
        Ok(())
    }
}
