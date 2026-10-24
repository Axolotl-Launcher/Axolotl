use super::super::synced_options::{
    nbt_from_bytes, nbt_to_bytes, safe_instance_id, synced_options_path,
};
use super::SERVERS_FILE;
use super::limits::validate_size;
use super::types::{
    CanonicalServer, LocalServer, ProjectionEntry, ProjectionOwner,
    ServerRecord, ServerSource,
};
use crate::{ErrorKind, State};
use sqlx::{Sqlite, SqliteConnection, Transaction};
use std::collections::HashSet;
use std::path::PathBuf;

pub(in crate::api::instance) async fn canonical_exists(
    state: &State,
) -> crate::Result<bool> {
    canonical_initialized(state).await
}

async fn canonical_initialized(state: &State) -> crate::Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"
		SELECT EXISTS(
			SELECT 1 FROM synced_server_state WHERE singleton = 1
		) AS "initialized!: bool"
		"#,
    )
    .fetch_one(&state.pool)
    .await?)
}

pub(super) async fn read_canonical(
    state: &State,
) -> crate::Result<Vec<CanonicalServer>> {
    read_canonical_with(&mut *state.pool.acquire().await?).await
}

async fn read_canonical_with(
    executor: &mut SqliteConnection,
) -> crate::Result<Vec<CanonicalServer>> {
    validate_stored_list(executor, "synced_servers", None).await?;
    let rows = sqlx::query!(
        "
		SELECT id, nbt
		FROM synced_servers
		ORDER BY position
		",
    )
    .fetch_all(executor)
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(CanonicalServer {
                id: row.id,
                data: nbt_from_bytes(row.nbt)?,
            })
        })
        .collect()
}

/// Reads canonical servers and instance overrides from the same committed revision.
pub(super) async fn read_server_snapshot(
    instance_id: &str,
    state: &State,
) -> crate::Result<(Vec<CanonicalServer>, Vec<LocalServer>)> {
    let mut tx = state.pool.begin().await?;
    let canonical = read_canonical_with(&mut *tx).await?;
    let local = load_local_with(instance_id, &mut *tx).await?;
    tx.commit().await?;
    Ok((canonical, local))
}

pub(super) async fn commit_server_state(
    canonical: Option<&[CanonicalServer]>,
    local: Option<(&str, &[LocalServer])>,
    state: &State,
) -> crate::Result<bool> {
    let mut tx = state.pool.begin().await?;
    let canonical_changed =
        write_server_state(&mut tx, canonical, local).await?;
    tx.commit().await?;
    Ok(canonical_changed)
}

/// A consumed file and its database changes must survive a failed fan-out together.
/// Pending checkpoints retry projection, never import the same change again.
pub(super) async fn commit_captured_servers(
    instance_id: &str,
    canonical: &[CanonicalServer],
    local: &[LocalServer],
    state: &State,
) -> crate::Result<bool> {
    let mut tx = state.pool.begin().await?;
    let changed = write_server_state(
        &mut tx,
        Some(canonical),
        Some((instance_id, local)),
    )
    .await?;
    sqlx::query(
        "INSERT INTO instance_sync_checkpoints
         (instance_id, feature, variant, expected_sha1, source_revision, status)
         VALUES (?, 'multiplayer_servers', 'default', '',
                 (SELECT revision FROM synced_server_state WHERE singleton = 1), 'pending')
         ON CONFLICT(instance_id, feature, variant) DO UPDATE SET
             expected_sha1 = '', source_revision = excluded.source_revision,
             status = 'pending', link_mode = NULL, merge_base = NULL"
    ).bind(instance_id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(changed)
}

async fn write_server_state(
    tx: &mut Transaction<'_, Sqlite>,
    canonical: Option<&[CanonicalServer]>,
    local: Option<(&str, &[LocalServer])>,
) -> crate::Result<bool> {
    let mut canonical_changed = false;
    if let Some(canonical) = canonical {
        canonical_changed = write_canonical_rows(tx, canonical).await?;
        sqlx::query!(
            "
			INSERT INTO synced_server_state (singleton, revision)
			VALUES (1, 0)
			ON CONFLICT(singleton) DO NOTHING
			",
        )
        .execute(&mut **tx)
        .await?;
        if canonical_changed {
            sqlx::query!(
                "
				UPDATE synced_server_state
				SET revision = revision + 1
				WHERE singleton = 1
				",
            )
            .execute(&mut **tx)
            .await?;
        }
    }
    if let Some((instance_id, local)) = local {
        write_local_rows(tx, instance_id, local).await?;
    }
    Ok(canonical_changed)
}

async fn validate_stored_list(
    connection: &mut SqliteConnection,
    table: &str,
    instance_id: Option<&str>,
) -> crate::Result<()> {
    // Table names are private constants supplied by the storage functions below.
    let filter = if instance_id.is_some() {
        " WHERE instance_id = ?"
    } else {
        ""
    };
    let query = format!(
        "SELECT COUNT(*), COALESCE(SUM(length(nbt)), 0) FROM {table}{filter}"
    );
    let mut query = sqlx::query_as::<_, (i64, i64)>(&query);
    if let Some(id) = instance_id {
        query = query.bind(id);
    }
    let (count, bytes) = query.fetch_one(connection).await?;
    validate_size(
        count.try_into().unwrap_or(usize::MAX),
        bytes.try_into().unwrap_or(usize::MAX),
    )
}

async fn write_canonical_rows(
    tx: &mut Transaction<'_, Sqlite>,
    servers: &[CanonicalServer],
) -> crate::Result<bool> {
    validate_size(servers.len(), 0)?;
    validate_stored_list(&mut **tx, "synced_servers", None).await?;
    let current = sqlx::query!(
        "
		SELECT id, position, nbt
		FROM synced_servers
		ORDER BY position
		",
    )
    .fetch_all(&mut **tx)
    .await?;
    let mut desired = Vec::with_capacity(servers.len());
    let mut bytes = 0;
    for (position, server) in servers.iter().enumerate() {
        let nbt = nbt_to_bytes(&server.data)?;
        bytes += nbt.len();
        validate_size(servers.len(), bytes)?;
        desired.push((server.id.as_str(), position as i64, nbt));
    }
    let mut unchanged = current.len() == desired.len();
    if unchanged {
        for ((current, desired), server) in
            current.iter().zip(&desired).zip(servers)
        {
            // Compound field order is not meaningful; quartz_nbt uses HashMap.
            // Comparing serialized bytes marks unchanged lists as new revisions.
            if current.id != desired.0
                || current.position != desired.1
                || nbt_from_bytes(current.nbt.clone())? != server.data
            {
                unchanged = false;
                break;
            }
        }
    }
    if unchanged {
        return Ok(false);
    }

    let desired_ids =
        desired.iter().map(|(id, _, _)| *id).collect::<HashSet<_>>();
    for row in &current {
        if !desired_ids.contains(row.id.as_str()) {
            sqlx::query!("DELETE FROM synced_servers WHERE id = ?", row.id)
                .execute(&mut **tx)
                .await?;
        }
    }

    let position_offset = current
        .iter()
        .map(|row| row.position)
        .max()
        .unwrap_or(0)
        .max(servers.len() as i64)
        .saturating_add(1);
    sqlx::query!(
        "UPDATE synced_servers SET position = position + ?",
        position_offset,
    )
    .execute(&mut **tx)
    .await?;

    for (id, position, nbt) in desired {
        sqlx::query!(
            "
			INSERT INTO synced_servers (id, position, nbt)
			VALUES (?, ?, ?)
			ON CONFLICT(id) DO UPDATE SET
				position = excluded.position,
				nbt = excluded.nbt
			",
            id,
            position,
            nbt,
        )
        .execute(&mut **tx)
        .await?;
    }
    Ok(true)
}

pub(super) async fn server_revision(state: &State) -> crate::Result<i64> {
    Ok(sqlx::query_scalar!(
        r#"
		SELECT revision AS "revision!: i64"
		FROM synced_server_state
		WHERE singleton = 1
		"#,
    )
    .fetch_optional(&state.pool)
    .await?
    .unwrap_or(0))
}

pub(super) async fn load_local(
    instance_id: &str,
    state: &State,
) -> crate::Result<Vec<LocalServer>> {
    load_local_with(instance_id, &mut *state.pool.acquire().await?).await
}

async fn load_local_with(
    instance_id: &str,
    executor: &mut SqliteConnection,
) -> crate::Result<Vec<LocalServer>> {
    validate_stored_list(executor, "instance_servers", Some(instance_id))
        .await?;
    let rows = sqlx::query!(
        "
		SELECT id, source, excluded_synced_server_id, nbt, position
		FROM instance_servers
		WHERE instance_id = ?
		ORDER BY position
		",
        instance_id,
    )
    .fetch_all(executor)
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(LocalServer {
                id: row.id,
                source: ServerSource::from_str(&row.source).ok_or_else(
                    || {
                        ErrorKind::InputError(format!(
                            "Unknown server source {}",
                            row.source
                        ))
                    },
                )?,
                excluded_synced_server_id: row.excluded_synced_server_id,
                data: nbt_from_bytes(row.nbt)?,
                position: row.position,
            })
        })
        .collect()
}

pub(super) async fn write_local(
    instance_id: &str,
    servers: &[LocalServer],
    state: &State,
) -> crate::Result<()> {
    commit_server_state(None, Some((instance_id, servers)), state).await?;
    Ok(())
}

pub(super) async fn write_local_rows(
    tx: &mut Transaction<'_, Sqlite>,
    instance_id: &str,
    servers: &[LocalServer],
) -> crate::Result<()> {
    validate_size(servers.len(), 0)?;
    let mut bytes = 0;
    sqlx::query!(
        "DELETE FROM instance_servers WHERE instance_id = ?",
        instance_id,
    )
    .execute(&mut **tx)
    .await?;
    for server in servers {
        let source = server.source.as_str();
        let nbt = nbt_to_bytes(&server.data)?;
        bytes += nbt.len();
        validate_size(servers.len(), bytes)?;
        sqlx::query!(
            "
			INSERT INTO instance_servers
				(instance_id, id, source, excluded_synced_server_id,
				 nbt, position)
			VALUES (?, ?, ?,
				(SELECT id FROM synced_servers WHERE id = ?), ?, ?)
			",
            instance_id,
            server.id,
            source,
            server.excluded_synced_server_id,
            nbt,
            server.position,
        )
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

pub(super) async fn load_projection_entries(
    instance_id: &str,
    state: &State,
) -> crate::Result<Vec<ProjectionEntry>> {
    let mut connection = state.pool.acquire().await?;
    validate_stored_list(
        &mut connection,
        "instance_server_projection_entries",
        Some(instance_id),
    )
    .await?;
    let rows = sqlx::query!(
        "
		SELECT server_id, owner, nbt, position
		FROM instance_server_projection_entries
		WHERE instance_id = ?
		ORDER BY position
		",
        instance_id,
    )
    .fetch_all(&mut *connection)
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(ProjectionEntry {
                id: row.server_id,
                owner: ProjectionOwner::from_str(&row.owner).ok_or_else(
                    || {
                        ErrorKind::InputError(format!(
                            "Unknown server projection owner {}",
                            row.owner
                        ))
                    },
                )?,
                data: nbt_from_bytes(row.nbt)?,
                position: row.position,
            })
        })
        .collect()
}

pub(super) async fn begin_server_checkpoint(
    instance_id: &str,
    servers: &[ServerRecord],
    expected_sha1: &str,
    source_revision: i64,
    state: &State,
) -> crate::Result<()> {
    validate_size(servers.len(), 0)?;
    let mut bytes = 0;
    let mut tx = state.pool.begin().await?;
    sqlx::query!(
        "DELETE FROM instance_server_projection_entries WHERE instance_id = ?",
        instance_id,
    )
    .execute(&mut *tx)
    .await?;
    for (position, server) in servers.iter().enumerate() {
        let owner = if server.source == ServerSource::UserSynced {
            ProjectionOwner::Synced
        } else {
            ProjectionOwner::Instance
        }
        .as_str();
        let nbt = nbt_to_bytes(&server.data)?;
        bytes += nbt.len();
        validate_size(servers.len(), bytes)?;
        let position = position as i64;
        sqlx::query!(
            "
			INSERT INTO instance_server_projection_entries
				(instance_id, owner, server_id, nbt, position)
			VALUES (?, ?, ?, ?, ?)
			",
            instance_id,
            owner,
            server.id,
            nbt,
            position,
        )
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query!(
        "
		INSERT INTO instance_sync_checkpoints
			(instance_id, feature, variant, expected_sha1, merge_base,
			 source_revision, status, link_mode)
		VALUES (?, 'multiplayer_servers', 'default', ?, NULL, ?,
			'pending', NULL)
		ON CONFLICT(instance_id, feature, variant) DO UPDATE SET
			expected_sha1 = excluded.expected_sha1,
			merge_base = NULL,
			source_revision = excluded.source_revision,
			status = 'pending',
			link_mode = NULL
		",
        instance_id,
        expected_sha1,
        source_revision,
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

pub(super) fn generated_path(state: &State, instance_id: &str) -> PathBuf {
    synced_options_path(state)
        .join("servers/generated")
        .join(safe_instance_id(instance_id))
        .join(SERVERS_FILE)
}
