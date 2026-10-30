//! The dependency graph a content entry was installed with.

use sqlx::SqliteConnection;

use super::Collection;

/// The recorded graphs, one per content entry.
pub(crate) struct ResourceEntryGraphs;

/// A graph is identified by the content entry it belongs to.
pub(crate) type EntryGraphKey = String;

/// What a graph carries.
///
/// The graph is kept as the text it is stored as: what the text says is the
/// caller's business.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EntryGraphRecord {
    pub nodes: String,
    pub edges: String,
}

impl Collection for ResourceEntryGraphs {
    type Key = EntryGraphKey;
    type Scope = EntryGraphKey;
    type Record = EntryGraphRecord;

    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool {
        scope == key
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row: Option<(String, String)> = sqlx::query_as(
            "
            SELECT nodes, edges
            FROM resource_entry_graphs
            WHERE entry_id = ?
            ",
        )
        .bind(key)
        .fetch_optional(&mut *conn)
        .await?;

        Ok(row.map(|(nodes, edges)| Self::Record { nodes, edges }))
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query("DELETE FROM resource_entry_graphs WHERE entry_id = ?")
                .bind(key)
                .execute(&mut *conn)
                .await?;

            return Ok(());
        };

        write_row(conn, key, record).await
    }

    async fn read_all(
        conn: &mut SqliteConnection,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let rows: Vec<(String, String, String)> = sqlx::query_as(
            "
            SELECT entry_id, nodes, edges
            FROM resource_entry_graphs
            ",
        )
        .fetch_all(&mut *conn)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(entry_id, nodes, edges)| {
                (entry_id, Self::Record { nodes, edges })
            })
            .collect())
    }

    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        sqlx::query("DELETE FROM resource_entry_graphs")
            .execute(&mut *conn)
            .await?;
        for (key, record) in records {
            write_row(conn, key, record).await?;
        }
        Ok(())
    }

    async fn read_scoped(
        conn: &mut SqliteConnection,
        scope: &Self::Scope,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        Ok(Self::read(conn, scope)
            .await?
            .map(|record| vec![(scope.clone(), record)])
            .unwrap_or_default())
    }

    async fn replace_scoped(
        conn: &mut SqliteConnection,
        scope: &Self::Scope,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        Self::write(conn, scope, records.first().map(|(_, record)| record))
            .await
    }
}

impl ResourceEntryGraphs {
    /// Puts back a graph a history entry recorded, leaving a missing row
    /// absent.
    pub(crate) async fn restore_in(
        conn: &mut SqliteConnection,
        key: &str,
        record: &EntryGraphRecord,
    ) -> crate::Result<()> {
        sqlx::query(
            "
            UPDATE resource_entry_graphs
            SET nodes = ?, edges = ?
            WHERE entry_id = ?
            ",
        )
        .bind(&record.nodes)
        .bind(&record.edges)
        .bind(key)
        .execute(&mut *conn)
        .await?;

        Ok(())
    }

    /// Drops the graph an entry no longer has.
    pub(crate) async fn delete_in(
        conn: &mut SqliteConnection,
        key: &str,
    ) -> crate::Result<()> {
        let key = key.to_owned();
        Self::write(conn, &key, None).await
    }
}

async fn write_row(
    conn: &mut SqliteConnection,
    key: &str,
    record: &EntryGraphRecord,
) -> crate::Result<()> {
    sqlx::query(
        "
        INSERT INTO resource_entry_graphs (entry_id, nodes, edges)
        VALUES (?, ?, ?)
        ON CONFLICT(entry_id) DO UPDATE SET
            nodes = excluded.nodes,
            edges = excluded.edges
        ",
    )
    .bind(key)
    .bind(&record.nodes)
    .bind(&record.edges)
    .execute(&mut *conn)
    .await?;

    Ok(())
}
