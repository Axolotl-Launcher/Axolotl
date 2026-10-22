//! Bounded buffering for single-stream files; RangeWriter owns range output.

use std::path::{Path, PathBuf};
use std::time::Instant;
use tokio::io::{AsyncWrite, AsyncWriteExt, BufWriter};

use super::constants::{
    STREAM_OUTPUT_CAPACITY as CAPACITY,
    STREAM_OUTPUT_FLUSH_INTERVAL as FLUSH_INTERVAL,
};

pub(crate) struct StreamOutput<W = tokio::fs::File> {
    writer: BufWriter<W>,
    path: PathBuf,
    flushed_at: Instant,
}

impl<W: AsyncWrite + Unpin> StreamOutput<W> {
    pub(crate) fn pending_bytes(&self) -> u64 {
        self.writer.buffer().len() as u64
    }
    pub(crate) fn new(file: W, path: &Path) -> Self {
        Self {
            writer: BufWriter::with_capacity(CAPACITY, file),
            path: path.to_path_buf(),
            flushed_at: Instant::now(),
        }
    }

    pub(crate) async fn write_chunk(
        &mut self,
        bytes: &[u8],
    ) -> std::io::Result<()> {
        let flush_due = self.flushed_at.elapsed() >= FLUSH_INTERVAL;
        super::local_resources::write(&self.path, bytes.len() as u64, async {
            self.writer.write_all(bytes).await?;
            if flush_due {
                self.writer.flush().await?;
            }
            Ok(())
        })
        .await?;
        if flush_due {
            self.flushed_at = Instant::now();
        }
        Ok(())
    }

    pub(crate) async fn flush(&mut self) -> std::io::Result<()> {
        super::local_resources::write(&self.path, 0, self.writer.flush())
            .await?;
        self.flushed_at = Instant::now();
        Ok(())
    }
}

impl StreamOutput {
    pub(crate) async fn sync_data(&self) -> std::io::Result<()> {
        self.writer.get_ref().sync_data().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::pin::Pin;
    use std::sync::{Arc, Mutex};
    use std::task::{Context, Poll};

    #[derive(Default)]
    struct SinkState {
        writes: usize,
        bytes: usize,
        fail_flush: bool,
    }
    struct Sink(Arc<Mutex<SinkState>>);
    impl AsyncWrite for Sink {
        fn poll_write(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
            bytes: &[u8],
        ) -> Poll<std::io::Result<usize>> {
            let mut state = self.0.lock().unwrap();
            state.writes += 1;
            state.bytes += bytes.len();
            Poll::Ready(Ok(bytes.len()))
        }
        fn poll_flush(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
        ) -> Poll<std::io::Result<()>> {
            Poll::Ready(if self.0.lock().unwrap().fail_flush {
                Err(std::io::Error::other("flush failed"))
            } else {
                Ok(())
            })
        }
        fn poll_shutdown(
            self: Pin<&mut Self>,
            context: &mut Context<'_>,
        ) -> Poll<std::io::Result<()>> {
            self.poll_flush(context)
        }
    }

    #[tokio::test]
    async fn small_chunks_are_coalesced_and_completion_requires_flush() {
        let state = Arc::new(Mutex::new(SinkState::default()));
        let mut output =
            StreamOutput::new(Sink(state.clone()), Path::new("stream-test"));
        for _ in 0..100 {
            output.write_chunk(&[7; 1024]).await.unwrap();
        }
        assert_eq!(state.lock().unwrap().writes, 0);
        assert_eq!(output.pending_bytes(), 100 * 1024);
        output.flush().await.unwrap();
        assert_eq!(output.pending_bytes(), 0);
        assert_eq!(state.lock().unwrap().writes, 1);
        assert_eq!(state.lock().unwrap().bytes, 100 * 1024);
        state.lock().unwrap().fail_flush = true;
        assert!(output.flush().await.is_err());
    }

    #[tokio::test]
    async fn cancellation_resume_uses_only_the_prefix_already_written_to_disk()
    {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("stream.part");
        let file = tokio::fs::File::create(&path).await.unwrap();
        let mut output = StreamOutput::new(file, &path);
        output.write_chunk(&vec![7; CAPACITY]).await.unwrap();
        output.flush().await.unwrap();
        output.write_chunk(&[8; 1024]).await.unwrap();
        drop(output);
        let persisted = tokio::fs::read(&path).await.unwrap();
        assert_eq!(persisted, vec![7; CAPACITY]);
    }
}
