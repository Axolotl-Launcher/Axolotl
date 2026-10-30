//! The Minecraft accounts the launcher keeps, one row per identity.
//!
//! The tokens of an account are kept in the system credential store, and the
//! row's `secrets_externalized` says which of its token columns the store holds;
//! a token the store would not take stays in its column.

use sqlx::Row;
use sqlx::SqliteConnection;
use sqlx::sqlite::SqliteRow;

use super::Collection;
use super::secrets::{self, Stored, Undo};

/// The accounts, one per identity.
pub(crate) struct MinecraftUsers;

/// The bits of `secrets_externalized` that say which token the store holds, in
/// the order the token columns are declared.
const ACCESS_TOKEN_KEPT: i64 = 1;
const REFRESH_TOKEN_KEPT: i64 = 1 << 1;
const CLIENT_TOKEN_KEPT: i64 = 1 << 2;

/// The token columns of a row, each with the bit of its mark.
const TOKENS: [(i64, &str); 3] = [
    (ACCESS_TOKEN_KEPT, "access_token"),
    (REFRESH_TOKEN_KEPT, "refresh_token"),
    (CLIENT_TOKEN_KEPT, "yggdrasil_client_token"),
];

/// Where one account's token is stored.
fn token_key(account_id: &str, column: &str) -> String {
    secrets::key(&["minecraft-user", account_id, column])
}

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

/// What one row says about the tokens it names: which of them the store holds,
/// and which still sit in their columns.
#[derive(Clone, PartialEq, Eq)]
struct Marks {
    mark: i64,
    columns: [String; 3],
}

impl Marks {
    fn of(row: &SqliteRow) -> crate::Result<Self> {
        let mut columns = [String::new(), String::new(), String::new()];
        for (index, (_, column)) in TOKENS.into_iter().enumerate() {
            columns[index] = row.try_get(column)?;
        }

        Ok(Self {
            mark: row.try_get("secrets_externalized")?,
            columns,
        })
    }
}

/// What the row says now, when it says something else than `heard`.
///
/// A write that commits while this read waits leaves the row naming other
/// tokens, and the two secrets of one account must not come from two versions of
/// it.
async fn changed(
    conn: &mut SqliteConnection,
    account_id: &str,
    heard: &Marks,
) -> crate::Result<Option<Marks>> {
    let row = sqlx::query(
        "
        SELECT access_token, refresh_token, yggdrasil_client_token,
               secrets_externalized
        FROM minecraft_users
        WHERE account_id = ?
        ",
    )
    .bind(account_id)
    .fetch_optional(&mut *conn)
    .await?;

    let Some(row) = row else {
        return Ok(None);
    };
    let now = Marks::of(&row)?;

    Ok((now != *heard).then_some(now))
}

/// Writes back what the row should say now that its columns are settled.
async fn write_marks(
    conn: &mut SqliteConnection,
    account_id: &str,
    marks: &Marks,
) -> crate::Result<()> {
    sqlx::query(
        "
        UPDATE minecraft_users
        SET access_token = ?, refresh_token = ?,
            yggdrasil_client_token = ?, secrets_externalized = ?
        WHERE account_id = ?
        ",
    )
    .bind(&marks.columns[0])
    .bind(&marks.columns[1])
    .bind(&marks.columns[2])
    .bind(marks.mark)
    .bind(account_id)
    .execute(&mut *conn)
    .await?;

    Ok(())
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
                yggdrasil_server_name, yggdrasil_login, yggdrasil_client_token,
                secrets_externalized
            FROM minecraft_users
            WHERE account_id = ?
            ",
        )
        .bind(key)
        .fetch_optional(&mut *conn)
        .await?;

        let user = match row {
            Some(row) => to_record(conn, &row).await?,
            None => None,
        };

        Ok(user.map(|(_, record)| record))
    }

    async fn write(
        conn: &mut SqliteConnection,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        match record {
            Some(record) => write_row(conn, key, record).await,
            None => remove_row(conn, key).await,
        }
    }

    async fn read_all(
        conn: &mut SqliteConnection,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let rows = sqlx::query(
            "
            SELECT
                account_id, uuid, active, username, account_type, access_token,
                refresh_token, expires, yggdrasil_api_root,
                yggdrasil_server_name, yggdrasil_login, yggdrasil_client_token,
                secrets_externalized
            FROM minecraft_users
            ",
        )
        .fetch_all(&mut *conn)
        .await?;

        let mut users = Vec::with_capacity(rows.len());
        for row in rows {
            if let Some(user) = to_record(conn, &row).await? {
                users.push(user);
            }
        }

        Ok(users)
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

    // The store is written first, so a row only says a token was moved once the
    // token is there. What the store held before comes back if the row below
    // cannot be written, which would otherwise leave the two disagreeing about
    // which secret is the account's.
    let mut undo = Undo::default();
    let marks = keep_tokens(
        account_id,
        [
            record.access_token.as_str(),
            record.refresh_token.as_str(),
            record.yggdrasil_client_token.as_str(),
        ],
        &mut undo,
    )
    .await?;
    let moved = marks.is_some();
    let (access_token, refresh_token, client_token) = if moved {
        ("", "", "")
    } else {
        (
            record.access_token.as_str(),
            record.refresh_token.as_str(),
            record.yggdrasil_client_token.as_str(),
        )
    };

    let written = sqlx::query(
        "
        INSERT INTO minecraft_users (
            account_id, uuid, active, username, account_type, access_token,
            refresh_token, expires, yggdrasil_api_root, yggdrasil_server_name,
            yggdrasil_login, yggdrasil_client_token, secrets_externalized
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
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
            yggdrasil_client_token = excluded.yggdrasil_client_token,
            secrets_externalized = excluded.secrets_externalized
        ",
    )
    .bind(account_id)
    .bind(&record.uuid)
    .bind(record.active)
    .bind(&record.username)
    .bind(record.account_type.as_str())
    .bind(access_token)
    .bind(refresh_token)
    .bind(record.expires)
    .bind(&record.yggdrasil_api_root)
    .bind(&record.yggdrasil_server_name)
    .bind(&record.yggdrasil_login)
    .bind(client_token)
    .bind(marks.unwrap_or_default())
    .execute(&mut *conn)
    .await;

    match written {
        Ok(_) => Ok(()),
        Err(error) => {
            undo.restore().await;
            Err(error.into())
        }
    }
}

/// Removes one account and the tokens it kept in the store.
///
/// A token the store will not drop keeps the row: deleting it would leave the
/// token behind with nothing left to name it.
async fn remove_row(
    conn: &mut SqliteConnection,
    account_id: &str,
) -> crate::Result<()> {
    let mark: i64 = sqlx::query_scalar(
        "SELECT secrets_externalized FROM minecraft_users WHERE account_id = ?",
    )
    .bind(account_id)
    .fetch_optional(&mut *conn)
    .await?
    .unwrap_or_default();

    let mut undo = Undo::default();
    if let Err(error) = forget_tokens(account_id, mark, &mut undo).await {
        undo.restore().await;
        return Err(error);
    }

    let removed =
        sqlx::query("DELETE FROM minecraft_users WHERE account_id = ?")
            .bind(account_id)
            .execute(&mut *conn)
            .await;

    match removed {
        Ok(_) => Ok(()),
        Err(error) => {
            undo.restore().await;
            Err(error.into())
        }
    }
}

/// Drops the tokens of one account from the store, keeping what they held so a
/// row that stays can be read again.
async fn forget_tokens(
    account_id: &str,
    mark: i64,
    undo: &mut Undo,
) -> crate::Result<()> {
    for (bit, column) in TOKENS {
        undo.drop(&token_key(account_id, column), mark & bit != 0)
            .await?;
    }

    Ok(())
}

/// Puts the tokens of one account in the store and says which of them it holds.
///
/// Nothing means at least one token is only in the row, which the caller writes
/// as an ordinary row: a store that takes two of three tokens has still lost
/// none of them, and the next write tries the rest again. A token that ends up
/// in the row is a token the store no longer holds, so its entry is dropped.
async fn keep_tokens(
    account_id: &str,
    tokens: [&str; 3],
    undo: &mut Undo,
) -> crate::Result<Option<i64>> {
    let mut marks = 0;

    for ((bit, column), token) in TOKENS.into_iter().zip(tokens) {
        let key = token_key(account_id, column);

        if token.is_empty() {
            // An account without a token has nothing to hide, and an entry the
            // store still holds for it would be a secret nothing names.
            undo.drop(&key, false).await?;
            continue;
        }

        undo.watch(&key).await;
        if !secrets::keep(key, token).await {
            for (_, column) in TOKENS {
                undo.drop(&token_key(account_id, column), false).await?;
            }
            return Ok(None);
        }
        marks |= bit;
    }

    Ok(Some(marks))
}

/// The account's tokens, each read from the store where the row's mark says the
/// store holds it.
///
/// The row is read again once the store has answered: a write that committed
/// while this read waited leaves the row saying something else, and the tokens
/// are then resolved once more from what it says now. A row still changing after
/// that is read as it says latest.
async fn tokens(
    conn: &mut SqliteConnection,
    account_id: &str,
    row: &SqliteRow,
) -> crate::Result<[String; 3]> {
    let heard = Marks::of(row)?;
    let (values, settled) = resolved(conn, account_id, heard).await?;

    let Some(latest) = changed(conn, account_id, &settled).await? else {
        return Ok(values);
    };

    Ok(resolved(conn, account_id, latest).await?.0)
}

/// What one row's tokens read as, and what the row should say afterwards.
///
/// A column that still holds a token was written by a build that did not move
/// it, so the column is the newer of the two and is what the account reads as;
/// the token is moved into the store here, which is what settles the two.
async fn resolved(
    conn: &mut SqliteConnection,
    account_id: &str,
    marks: Marks,
) -> crate::Result<([String; 3], Marks)> {
    let mut values = [String::new(), String::new(), String::new()];
    let mut settled = marks.clone();
    let mut repaired = false;

    for (index, (bit, column)) in TOKENS.into_iter().enumerate() {
        if marks.mark & bit == 0 {
            values[index] = marks.columns[index].clone();
            continue;
        }

        let column_value = &marks.columns[index];
        if column_value.is_empty() {
            values[index] =
                secrets::resolve(token_key(account_id, column)).await?;
            continue;
        }

        values[index] = column_value.clone();
        repaired = true;
        if secrets::keep(token_key(account_id, column), column_value).await {
            settled.columns[index] = String::new();
        } else {
            // The store would not take it, so the row says so and keeps the
            // token in its column.
            settled.mark &= !bit;
        }
    }

    if repaired {
        write_marks(conn, account_id, &settled).await?;
    }

    Ok((values, settled))
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
        remove_row(conn, &account_id).await?;
    }

    Ok(())
}

async fn to_record(
    conn: &mut SqliteConnection,
    row: &SqliteRow,
) -> crate::Result<Option<(String, MinecraftUserRecord)>> {
    let account_type: String = row.try_get("account_type")?;
    let Some(account_type) = to_account_type(&account_type) else {
        return Ok(None);
    };

    let account_id: String = row.try_get("account_id")?;
    let [access_token, refresh_token, yggdrasil_client_token] =
        tokens(conn, &account_id, row).await?;

    Ok(Some((
        account_id,
        MinecraftUserRecord {
            uuid: row.try_get("uuid")?,
            active: row.try_get("active")?,
            username: row.try_get("username")?,
            account_type,
            access_token,
            refresh_token,
            expires: row.try_get("expires")?,
            yggdrasil_api_root: row.try_get("yggdrasil_api_root")?,
            yggdrasil_server_name: row.try_get("yggdrasil_server_name")?,
            yggdrasil_login: row.try_get("yggdrasil_login")?,
            yggdrasil_client_token,
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
    let rows = sqlx::query(
        "SELECT account_id, access_token, refresh_token, secrets_externalized
         FROM minecraft_users
         WHERE account_type <> 'offline'",
    )
    .fetch_all(&mut *connection)
    .await?;

    let mut pairs = Vec::with_capacity(rows.len());
    for row in rows {
        let account_id: String = row.try_get("account_id")?;
        let [access_token, refresh_token, _] =
            tokens(&mut connection, &account_id, &row).await?;
        pairs.push((access_token, refresh_token));
    }

    Ok(pairs)
}

/// Moves the tokens these rows still carry into the credential store, and clears
/// the columns whose token is there.
///
/// Runs once at startup, and again after any run that could not move a row. A row
/// whose tokens are all in the store has nothing left in its columns, so a second
/// pass has nothing to do.
pub(crate) async fn externalize(
    conn: &mut SqliteConnection,
) -> crate::Result<()> {
    let pending: Vec<(String, String, String, String)> = sqlx::query_as(
        "
        SELECT account_id, access_token, refresh_token, yggdrasil_client_token
        FROM minecraft_users
        WHERE secrets_externalized = 0
          AND (access_token <> '' OR refresh_token <> ''
               OR yggdrasil_client_token <> '')
        ",
    )
    .fetch_all(&mut *conn)
    .await?;

    for (account_id, access_token, refresh_token, client_token) in pending {
        let mut undo = Undo::default();
        let marks = keep_tokens(
            &account_id,
            [&access_token, &refresh_token, &client_token],
            &mut undo,
        )
        .await?;

        let Some(marks) = marks else {
            if secrets::unavailable() {
                return Ok(());
            }
            continue;
        };

        sqlx::query(
            "
            UPDATE minecraft_users
            SET access_token = '', refresh_token = '',
                yggdrasil_client_token = '', secrets_externalized = ?
            WHERE account_id = ?
            ",
        )
        .bind(marks)
        .bind(&account_id)
        .execute(&mut *conn)
        .await?;
    }

    reconcile(conn).await
}

/// A row that says its token was moved but whose entry is gone holds no token at
/// all: its mark is cleared, so the account reads as one that needs a new
/// sign-in instead of failing, and the next write moves it again.
async fn reconcile(conn: &mut SqliteConnection) -> crate::Result<()> {
    let marked: Vec<(String, i64)> = sqlx::query_as(
        "SELECT account_id, secrets_externalized
         FROM minecraft_users
         WHERE secrets_externalized <> 0",
    )
    .fetch_all(&mut *conn)
    .await?;

    for (account_id, marks) in marked {
        let mut kept = 0;
        for (bit, column) in TOKENS {
            if marks & bit == 0 {
                continue;
            }
            match secrets::take(token_key(&account_id, column)).await {
                Stored::Value(_) => kept |= bit,
                Stored::Missing => {}
                Stored::Unavailable => return Ok(()),
            }
        }

        if kept != marks {
            sqlx::query(
                "UPDATE minecraft_users SET secrets_externalized = ?
                 WHERE account_id = ?",
            )
            .bind(kept)
            .bind(&account_id)
            .execute(&mut *conn)
            .await?;
        }
    }

    Ok(())
}
