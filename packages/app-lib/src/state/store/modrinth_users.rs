//! The Modrinth account the launcher is signed in with.

use sqlx::SqliteConnection;

use super::Collection;

/// The Modrinth sessions, one per account.
pub(crate) struct ModrinthUsers;

/// A session is identified by the account it belongs to.
pub(crate) type ModrinthUserKey = String;

/// Which sessions a read wants: the active one, or the ones that are not.
pub(crate) type ModrinthUserScope = bool;

/// What a session carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ModrinthUserRecord {
    pub active: bool,
    pub session_id: String,
    pub expires: i64,
}

impl Collection for ModrinthUsers {
    type Key = ModrinthUserKey;
    type Scope = ModrinthUserScope;
    type Record = ModrinthUserRecord;

    /// The state a session is in lives in the record, so the collection cannot
    /// tell from the account id alone whether it is the active one.
    fn in_scope(_scope: &Self::Scope, _key: &Self::Key) -> bool {
        true
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row: Option<(bool, String, i64)> = sqlx::query_as(
            "SELECT active, session_id, expires
             FROM modrinth_users
             WHERE id = ?",
        )
        .bind(key)
        .fetch_optional(&mut *conn)
        .await?;

        Ok(row.map(|(active, session_id, expires)| Self::Record {
            active,
            session_id,
            expires,
        }))
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query("DELETE FROM modrinth_users WHERE id = ?")
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
        let rows: Vec<(String, bool, String, i64)> = sqlx::query_as(
            "SELECT id, active, session_id, expires FROM modrinth_users",
        )
        .fetch_all(&mut *conn)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(id, active, session_id, expires)| {
                (
                    id,
                    Self::Record {
                        active,
                        session_id,
                        expires,
                    },
                )
            })
            .collect())
    }

    async fn replace_all(
        conn: &mut SqliteConnection,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        ensure_one_active(records)?;
        delete_omitted(conn, None, records).await?;
        for (key, record) in records {
            write_row(conn, key, record).await?;
        }
        Ok(())
    }

    async fn read_scoped(
        conn: &mut SqliteConnection,
        scope: &Self::Scope,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let rows: Vec<(String, bool, String, i64)> = sqlx::query_as(
            "SELECT id, active, session_id, expires
             FROM modrinth_users
             WHERE active = ?",
        )
        .bind(*scope)
        .fetch_all(&mut *conn)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(id, active, session_id, expires)| {
                (
                    id,
                    Self::Record {
                        active,
                        session_id,
                        expires,
                    },
                )
            })
            .collect())
    }

    async fn replace_scoped(
        conn: &mut SqliteConnection,
        scope: &Self::Scope,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        ensure_one_active(records)?;
        ensure_records_are_in_scope(scope, records)?;
        ensure_keys_belong_to_scope(conn, scope, records).await?;
        delete_omitted(conn, Some(*scope), records).await?;
        for (key, record) in records {
            write_row(conn, key, record).await?;
        }
        Ok(())
    }
}

/// A session written into a scope is in the state that scope names: writing a
/// signed-in session into the idle group would clear the others and then take
/// the session away from where it belongs.
fn ensure_records_are_in_scope(
    scope: &ModrinthUserScope,
    records: &[(ModrinthUserKey, ModrinthUserRecord)],
) -> crate::Result<()> {
    for (key, record) in records {
        if record.active != *scope {
            return Err(crate::ErrorKind::InputError(format!(
                "Session {key} was written into a state it is not in"
            ))
            .into());
        }
    }

    Ok(())
}

/// A replacement writes by account, so a session stored in the other state
/// would be moved rather than replaced: its row is deleted here and rewritten
/// there, taking the signed-in session with it.
async fn ensure_keys_belong_to_scope(
    conn: &mut SqliteConnection,
    scope: &ModrinthUserScope,
    records: &[(ModrinthUserKey, ModrinthUserRecord)],
) -> crate::Result<()> {
    for (key, _) in records {
        let Some(existing) = ModrinthUsers::read(conn, key).await? else {
            continue;
        };
        if existing.active != *scope {
            return Err(crate::ErrorKind::InputError(format!(
                "Session {key} belongs to the other state"
            ))
            .into());
        }
    }

    Ok(())
}

/// The table carries one active session; it no longer has an index that says so,
/// so every write that marks a session active clears the others first.
async fn clear_other_active(
    conn: &mut SqliteConnection,
    key: &str,
) -> crate::Result<()> {
    sqlx::query("UPDATE modrinth_users SET active = 0 WHERE id <> ?")
        .bind(key)
        .execute(&mut *conn)
        .await?;

    Ok(())
}

/// At most one of the records a replacement carries may be the active one.
fn ensure_one_active(
    records: &[(ModrinthUserKey, ModrinthUserRecord)],
) -> crate::Result<()> {
    let active = records.iter().filter(|(_, record)| record.active).count();
    if active > 1 {
        return Err(crate::ErrorKind::InputError(
            "More than one Modrinth session was written as the active one"
                .to_string(),
        )
        .into());
    }

    Ok(())
}

async fn write_row(
    conn: &mut SqliteConnection,
    key: &str,
    record: &ModrinthUserRecord,
) -> crate::Result<()> {
    if record.active {
        clear_other_active(conn, key).await?;
    }

    sqlx::query(
        "
        INSERT INTO modrinth_users (id, active, session_id, expires)
        VALUES (?, ?, ?, ?)
        ON CONFLICT (id) DO UPDATE SET
            active = excluded.active,
            session_id = excluded.session_id,
            expires = excluded.expires
        ",
    )
    .bind(key)
    .bind(record.active)
    .bind(&record.session_id)
    .bind(record.expires)
    .execute(&mut *conn)
    .await?;

    Ok(())
}

/// Removes the sessions a replacement leaves out, and only those.
async fn delete_omitted(
    conn: &mut SqliteConnection,
    scope: Option<ModrinthUserScope>,
    records: &[(ModrinthUserKey, ModrinthUserRecord)],
) -> crate::Result<()> {
    let existing: Vec<String> = match scope {
        Some(active) => {
            sqlx::query_scalar("SELECT id FROM modrinth_users WHERE active = ?")
                .bind(active)
                .fetch_all(&mut *conn)
                .await?
        }
        None => {
            sqlx::query_scalar("SELECT id FROM modrinth_users")
                .fetch_all(&mut *conn)
                .await?
        }
    };

    for id in existing {
        if records.iter().any(|(key, _)| key == &id) {
            continue;
        }
        sqlx::query("DELETE FROM modrinth_users WHERE id = ?")
            .bind(&id)
            .execute(&mut *conn)
            .await?;
    }

    Ok(())
}
