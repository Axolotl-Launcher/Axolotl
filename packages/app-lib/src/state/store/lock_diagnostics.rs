//! Diagnostic scopes, not an assertion about SQLite's operating-system locks.
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::collections::HashMap;
use std::future::Future;
use std::panic::Location;
use std::sync::{Mutex, OnceLock};
use tokio::time::{Duration, Instant};
use uuid::Uuid;

const FIRST_REPORT: Duration = Duration::from_secs(2);
const REPORT_INTERVAL: Duration = Duration::from_secs(10);
const MAX_CANDIDATES: usize = 16;

#[derive(Clone, Debug)]
struct Entry {
    database: String,
    source: String,
    span: Option<&'static str>,
    mode: &'static str,
    phase: &'static str,
    operation: Option<&'static str>,
    started: Instant,
    acquired: Option<Instant>,
    sql_release_confirmed: bool,
}

fn registry() -> &'static Mutex<HashMap<Uuid, Entry>> {
    static ACTIVE: OnceLock<Mutex<HashMap<Uuid, Entry>>> = OnceLock::new();
    ACTIVE.get_or_init(Mutex::default)
}

#[derive(Debug, serde::Serialize)]
struct Candidate {
    operation_id: Uuid,
    source: String,
    span: Option<&'static str>,
    mode: &'static str,
    phase: &'static str,
    operation: Option<&'static str>,
    elapsed_ms: u128,
    acquired_ms: Option<u128>,
    sql_release_confirmed: bool,
}

fn candidates(
    database: &str,
    requester: Uuid,
) -> (usize, usize, usize, Vec<Candidate>) {
    let entries: Vec<_> = registry()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .iter()
        .filter(|(id, entry)| **id != requester && entry.database == database)
        .map(|(id, entry)| (*id, entry.clone()))
        .collect();
    let mut selected: Vec<_> = entries
        .into_iter()
        .map(|(id, entry)| Candidate {
            operation_id: id,
            source: entry.source,
            span: entry.span,
            mode: entry.mode,
            phase: entry.phase,
            operation: entry.operation,
            elapsed_ms: entry.started.elapsed().as_millis(),
            acquired_ms: entry.acquired.map(|time| time.elapsed().as_millis()),
            sql_release_confirmed: entry.sql_release_confirmed,
        })
        .collect();
    selected.sort_by_key(|entry| {
        (
            (entry.acquired_ms.is_none()
                && entry.phase != "read_wait"
                && entry.phase != "statement_wait")
                || entry.sql_release_confirmed,
            std::cmp::Reverse(entry.elapsed_ms),
        )
    });
    let count = selected.len();
    let acquired_count = selected
        .iter()
        .filter(|entry| {
            entry.acquired_ms.is_some() && !entry.sql_release_confirmed
        })
        .count();
    let released_count = selected
        .iter()
        .filter(|entry| entry.sql_release_confirmed)
        .count();
    selected.truncate(MAX_CANDIDATES);
    (count, acquired_count, released_count, selected)
}

/// Retains the source of an active application operation. A scope can outlive
/// SQL COMMIT; `phase` distinguishes SQL work from later file publication.
/// Dropping it reports scope end, not proof that an asynchronous rollback or
/// another process's operating-system lock has completed.
#[derive(Debug)]
pub(crate) struct Scope {
    id: Uuid,
    database: String,
    started: Instant,
}

impl Scope {
    pub(crate) fn id(&self) -> Uuid {
        self.id
    }
    #[track_caller]
    pub(crate) fn new(pool: &SqlitePool, mode: &'static str) -> Self {
        let location = Location::caller();
        let scope = Self {
            id: Uuid::new_v4(),
            database: pool
                .connect_options()
                .get_filename()
                .display()
                .to_string(),
            started: Instant::now(),
        };
        let source = format!(
            "{}:{}:{}",
            location.file(),
            location.line(),
            location.column()
        );
        let span = tracing::Span::current()
            .metadata()
            .map(|metadata| metadata.name());
        registry()
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(
                scope.id,
                Entry {
                    database: scope.database.clone(),
                    source: source.clone(),
                    span,
                    mode,
                    phase: "created",
                    operation: None,
                    started: scope.started,
                    acquired: None,
                    sql_release_confirmed: false,
                },
            );
        tracing::debug!(target: "theseus::database_locks", operation_id = %scope.id,
            database = %scope.database, %source, ?span, mode, "Database diagnostic scope started");
        scope
    }

    pub(crate) fn label(&self, operation: &'static str) {
        if let Some(entry) = registry()
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get_mut(&self.id)
        {
            entry.operation = Some(operation);
        }
    }

    pub(crate) fn complete(&self, phase: &'static str) {
        self.phase(phase);
        if let Some(entry) = registry()
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get_mut(&self.id)
        {
            entry.sql_release_confirmed = true;
        }
    }

    pub(crate) fn phase(&self, phase: &'static str) {
        if let Some(entry) = registry()
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get_mut(&self.id)
        {
            entry.phase = phase;
        }
        tracing::debug!(target: "theseus::database_locks", operation_id = %self.id,
            database = %self.database, phase, elapsed_ms = self.started.elapsed().as_millis(),
            "Database diagnostic phase");
    }

    fn acquired(&self) {
        if let Some(entry) = registry()
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get_mut(&self.id)
        {
            entry.acquired = Some(Instant::now());
        }
        self.phase("transaction_active");
    }

    pub(crate) async fn observe<F: Future>(
        &self,
        phase: &'static str,
        future: F,
    ) -> F::Output {
        self.phase(phase);
        tokio::pin!(future);
        let delay = tokio::time::sleep(FIRST_REPORT);
        tokio::pin!(delay);
        loop {
            tokio::select! {
                result = &mut future => return result,
                _ = &mut delay => {
                    let (candidate_count, active_transactions, sql_released_scopes, active_candidates) = candidates(&self.database, self.id);
                    tracing::warn!(target: "theseus::database_locks", operation_id = %self.id,
                        database = %self.database, phase,
                        elapsed_ms = self.started.elapsed().as_millis(), candidate_count,
                        active_transactions, sql_released_scopes,
                        shown = active_candidates.len(),
                        "Database operation is waiting; registered operations are possible blockers, not confirmed OS lock owners");
                    for candidate in active_candidates {
                        tracing::warn!(target: "theseus::database_locks",
                            requester_operation_id = %self.id,
                            candidate_operation_id = %candidate.operation_id,
                            source = %candidate.source, span = candidate.span,
                            mode = candidate.mode, phase = candidate.phase,
                            operation = candidate.operation,
                            elapsed_ms = candidate.elapsed_ms,
                            acquired_ms = candidate.acquired_ms,
                            sql_release_confirmed = candidate.sql_release_confirmed,
                            "Database lock candidate");
                    }
                    delay.as_mut().reset(Instant::now() + REPORT_INTERVAL);
                }
            }
        }
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        let entry = registry()
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(&self.id);
        tracing::debug!(target: "theseus::database_locks", operation_id = %self.id,
            database = %self.database, phase = entry.as_ref().map(|entry| entry.phase),
            sql_release_confirmed = entry.as_ref().is_some_and(|entry| entry.sql_release_confirmed),
            elapsed_ms = self.started.elapsed().as_millis(),
            "Database diagnostic scope ended; an implicit rollback may still be pending on the SQLite worker");
    }
}

/// Keeps SQLx's original transaction type and exact BEGIN statement. The wait
/// includes pool acquisition, so it must not be labelled solely as lock wait.
#[track_caller]
pub(crate) fn begin<'a>(
    pool: &'a SqlitePool,
    statement: &'static str,
) -> impl Future<Output = Result<(Transaction<'static, Sqlite>, Scope), sqlx::Error>>
+ Send
+ 'a {
    let scope = Scope::new(pool, statement);
    async move {
        let result = if statement == "BEGIN" {
            scope.observe("pool_or_begin_wait", pool.begin()).await
        } else {
            scope
                .observe("pool_or_begin_wait", pool.begin_with(statement))
                .await
        };
        match result {
            Ok(transaction) => {
                scope.acquired();
                Ok((transaction, scope))
            }
            Err(error) => {
                scope.phase("begin_failed");
                tracing::warn!(target: "theseus::database_locks", operation_id = %scope.id,
                    database = %scope.database, %error, "Database transaction acquisition failed");
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn scope_drop_removes_an_operation_even_without_commit() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        let (_, scope) = begin(&pool, "BEGIN IMMEDIATE").await.unwrap();
        let id = scope.id;
        assert!(registry().lock().unwrap().contains_key(&id));
        drop(scope);
        assert!(!registry().lock().unwrap().contains_key(&id));
    }
    #[tokio::test]
    async fn snapshot_identifies_an_acquired_reader_and_its_source() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        let (tx, scope) = begin(&pool, "BEGIN").await.unwrap();
        let (count, _, _, records) = candidates(&scope.database, Uuid::nil());
        assert!(count >= 1);
        let entry = records
            .iter()
            .find(|entry| entry.operation_id == scope.id)
            .unwrap();
        assert_eq!(entry.mode, "BEGIN");
        assert!(entry.acquired_ms.is_some());
        assert!(entry.source.contains("lock_diagnostics.rs:"));
        tx.rollback().await.unwrap();
        scope.complete("rolled_back");
        drop(scope);
    }
    #[tokio::test]
    async fn blocked_writer_reports_the_registered_transaction_before_sqlite_times_out()
     {
        use std::io::Write;
        use std::sync::Arc;
        #[derive(Clone)]
        struct Logs(Arc<Mutex<Vec<u8>>>);
        impl Write for Logs {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let temporary = tempfile::tempdir().unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(2)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(temporary.path().join("diagnostic.db"))
                    .create_if_missing(true)
                    .busy_timeout(Duration::from_secs(5)),
            )
            .await
            .unwrap();
        let logs = Logs(Arc::new(Mutex::new(Vec::new())));
        let output = logs.clone();
        let subscriber = tracing_subscriber::fmt()
            .without_time()
            .with_ansi(false)
            .with_max_level(tracing::Level::DEBUG)
            .with_writer(move || output.clone())
            .finish();
        let _subscriber = tracing::subscriber::set_default(subscriber);
        let (held, holder) = begin(&pool, "BEGIN IMMEDIATE").await.unwrap();
        holder.label("held_writer_test");
        let holder_id = holder.id.to_string();
        let pending = begin(&pool, "BEGIN IMMEDIATE");
        tokio::pin!(pending);
        assert!(
            tokio::time::timeout(Duration::from_millis(2200), &mut pending)
                .await
                .is_err()
        );
        let text = String::from_utf8(logs.0.lock().unwrap().clone()).unwrap();
        assert!(
            text.contains("possible blockers"),
            "missing periodic warning: {text}"
        );
        assert!(text.contains("Database lock candidate"));
        assert!(text.contains("requester_operation_id="));
        assert!(text.contains(&format!("candidate_operation_id={holder_id}")));
        assert!(!text.contains("active_candidates="));
        assert!(text.contains("held_writer_test"));
        assert!(text.contains("transaction_active"));
        assert!(text.contains("lock_diagnostics.rs:"));
        held.rollback().await.unwrap();
        holder.complete("rolled_back");
        let (next, waiter) = pending.await.unwrap();
        next.rollback().await.unwrap();
        waiter.complete("rolled_back");
        drop(waiter);
        drop(holder);
        pool.close().await;
    }

    #[tokio::test]
    async fn acquired_transaction_is_shown_ahead_of_more_than_sixteen_waiters()
    {
        let temporary = tempfile::tempdir().unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(temporary.path().join("priority.db"))
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        let waiting: Vec<_> = (0..20)
            .map(|_| Scope::new(&pool, "BEGIN IMMEDIATE"))
            .collect();
        let holder = Scope::new(&pool, "BEGIN");
        holder.acquired();
        let (total, acquired, _, shown) =
            candidates(&holder.database, Uuid::nil());
        assert!(total >= 21);
        assert!(acquired >= 1);
        assert_eq!(shown[0].operation_id, holder.id);
        assert_eq!(shown.len(), MAX_CANDIDATES);
        drop(waiting);
    }

    #[tokio::test]
    async fn confirmed_sql_release_survives_a_later_file_publication_phase() {
        let temporary = tempfile::tempdir().unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(temporary.path().join("release.db"))
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        let scope = Scope::new(&pool, "BEGIN IMMEDIATE");
        scope.acquired();
        scope.complete("committed");
        scope.phase("file_publication");
        let (_, _, released, shown) = candidates(&scope.database, Uuid::nil());
        let entry = shown
            .iter()
            .find(|entry| entry.operation_id == scope.id)
            .unwrap();
        assert_eq!(entry.phase, "file_publication");
        assert!(entry.sql_release_confirmed);
        assert!(released >= 1);
    }

    #[tokio::test]
    async fn canceled_waiting_scope_is_removed() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        let scope = Scope::new(&pool, "statement");
        let id = scope.id;
        let future = async move {
            scope
                .observe("statement_wait", std::future::pending::<()>())
                .await
        };
        assert!(
            tokio::time::timeout(Duration::from_millis(1), future)
                .await
                .is_err()
        );
        assert!(!registry().lock().unwrap().contains_key(&id));
    }
}
