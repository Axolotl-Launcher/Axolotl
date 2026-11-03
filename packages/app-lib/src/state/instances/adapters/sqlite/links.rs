use super::instance_rows::InstanceLinkRow;
use crate::state::InstanceLink;
use sqlx::{Executor, Sqlite, Transaction};

pub(crate) async fn get_instance_link<'e, E>(
    instance_id: &str,
    exec: E,
) -> crate::Result<InstanceLink>
where
    E: Executor<'e, Database = Sqlite>,
{
    let row = sqlx::query_as!(
        InstanceLinkRow,
        r#"
        SELECT
            instance_id,
            link_kind,
            modrinth_project_id,
            modrinth_version_id,
            server_project_id,
            content_project_id,
            content_version_id,
            hosting_server_id,
            json(hosting_instance_ids) AS "hosting_instance_ids?: String",
            hosting_active_instance_id,
            shared_instance_id,
            imported_name,
            imported_version_number,
            imported_filename
        FROM instance_links
        WHERE instance_id = ?
        "#,
        instance_id,
    )
    .fetch_optional(exec)
    .await?;

    match row {
        Some(row) => row.try_into(),
        None => Ok(InstanceLink::Unmanaged),
    }
}

pub(crate) async fn upsert_instance_link(
    instance_id: &str,
    link: &InstanceLink,
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    let columns = instance_link_columns(link)?;
    if !matches!(link, InstanceLink::Unmanaged) {
        sqlx::query("DELETE FROM resource_pack_unlinks WHERE instance_id = ?")
            .bind(instance_id)
            .execute(&mut **tx)
            .await?;
    }
    let modrinth_project_id = columns.modrinth_project_id.as_deref();
    let modrinth_version_id = columns.modrinth_version_id.as_deref();
    let server_project_id = columns.server_project_id.as_deref();
    let content_project_id = columns.content_project_id.as_deref();
    let content_version_id = columns.content_version_id.as_deref();
    let hosting_server_id = columns.hosting_server_id.as_deref();
    let hosting_instance_ids = columns.hosting_instance_ids.as_deref();
    let hosting_active_instance_id =
        columns.hosting_active_instance_id.as_deref();
    let shared_instance_id = columns.shared_instance_id.as_deref();
    let imported_name = columns.imported_name.as_deref();
    let imported_version_number = columns.imported_version_number.as_deref();
    let imported_filename = columns.imported_filename.as_deref();

    sqlx::query!(
        "
        INSERT INTO instance_links (
            instance_id,
            link_kind,
            modrinth_project_id,
            modrinth_version_id,
            server_project_id,
            content_project_id,
            content_version_id,
            hosting_server_id,
            hosting_instance_ids,
            hosting_active_instance_id,
            shared_instance_id,
            imported_name,
            imported_version_number,
            imported_filename
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, jsonb(?), ?, ?, ?, ?, ?)
        ON CONFLICT (instance_id) DO UPDATE SET
            link_kind = excluded.link_kind,
            modrinth_project_id = excluded.modrinth_project_id,
            modrinth_version_id = excluded.modrinth_version_id,
            server_project_id = excluded.server_project_id,
            content_project_id = excluded.content_project_id,
            content_version_id = excluded.content_version_id,
            hosting_server_id = excluded.hosting_server_id,
            hosting_instance_ids = excluded.hosting_instance_ids,
            hosting_active_instance_id = excluded.hosting_active_instance_id,
            shared_instance_id = excluded.shared_instance_id,
            imported_name = excluded.imported_name,
            imported_version_number = excluded.imported_version_number,
            imported_filename = excluded.imported_filename
        ",
        instance_id,
        columns.link_kind,
        modrinth_project_id,
        modrinth_version_id,
        server_project_id,
        content_project_id,
        content_version_id,
        hosting_server_id,
        hosting_instance_ids,
        hosting_active_instance_id,
        shared_instance_id,
        imported_name,
        imported_version_number,
        imported_filename,
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}

struct InstanceLinkColumns {
    link_kind: &'static str,
    modrinth_project_id: Option<String>,
    modrinth_version_id: Option<String>,
    server_project_id: Option<String>,
    content_project_id: Option<String>,
    content_version_id: Option<String>,
    hosting_server_id: Option<String>,
    hosting_instance_ids: Option<String>,
    hosting_active_instance_id: Option<String>,
    shared_instance_id: Option<String>,
    imported_name: Option<String>,
    imported_version_number: Option<String>,
    imported_filename: Option<String>,
}

fn instance_link_columns(
    link: &InstanceLink,
) -> crate::Result<InstanceLinkColumns> {
    match link {
        InstanceLink::Unmanaged => Ok(InstanceLinkColumns {
            link_kind: "unmanaged",
            modrinth_project_id: None,
            modrinth_version_id: None,
            server_project_id: None,
            content_project_id: None,
            content_version_id: None,
            hosting_server_id: None,
            hosting_instance_ids: None,
            hosting_active_instance_id: None,
            shared_instance_id: None,
            imported_name: None,
            imported_version_number: None,
            imported_filename: None,
        }),
        InstanceLink::ModrinthModpack {
            project_id,
            version_id,
        } => Ok(InstanceLinkColumns {
            link_kind: "modrinth_modpack",
            modrinth_project_id: Some(project_id.clone()),
            modrinth_version_id: Some(version_id.clone()),
            server_project_id: None,
            content_project_id: None,
            content_version_id: None,
            hosting_server_id: None,
            hosting_instance_ids: None,
            hosting_active_instance_id: None,
            shared_instance_id: None,
            imported_name: None,
            imported_version_number: None,
            imported_filename: None,
        }),
        InstanceLink::CurseForgeModpack {
            project_id,
            version_id,
        } => Ok(InstanceLinkColumns {
            link_kind: "curseforge_modpack",
            modrinth_project_id: Some(project_id.clone()),
            modrinth_version_id: Some(version_id.clone()),
            server_project_id: None,
            content_project_id: None,
            content_version_id: None,
            hosting_server_id: None,
            hosting_instance_ids: None,
            hosting_active_instance_id: None,
            shared_instance_id: None,
            imported_name: None,
            imported_version_number: None,
            imported_filename: None,
        }),
        InstanceLink::ServerProject { project_id } => Ok(InstanceLinkColumns {
            link_kind: "server_project",
            modrinth_project_id: None,
            modrinth_version_id: None,
            server_project_id: Some(project_id.clone()),
            content_project_id: None,
            content_version_id: None,
            hosting_server_id: None,
            hosting_instance_ids: None,
            hosting_active_instance_id: None,
            shared_instance_id: None,
            imported_name: None,
            imported_version_number: None,
            imported_filename: None,
        }),
        InstanceLink::ServerProjectModpack {
            server_project_id,
            content_project_id,
            content_version_id,
        } => Ok(InstanceLinkColumns {
            link_kind: "server_project_modpack",
            modrinth_project_id: None,
            modrinth_version_id: None,
            server_project_id: Some(server_project_id.clone()),
            content_project_id: Some(content_project_id.clone()),
            content_version_id: Some(content_version_id.clone()),
            hosting_server_id: None,
            hosting_instance_ids: None,
            hosting_active_instance_id: None,
            shared_instance_id: None,
            imported_name: None,
            imported_version_number: None,
            imported_filename: None,
        }),
        InstanceLink::ImportedModpack {
            project_id,
            version_id,
            name,
            version_number,
            filename,
        } => Ok(InstanceLinkColumns {
            link_kind: "imported_modpack",
            modrinth_project_id: project_id.clone(),
            modrinth_version_id: version_id.clone(),
            server_project_id: None,
            content_project_id: None,
            content_version_id: None,
            hosting_server_id: None,
            hosting_instance_ids: None,
            hosting_active_instance_id: None,
            shared_instance_id: None,
            imported_name: name.clone(),
            imported_version_number: version_number.clone(),
            imported_filename: filename.clone(),
        }),
        InstanceLink::SharedInstance { shared_instance_id } => {
            Ok(InstanceLinkColumns {
                link_kind: "shared_instance",
                modrinth_project_id: None,
                modrinth_version_id: None,
                server_project_id: None,
                content_project_id: None,
                content_version_id: None,
                hosting_server_id: None,
                hosting_instance_ids: None,
                hosting_active_instance_id: None,
                shared_instance_id: Some(shared_instance_id.to_string()),
                imported_name: None,
                imported_version_number: None,
                imported_filename: None,
            })
        }
    }
}
