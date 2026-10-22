use crate::state::instances::{
    LoaderComponent, LoaderComponentKind, LoaderComponentRole,
};
use sqlx::{FromRow, Sqlite, SqlitePool, Transaction};
use std::collections::HashMap;

#[derive(Debug, FromRow)]
struct LoaderComponentRow {
    instance_id: String,
    kind: String,
    version: Option<String>,
    role: String,
    provider_metadata: Option<String>,
}

impl TryFrom<LoaderComponentRow> for LoaderComponent {
    type Error = crate::Error;

    fn try_from(row: LoaderComponentRow) -> crate::Result<Self> {
        let provider_metadata = row
            .provider_metadata
            .map(|value| serde_json::from_str(&value))
            .transpose()
            .map_err(|err| {
                crate::ErrorKind::InputError(format!(
                    "Invalid loader provider metadata: {err}"
                ))
                .as_error()
            })?;

        Ok(Self {
            instance_id: row.instance_id,
            kind: LoaderComponentKind::from_str(&row.kind)?,
            version: row.version,
            role: LoaderComponentRole::from_str(&row.role)?,
            provider_metadata,
        })
    }
}

pub(crate) async fn list_loader_components(
    instance_id: &str,
    pool: &SqlitePool,
) -> crate::Result<Vec<LoaderComponent>> {
    let rows = sqlx::query_as::<_, LoaderComponentRow>(
        "SELECT instance_id, kind, version, role, provider_metadata
		 FROM instance_loader_components
		 WHERE instance_id = ?
		 ORDER BY CASE role WHEN 'primary' THEN 0 ELSE 1 END, kind",
    )
    .bind(instance_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter().map(TryInto::try_into).collect()
}

pub(crate) async fn list_loader_components_many(
    instance_ids: &[&str],
    pool: &SqlitePool,
) -> crate::Result<HashMap<String, Vec<LoaderComponent>>> {
    let rows = sqlx::query_as::<_, LoaderComponentRow>(
        "SELECT instance_id, kind, version, role, provider_metadata
         FROM instance_loader_components
         WHERE instance_id IN (SELECT value FROM json_each(?))
         ORDER BY instance_id, CASE role WHEN 'primary' THEN 0 ELSE 1 END, kind",
    )
    .bind(serde_json::to_string(instance_ids)?)
    .fetch_all(pool)
    .await?;
    let mut grouped = HashMap::<String, Vec<LoaderComponent>>::new();
    for row in rows {
        let component = LoaderComponent::try_from(row)?;
        grouped
            .entry(component.instance_id.clone())
            .or_default()
            .push(component);
    }
    Ok(grouped)
}

pub(crate) async fn replace_loader_components(
    instance_id: &str,
    components: &[LoaderComponent],
    tx: &mut Transaction<'_, Sqlite>,
) -> crate::Result<()> {
    sqlx::query("DELETE FROM instance_loader_components WHERE instance_id = ?")
        .bind(instance_id)
        .execute(&mut **tx)
        .await?;

    for component in components {
        if component.instance_id != instance_id {
            return Err(crate::ErrorKind::InputError(format!(
                "Loader component {} belongs to a different instance",
                component.kind.as_str()
            ))
            .into());
        }
        let provider_metadata = component
            .provider_metadata
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        sqlx::query(
            "INSERT INTO instance_loader_components (
				instance_id, kind, version, role, provider_metadata
			 ) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(instance_id)
        .bind(component.kind.as_str())
        .bind(&component.version)
        .bind(component.role.as_str())
        .bind(provider_metadata)
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{LoaderComponentKind, ModLoader};
    use sqlx::sqlite::SqlitePoolOptions;

    #[tokio::test]
    async fn component_rows_round_trip_and_cascade() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(
            "CREATE TABLE instances (id TEXT PRIMARY KEY);
			 CREATE TABLE instance_loader_components (
				instance_id TEXT NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
				kind TEXT NOT NULL,
				version TEXT,
				role TEXT NOT NULL,
				provider_metadata TEXT,
				PRIMARY KEY(instance_id, kind)
			 );",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO instances(id) VALUES ('instance')")
            .execute(&pool)
            .await
            .unwrap();
        let components = vec![
            LoaderComponent::new_primary(
                "instance",
                ModLoader::Forge,
                Some("47.4.0".to_string()),
            ),
            LoaderComponent {
                instance_id: "instance".to_string(),
                kind: LoaderComponentKind::OptiFine,
                version: Some("HD_U_I6".to_string()),
                role: LoaderComponentRole::Adjunct,
                provider_metadata: Some(
                    serde_json::json!({ "source": "bmclapi" }),
                ),
            },
        ];
        let mut tx = pool.begin().await.unwrap();
        replace_loader_components("instance", &components, &mut tx)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        assert_eq!(
            list_loader_components("instance", &pool).await.unwrap(),
            components
        );
        sqlx::raw_sql(
            "CREATE TABLE instance_sync_preferences (
                instance_id TEXT, feature TEXT, enabled INTEGER
             );
             INSERT INTO instance_sync_preferences VALUES
                ('instance', 'screenshots', 1),
                ('instance', 'resource_packs', 0),
                ('other', 'game_options', 1);",
        )
        .execute(&pool)
        .await
        .unwrap();
        let ids = ["instance", "missing", "instance"];
        let grouped = list_loader_components_many(&ids, &pool).await.unwrap();
        assert_eq!(grouped.len(), 1);
        assert_eq!(grouped["instance"], components);
        let preferences =
            super::super::instance_rows::get_instance_synced_options_many(
                &ids, &pool,
            )
            .await
            .unwrap();
        assert_eq!(preferences.len(), 1);
        assert_eq!(
            preferences["instance"],
            super::super::instance_rows::get_instance_synced_options(
                "instance", &pool
            )
            .await
            .unwrap()
        );
        assert!(
            list_loader_components_many(&[], &pool)
                .await
                .unwrap()
                .is_empty()
        );

        sqlx::query("DELETE FROM instances WHERE id = 'instance'")
            .execute(&pool)
            .await
            .unwrap();
        assert!(
            list_loader_components("instance", &pool)
                .await
                .unwrap()
                .is_empty()
        );
    }
}
