//! The Minecraft accounts the launcher keeps, one row per identity.

use sqlx::Row;
use sqlx::SqliteConnection;
use sqlx::sqlite::SqliteRow;

use super::Collection;

/// The accounts, one per identity.
pub(crate) struct MinecraftUsers;

/// How an account signs in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AccountType {
    Microsoft,
    Offline,
    Yggdrasil,
}

impl AccountType {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Microsoft => "microsoft",
            Self::Offline => "offline",
            Self::Yggdrasil => "yggdrasil",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "microsoft" => Some(Self::Microsoft),
            "offline" => Some(Self::Offline),
            "yggdrasil" => Some(Self::Yggdrasil),
            _ => None,
        }
    }
}

/// What an account carries beyond the identity it is stored under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MinecraftUserRecord {
    pub uuid: String,
    pub active: bool,
    pub username: String,
    pub account_type: AccountType,
    pub access_token: String,
    pub refresh_token: String,
    pub expires: i64,
    pub yggdrasil_api_root: String,
    pub yggdrasil_server_name: String,
    pub yggdrasil_login: String,
    pub yggdrasil_client_token: String,
}

impl Collection for MinecraftUsers {
    type Key = String;
    type Scope = String;
    type Record = MinecraftUserRecord;

    fn in_scope(scope: &Self::Scope, key: &Self::Key) -> bool {
        scope == key
    }

    async fn read(
        conn: &mut SqliteConnection,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let row = sqlx::query(
            "
            SELECT
                account_id, uuid, active, username, account_type, access_token,
                refresh_token, expires, yggdrasil_api_root,
                yggdrasil_server_name, yggdrasil_login, yggdrasil_client_token
            FROM minecraft_users
            WHERE account_id = ?
            ",
        )
        .bind(key)
        .fetch_optional(&mut *conn)
        .await?;

        let user = row.map(to_record).transpose()?.flatten();

        Ok(user.map(|(_, record)| record))
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query("DELETE FROM minecraft_users WHERE account_id = ?")
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
        let rows = sqlx::query(
            "
            SELECT
                account_id, uuid, active, username, account_type, access_token,
                refresh_token, expires, yggdrasil_api_root,
                yggdrasil_server_name, yggdrasil_login, yggdrasil_client_token
            FROM minecraft_users
            ",
        )
        .fetch_all(&mut *conn)
        .await?;

        rows.into_iter()
            .filter_map(|row| to_record(row).transpose())
            .collect::<crate::Result<Vec<_>>>()
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
        ensure_one_active(records)?;
        delete_omitted(conn, Some(scope), records).await?;
        for (key, record) in records {
            write_row(conn, key, record).await?;
        }
        Ok(())
    }
}

/// The table says one account is active; it no longer has an index that says so,
/// so every write that marks an account active clears the others first.
async fn clear_other_active(
    conn: &mut SqliteConnection,
    key: &str,
) -> crate::Result<()> {
    sqlx::query("UPDATE minecraft_users SET active = 0 WHERE account_id <> ?")
        .bind(key)
        .execute(&mut *conn)
        .await?;

    Ok(())
}

/// At most one of the records a replacement carries may be the active one.
fn ensure_one_active(
    records: &[(<MinecraftUsers as Collection>::Key, MinecraftUserRecord)],
) -> crate::Result<()> {
    let active = records.iter().filter(|(_, record)| record.active).count();
    if active > 1 {
        return Err(crate::ErrorKind::InputError(
            "More than one account was written as the active one".to_string(),
        )
        .into());
    }

    Ok(())
}

async fn write_row(
    conn: &mut SqliteConnection,
    account_id: &str,
    record: &MinecraftUserRecord,
) -> crate::Result<()> {
    if record.active {
        clear_other_active(conn, account_id).await?;
    }

    sqlx::query(
        "
        INSERT INTO minecraft_users (
            account_id, uuid, active, username, account_type, access_token,
            refresh_token, expires, yggdrasil_api_root, yggdrasil_server_name,
            yggdrasil_login, yggdrasil_client_token
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT (account_id) DO UPDATE SET
            uuid = excluded.uuid,
            active = excluded.active,
            username = excluded.username,
            account_type = excluded.account_type,
            access_token = excluded.access_token,
            refresh_token = excluded.refresh_token,
            expires = excluded.expires,
            yggdrasil_api_root = excluded.yggdrasil_api_root,
            yggdrasil_server_name = excluded.yggdrasil_server_name,
            yggdrasil_login = excluded.yggdrasil_login,
            yggdrasil_client_token = excluded.yggdrasil_client_token
        ",
    )
    .bind(account_id)
    .bind(&record.uuid)
    .bind(record.active)
    .bind(&record.username)
    .bind(record.account_type.as_str())
    .bind(&record.access_token)
    .bind(&record.refresh_token)
    .bind(record.expires)
    .bind(&record.yggdrasil_api_root)
    .bind(&record.yggdrasil_server_name)
    .bind(&record.yggdrasil_login)
    .bind(&record.yggdrasil_client_token)
    .execute(&mut *conn)
    .await?;

    Ok(())
}

/// Removes the accounts the replacement leaves out, and only those.
async fn delete_omitted(
    conn: &mut SqliteConnection,
    scope: Option<&String>,
    records: &[(String, MinecraftUserRecord)],
) -> crate::Result<()> {
    let existing: Vec<String> =
        match scope {
            Some(account_id) => sqlx::query_scalar(
                "SELECT account_id FROM minecraft_users WHERE account_id = ?",
            )
            .bind(account_id)
            .fetch_all(&mut *conn)
            .await?,
            None => {
                sqlx::query_scalar("SELECT account_id FROM minecraft_users")
                    .fetch_all(&mut *conn)
                    .await?
            }
        };

    for account_id in existing {
        if records.iter().any(|(key, _)| key == &account_id) {
            continue;
        }
        sqlx::query("DELETE FROM minecraft_users WHERE account_id = ?")
            .bind(&account_id)
            .execute(&mut *conn)
            .await?;
    }

    Ok(())
}

fn to_record(
    row: SqliteRow,
) -> crate::Result<Option<(String, MinecraftUserRecord)>> {
    let account_type: String = row.try_get("account_type")?;
    let Some(account_type) = to_account_type(&account_type) else {
        return Ok(None);
    };

    Ok(Some((
        row.try_get("account_id")?,
        MinecraftUserRecord {
            uuid: row.try_get("uuid")?,
            active: row.try_get("active")?,
            username: row.try_get("username")?,
            account_type,
            access_token: row.try_get("access_token")?,
            refresh_token: row.try_get("refresh_token")?,
            expires: row.try_get("expires")?,
            yggdrasil_api_root: row.try_get("yggdrasil_api_root")?,
            yggdrasil_server_name: row.try_get("yggdrasil_server_name")?,
            yggdrasil_login: row.try_get("yggdrasil_login")?,
            yggdrasil_client_token: row.try_get("yggdrasil_client_token")?,
        },
    )))
}

/// The type a stored account names, or nothing when this build does not know
/// it: an account is only useful while the type its identity was built from can
/// be read.
fn to_account_type(account_type: &str) -> Option<AccountType> {
    AccountType::parse(account_type).or_else(|| {
        tracing::warn!(
            %account_type,
            "Skipping an account whose type this build does not know"
        );
        None
    })
}

/// The token pairs of every account that is not offline, whether or not this
/// build can read the row as an account: a token it cannot attribute is still a
/// secret that has to be hidden.
pub(crate) async fn token_pairs(
    handle: &super::Handle,
) -> crate::Result<Vec<(String, String)>> {
    let mut connection = handle.connection().await?;
    let rows = sqlx::query_as(
        "SELECT access_token, refresh_token
         FROM minecraft_users
         WHERE account_type <> 'offline'",
    )
    .fetch_all(&mut *connection)
    .await?;

    Ok(rows)
}
