//! When each server of an instance was last joined.

use chrono::{DateTime, TimeZone, Utc};

use super::{Collection, Handle};

/// The server join log, kept per instance and server address.
pub(crate) struct JoinLog;

/// One record of the log, identified by the instance and the server joined.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JoinLogKey {
    pub instance_id: String,
    pub host: String,
    pub port: u16,
}

/// What a join left behind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct JoinLogEntry {
    pub join_time: DateTime<Utc>,
}

impl Collection for JoinLog {
    type Key = JoinLogKey;
    type Record = JoinLogEntry;

    async fn set(
        handle: &Handle,
        key: &Self::Key,
        record: Option<&Self::Record>,
    ) -> crate::Result<()> {
        let Some(record) = record else {
            sqlx::query(
                "
                DELETE FROM join_log
                WHERE instance_id = ? AND host = ? AND port = ?
                ",
            )
            .bind(&key.instance_id)
            .bind(&key.host)
            .bind(i64::from(key.port))
            .execute(handle.pool())
            .await?;

            return Ok(());
        };

        sqlx::query(
            "
            INSERT INTO join_log (instance_id, host, port, join_time)
            VALUES (?, ?, ?, ?)
            ON CONFLICT (instance_id, host, port) DO UPDATE SET
                join_time = excluded.join_time
            ",
        )
        .bind(&key.instance_id)
        .bind(&key.host)
        .bind(i64::from(key.port))
        .bind(record.join_time.timestamp())
        .execute(handle.pool())
        .await?;

        Ok(())
    }

    async fn get(
        handle: &Handle,
        key: &Self::Key,
    ) -> crate::Result<Option<Self::Record>> {
        let join_time: Option<i64> = sqlx::query_scalar(
            "
            SELECT join_time
            FROM join_log
            WHERE instance_id = ? AND host = ? AND port = ?
            ",
        )
        .bind(&key.instance_id)
        .bind(&key.host)
        .bind(i64::from(key.port))
        .fetch_optional(handle.pool())
        .await?;

        Ok(join_time.map(|join_time| Self::Record {
            join_time: to_time(join_time),
        }))
    }

    async fn set_all(
        handle: &Handle,
        records: &[(Self::Key, Self::Record)],
    ) -> crate::Result<()> {
        let mut transaction = handle.pool().begin().await?;
        for (key, record) in records {
            sqlx::query(
                "
                INSERT INTO join_log (instance_id, host, port, join_time)
                VALUES (?, ?, ?, ?)
                ON CONFLICT (instance_id, host, port) DO UPDATE SET
                    join_time = excluded.join_time
                ",
            )
            .bind(&key.instance_id)
            .bind(&key.host)
            .bind(i64::from(key.port))
            .bind(record.join_time.timestamp())
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;

        Ok(())
    }

    async fn get_all(
        handle: &Handle,
    ) -> crate::Result<Vec<(Self::Key, Self::Record)>> {
        let rows: Vec<(String, String, i64, i64)> = sqlx::query_as(
            "
            SELECT instance_id, host, port, join_time
            FROM join_log
            ",
        )
        .fetch_all(handle.pool())
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|(instance_id, host, port, join_time)| {
                let Some(port) = u16::try_from(port).ok() else {
                    tracing::warn!(
                        %instance_id,
                        %host,
                        port,
                        "Ignoring a join log entry whose port is out of range"
                    );
                    return None;
                };
                Some((
                    JoinLogKey {
                        instance_id,
                        host,
                        port,
                    },
                    JoinLogEntry {
                        join_time: to_time(join_time),
                    },
                ))
            })
            .collect())
    }
}

fn to_time(timestamp: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(timestamp, 0)
        .single()
        .unwrap_or_else(Utc::now)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A log of its own, with the instances the log refers to already there.
    async fn handle(instances: &[&str]) -> Handle {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        for instance_id in instances {
            sqlx::query(
                "INSERT INTO instances
                    (id, path, install_stage, launcher_feature_version,
                     name, created, modified)
                 VALUES (?, ?, 'installed', '1', ?, 1, 1)",
            )
            .bind(instance_id)
            .bind(instance_id)
            .bind(instance_id)
            .execute(&pool)
            .await
            .unwrap();
        }
        Handle::new(pool)
    }

    fn key(instance_id: &str, host: &str, port: u16) -> JoinLogKey {
        JoinLogKey {
            instance_id: instance_id.to_string(),
            host: host.to_string(),
            port,
        }
    }

    fn at(seconds: i64) -> JoinLogEntry {
        JoinLogEntry {
            join_time: Utc.timestamp_opt(seconds, 0).unwrap(),
        }
    }

    #[tokio::test]
    async fn a_join_replaces_the_time_the_address_held() {
        let handle = handle(&["instance"]).await;
        let address = key("instance", "mc.example.com", 25565);

        JoinLog::set(&handle, &address, Some(&at(1_700_000_000)))
            .await
            .unwrap();
        JoinLog::set(&handle, &address, Some(&at(1_700_000_600)))
            .await
            .unwrap();

        assert_eq!(
            JoinLog::get(&handle, &address).await.unwrap(),
            Some(at(1_700_000_600))
        );
        assert_eq!(
            JoinLog::get_all(&handle).await.unwrap(),
            vec![(address, at(1_700_000_600))]
        );
    }

    #[tokio::test]
    async fn writing_nothing_removes_the_record() {
        let handle = handle(&["instance"]).await;
        let address = key("instance", "mc.example.com", 25565);

        JoinLog::set(&handle, &address, Some(&at(1_700_000_000)))
            .await
            .unwrap();
        JoinLog::set(&handle, &address, None).await.unwrap();

        assert_eq!(JoinLog::get(&handle, &address).await.unwrap(), None);
        assert!(JoinLog::get_all(&handle).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_whole_collection_round_trips() {
        let handle = handle(&["first", "second"]).await;
        let records = vec![
            (key("first", "mc.example.com", 25565), at(1_700_000_000)),
            (key("second", "mc.example.org", 25566), at(1_700_000_600)),
        ];

        JoinLog::set_all(&handle, &records).await.unwrap();

        let mut stored = JoinLog::get_all(&handle).await.unwrap();
        stored.sort_by(|left, right| {
            left.0.instance_id.cmp(&right.0.instance_id)
        });
        assert_eq!(stored, records);
    }

    #[tokio::test]
    async fn a_port_outside_the_range_is_ignored() {
        let handle = handle(&["instance"]).await;
        JoinLog::set(
            &handle,
            &key("instance", "mc.example.com", 25565),
            Some(&at(1_700_000_000)),
        )
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO join_log (instance_id, host, port, join_time)
             VALUES ('instance', 'mc.example.org', 70000, 1700000000)",
        )
        .execute(handle.pool())
        .await
        .unwrap();

        let stored = JoinLog::get_all(&handle).await.unwrap();

        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].0.host, "mc.example.com");
        assert_eq!(stored[0].0.port, 25565);
    }
}
