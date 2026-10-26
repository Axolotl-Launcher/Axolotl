//! Positional output for native HTTP/1.1 and HTTP/2 range downloads.

use crate::util::download::constants::{
    RANGE_OUTPUT_CREATING_RANGED_DOWNLOAD_OUTPUT,
    RANGE_OUTPUT_FLUSHING_RANGED_DOWNLOAD_OUTPUT,
    RANGE_OUTPUT_INVALID_RANGE_BOUNDS, RANGE_OUTPUT_OFFSET_OVERFLOW,
    RANGE_OUTPUT_OPENING_RANGED_DOWNLOAD_OUTPUT,
    RANGE_OUTPUT_RANGE_OUTPUT_LENGTH_CHANGED, RANGE_OUTPUT_WRITE_EXCEEDS_RANGE,
};
use crate::util::io::{self, IOError};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncSeekExt, AsyncWriteExt, BufWriter, SeekFrom};

use super::constants::RANGE_WRITER_BUFFER_CAPACITY;

#[cfg(test)]
struct RangeWriteTestProbe {
    delay: std::time::Duration,
    in_flight: std::sync::atomic::AtomicUsize,
    max_in_flight: std::sync::atomic::AtomicUsize,
}

#[cfg(test)]
static RANGE_WRITE_TEST_PROBE: std::sync::Mutex<
    Option<Arc<RangeWriteTestProbe>>,
> = std::sync::Mutex::new(None);

/// A preallocated `.part` file. Writers open their own handle so a slow range
/// never serializes unrelated ranges behind a file mutex.
pub(crate) struct RangeOutput {
    path: PathBuf,
    size: u64,
}

/// An independent sequential writer for one half-open byte range.
pub(crate) struct RangeWriter {
    file: BufWriter<File>,
    offset: u64,
    end_exclusive: u64,
    path: PathBuf,
}

impl RangeOutput {
    /// Extend a contiguous partial file without overwriting its accepted prefix.
    /// The caller holds the destination lock and verifies the final file hash.
    pub(crate) async fn resume_prefix(
        path: &Path,
        size: u64,
        prefix: u64,
    ) -> Result<Arc<Self>, IOError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .await
            .map_err(|error| IOError::with_path(error, path))?;
        let metadata = file
            .metadata()
            .await
            .map_err(|error| IOError::with_path(error, path))?;
        if prefix == 0
            || prefix >= size
            || !metadata.is_file()
            || metadata.len() != prefix
        {
            return Err(IOError::with_path(
                std::io::Error::other(RANGE_OUTPUT_RANGE_OUTPUT_LENGTH_CHANGED),
                path,
            ));
        }
        super::local_resources::write(path, 0, file.set_len(size))
            .await
            .map_err(|error| IOError::with_path(error, path))?;
        Ok(Arc::new(Self {
            path: path.to_path_buf(),
            size,
        }))
    }

    pub(crate) async fn reopen(
        path: &Path,
        size: u64,
    ) -> Result<Arc<Self>, IOError> {
        let metadata = tokio::fs::metadata(path)
            .await
            .map_err(|error| IOError::with_path(error, path))?;
        if !metadata.is_file() || metadata.len() != size {
            return Err(IOError::with_path(
                std::io::Error::other(RANGE_OUTPUT_RANGE_OUTPUT_LENGTH_CHANGED),
                path,
            ));
        }
        Ok(Arc::new(Self {
            path: path.to_path_buf(),
            size,
        }))
    }
    pub(crate) async fn create(
        path: &Path,
        size: u64,
    ) -> Result<Arc<Self>, IOError> {
        io::retry_windows_sharing_violation(
            path,
            RANGE_OUTPUT_CREATING_RANGED_DOWNLOAD_OUTPUT,
            || async {
                let file = OpenOptions::new()
                    .create(true)
                    .truncate(true)
                    .read(true)
                    .write(true)
                    .open(path)
                    .await?;
                super::local_resources::write(path, 0, file.set_len(size))
                    .await?;
                Ok(())
            },
        )
        .await
        .map_err(|error| io::io_error_with_lock_info(error, path))?;
        Ok(Arc::new(Self {
            path: path.to_path_buf(),
            size,
        }))
    }

    pub(crate) async fn open_range(
        &self,
        start: u64,
        end_exclusive: u64,
    ) -> Result<RangeWriter, IOError> {
        if start >= end_exclusive || end_exclusive > self.size {
            return Err(IOError::with_path(
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    RANGE_OUTPUT_INVALID_RANGE_BOUNDS,
                ),
                &self.path,
            ));
        }
        let path = self.path.clone();
        let mut file = io::retry_windows_sharing_violation(
            &path,
            RANGE_OUTPUT_OPENING_RANGED_DOWNLOAD_OUTPUT,
            || async {
                OpenOptions::new().read(true).write(true).open(&path).await
            },
        )
        .await
        .map_err(|error| io::io_error_with_lock_info(error, &path))?;
        file.seek(SeekFrom::Start(start))
            .await
            .map_err(|error| io::io_error_with_lock_info(error, &path))?;
        Ok(RangeWriter {
            file: BufWriter::with_capacity(RANGE_WRITER_BUFFER_CAPACITY, file),
            offset: start,
            end_exclusive,
            path,
        })
    }

    /// Legacy adapter; not used by the concurrent range hot path.
    #[allow(dead_code)]
    pub(crate) async fn write_at(
        &self,
        offset: u64,
        bytes: &[u8],
        path: &Path,
    ) -> Result<(), IOError> {
        let end = offset.checked_add(bytes.len() as u64).ok_or_else(|| {
            IOError::with_path(
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    RANGE_OUTPUT_OFFSET_OVERFLOW,
                ),
                path,
            )
        })?;
        let mut writer = self.open_range(offset, end).await?;
        writer.write_next(bytes).await?;
        writer.flush().await
    }

    /// Legacy adapter for callers that previously flushed the shared handle.
    #[allow(dead_code)]
    pub(crate) async fn flush(&self, path: &Path) -> Result<(), IOError> {
        let mut file = io::retry_windows_sharing_violation(
            path,
            RANGE_OUTPUT_FLUSHING_RANGED_DOWNLOAD_OUTPUT,
            || async {
                OpenOptions::new().read(true).write(true).open(path).await
            },
        )
        .await
        .map_err(|error| io::io_error_with_lock_info(error, path))?;
        super::local_resources::write(path, 0, file.flush())
            .await
            .map_err(|error| io::io_error_with_lock_info(error, path))
    }
}

impl RangeWriter {
    #[cfg(test)]
    pub(crate) fn offset(&self) -> u64 {
        self.offset
    }

    pub(crate) fn remaining(&self) -> u64 {
        self.end_exclusive.saturating_sub(self.offset)
    }

    pub(crate) async fn write_next(
        &mut self,
        bytes: &[u8],
    ) -> Result<(), IOError> {
        if bytes.len() as u64 > self.remaining() {
            return Err(IOError::with_path(
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    RANGE_OUTPUT_WRITE_EXCEEDS_RANGE,
                ),
                &self.path,
            ));
        }
        #[cfg(test)]
        let probe = RANGE_WRITE_TEST_PROBE.lock().unwrap().clone();
        #[cfg(test)]
        if let Some(probe) = &probe {
            let in_flight = probe
                .in_flight
                .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
                + 1;
            probe
                .max_in_flight
                .fetch_max(in_flight, std::sync::atomic::Ordering::AcqRel);
            tokio::time::sleep(probe.delay).await;
        }

        let result = super::local_resources::write(
            &self.path,
            bytes.len() as u64,
            async {
                self.file.write_all(bytes).await?;
                self.file.get_mut().flush().await
            },
        )
        .await
        .map_err(|error| io::io_error_with_lock_info(error, &self.path));
        #[cfg(test)]
        if let Some(probe) = probe {
            probe
                .in_flight
                .fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
        }
        result?;
        self.offset += bytes.len() as u64;
        Ok(())
    }

    pub(crate) async fn flush(&mut self) -> Result<(), IOError> {
        super::local_resources::write(&self.path, 0, self.file.flush())
            .await
            .map_err(|error| io::io_error_with_lock_info(error, &self.path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::future::try_join_all;
    use std::sync::atomic::Ordering;

    #[tokio::test]
    async fn extends_only_an_unchanged_contiguous_prefix() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("resume.part");
        tokio::fs::write(&path, [7; 128]).await.unwrap();
        assert!(RangeOutput::resume_prefix(&path, 512, 127).await.is_err());
        assert_eq!(tokio::fs::metadata(&path).await.unwrap().len(), 128);
        let output = RangeOutput::resume_prefix(&path, 512, 128).await.unwrap();
        let mut writer = output.open_range(128, 512).await.unwrap();
        writer.write_next(&[8; 384]).await.unwrap();
        writer.flush().await.unwrap();
        let bytes = tokio::fs::read(&path).await.unwrap();
        assert_eq!(&bytes[..128], &[7; 128]);
        assert_eq!(&bytes[128..], &[8; 384]);
    }

    #[tokio::test]
    async fn writes_non_overlapping_ranges_with_independent_writers() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("ranged.part");
        let output = RangeOutput::create(&path, 12).await.unwrap();
        let mut a = output.open_range(0, 4).await.unwrap();
        let mut b = output.open_range(4, 8).await.unwrap();
        let mut c = output.open_range(8, 12).await.unwrap();
        a.write_next(b"abcd").await.unwrap();
        b.write_next(b"efgh").await.unwrap();
        c.write_next(b"ijkl").await.unwrap();
        a.flush().await.unwrap();
        b.flush().await.unwrap();
        c.flush().await.unwrap();
        assert_eq!(tokio::fs::read(path).await.unwrap(), b"abcdefghijkl");
    }

    #[tokio::test]
    async fn rejects_writes_beyond_range() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("ranged.part");
        let output = RangeOutput::create(&path, 8).await.unwrap();
        let mut writer = output.open_range(0, 4).await.unwrap();
        assert!(writer.write_next(b"12345").await.is_err());
        assert_eq!(writer.offset(), 0);
    }

    #[tokio::test]
    async fn rejects_empty_and_out_of_file_ranges() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("ranged.part");
        let output = RangeOutput::create(&path, 8).await.unwrap();
        assert!(output.open_range(2, 2).await.is_err());
        assert!(output.open_range(7, 9).await.is_err());
    }

    #[tokio::test]
    async fn buffers_small_exact_and_large_writes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("ranged.part");
        let small = vec![1_u8; 1024];
        let exact = vec![2_u8; RANGE_WRITER_BUFFER_CAPACITY];
        let large = vec![3_u8; RANGE_WRITER_BUFFER_CAPACITY + 1];
        let total = small.len() + exact.len() + large.len();
        let output = RangeOutput::create(&path, total as u64).await.unwrap();
        let mut writer = output.open_range(0, total as u64).await.unwrap();

        writer.write_next(&small).await.unwrap();
        writer.write_next(&exact).await.unwrap();
        writer.write_next(&large).await.unwrap();
        writer.flush().await.unwrap();

        let mut expected = small;
        expected.extend(exact);
        expected.extend(large);
        assert_eq!(tokio::fs::read(path).await.unwrap(), expected);
    }

    #[tokio::test]
    async fn concurrent_writers_preserve_all_ranges_and_overlap() {
        let probe = Arc::new(RangeWriteTestProbe {
            delay: std::time::Duration::from_millis(2),
            in_flight: std::sync::atomic::AtomicUsize::new(0),
            max_in_flight: std::sync::atomic::AtomicUsize::new(0),
        });
        *RANGE_WRITE_TEST_PROBE.lock().unwrap() = Some(Arc::clone(&probe));

        for range_count in [2, 4, 8] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("ranged.part");
            let range_length = 32 * 1024;
            let expected = (0..range_length * range_count)
                .map(|index| (index % 251) as u8)
                .collect::<Vec<_>>();
            let output = RangeOutput::create(&path, expected.len() as u64)
                .await
                .unwrap();
            let writes = (0..range_count).map(|index| {
                let output = Arc::clone(&output);
                let start = index * range_length;
                let bytes = expected[start..start + range_length].to_vec();
                async move {
                    let mut writer = output
                        .open_range(start as u64, (start + range_length) as u64)
                        .await?;
                    let mut offset = 0;
                    while offset < bytes.len() {
                        let length = ((index * 521 + offset) % 4096 + 1)
                            .min(bytes.len() - offset);
                        writer
                            .write_next(&bytes[offset..offset + length])
                            .await?;
                        offset += length;
                    }
                    writer.flush().await
                }
            });
            try_join_all(writes).await.unwrap();
            assert_eq!(tokio::fs::read(path).await.unwrap(), expected);
        }
        *RANGE_WRITE_TEST_PROBE.lock().unwrap() = None;

        assert!(probe.max_in_flight.load(Ordering::Acquire) > 1);
    }
}
