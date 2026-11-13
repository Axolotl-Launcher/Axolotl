//! Absolute paths the instance rows keep into the launcher directory.

use sqlx::SqliteConnection;

/// Rewrites every stored path that names the launcher's previous directory, so
/// a moved launcher directory still points at the icons and Java runtime the
/// rows were recorded with.
pub(crate) async fn rewrite_launcher_directory(
    conn: &mut SqliteConnection,
    old: &str,
    new: &str,
) -> crate::Result<()> {
    sqlx::query(
        "
        UPDATE instances
        SET icon_path = replace(icon_path, ?, ?)
        WHERE icon_path IS NOT NULL
        ",
    )
    .bind(old)
    .bind(new)
    .execute(&mut *conn)
    .await?;

    sqlx::query(
        "
        UPDATE instance_files
        SET icon_path = replace(icon_path, ?, ?)
        WHERE icon_path IS NOT NULL AND icon_path != ''
        ",
    )
    .bind(old)
    .bind(new)
    .execute(&mut *conn)
    .await?;

    sqlx::query(
        "
        UPDATE instance_launch_overrides
        SET overrides = jsonb(json_set(
            overrides,
            '$.java_path',
            replace(json_extract(overrides, '$.java_path'), ?, ?)
        ))
        WHERE json_type(overrides, '$.java_path') = 'text'
        ",
    )
    .bind(old)
    .bind(new)
    .execute(&mut *conn)
    .await?;

    Ok(())
}
