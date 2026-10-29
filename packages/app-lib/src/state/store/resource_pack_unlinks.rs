//! The content set an instance's pack-managed content was last unlinked from.
//!
//! The marker is written by the half that owns the instance's content sets;
//! the resource layer only asks whether the set a plan is about was unlinked.

use sqlx::SqliteConnection;

/// Whether `instance_id`'s content was unlinked from `content_set_id`.
pub(crate) async fn marked(
    conn: &mut SqliteConnection,
    instance_id: &str,
    content_set_id: &str,
) -> crate::Result<bool> {
    let marked: i64 = sqlx::query_scalar(
        "
        SELECT EXISTS(
            SELECT 1
            FROM resource_pack_unlinks
            WHERE instance_id = ? AND content_set_id = ?)
        ",
    )
    .bind(instance_id)
    .bind(content_set_id)
    .fetch_one(&mut *conn)
    .await?;

    Ok(marked != 0)
}
