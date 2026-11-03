//! What an earlier launcher build left in the grouping tables.
//!
//! Groups live in a settings file now; these tables are only read once, when
//! that file is created. Two shapes of them exist, sharing the table name and
//! meaning different columns by it, so which shape a database has is answered
//! by asking its schema rather than by the rows.

use sqlx::{Executor, Sqlite};

/// The grouping rows a database holds, in the shape they were written in.
pub(crate) enum LegacyGroups {
    /// `instance_groups(id, name)`, with `instance_group_memberships` beside it.
    Named {
        groups: Vec<(String, String)>,
        memberships: Vec<(String, String)>,
    },
    /// `instance_groups(instance_id, group_name)`.
    Memberships(Vec<(String, String)>),
    /// Neither shape, but a table of that name is there.
    Unrecognised(Vec<String>),
    /// No grouping table at all.
    Absent,
}

pub(crate) async fn read_legacy<'e, E>(exec: E) -> crate::Result<LegacyGroups>
where
    E: Executor<'e, Database = Sqlite> + Copy,
{
    let columns: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM pragma_table_info('instance_groups')",
    )
    .fetch_all(exec)
    .await?;
    let has_column = |name: &str| columns.iter().any(|column| column == name);

    if has_column("id") && has_column("name") {
        let groups = sqlx::query_as::<_, (String, String)>(
            "SELECT id, name FROM instance_groups ORDER BY display_order, name, id",
        )
        .fetch_all(exec)
        .await?;

        let has_memberships: bool = sqlx::query_scalar(
            "SELECT EXISTS(
                SELECT 1 FROM sqlite_master
                WHERE type = 'table' AND name = 'instance_group_memberships'
            )",
        )
        .fetch_one(exec)
        .await?;

        let memberships = if has_memberships {
            sqlx::query_as::<_, (String, String)>(
                "SELECT instance_id, group_id FROM instance_group_memberships",
            )
            .fetch_all(exec)
            .await?
        } else {
            Vec::new()
        };

        return Ok(LegacyGroups::Named {
            groups,
            memberships,
        });
    }

    if has_column("instance_id") && has_column("group_name") {
        let rows = sqlx::query_as::<_, (String, String)>(
            "SELECT instance_id, group_name
             FROM instance_groups
             ORDER BY group_name, instance_id",
        )
        .fetch_all(exec)
        .await?;

        return Ok(LegacyGroups::Memberships(rows));
    }

    if columns.is_empty() {
        return Ok(LegacyGroups::Absent);
    }
    Ok(LegacyGroups::Unrecognised(columns))
}

#[cfg(test)]
#[path = "instance_groups_tests.rs"]
mod tests;
