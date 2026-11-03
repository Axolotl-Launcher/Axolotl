use super::{Observation, Origin};
use crate::state::store::game_option_locale_origins::{
    GameOptionLocaleOrigins, LocaleOriginKey, LocaleOriginRecord,
};
use crate::state::store::synced_game_option_values::SyncedGameOptionValues;
use crate::state::store::{Atom, Collection as _, Handle, atom};

pub(super) struct OriginRow {
    pub scope: String,
    pub option_id: String,
    pub source_instance_id: Option<String>,
    pub source_game_version: Option<String>,
    pub backfilled: bool,
    pub observation: Option<Observation>,
    pub origin: Option<Origin>,
}

#[tracing::instrument(skip_all, fields(scope), err)]
pub(super) async fn load(
    handle: &Handle,
    scope: Option<&str>,
) -> crate::Result<Vec<OriginRow>> {
    let mut rows = match scope {
        Some(scope) => {
            GameOptionLocaleOrigins::get_scoped(handle, &scope.to_owned())
                .await?
        }
        None => GameOptionLocaleOrigins::get_all(handle).await?,
    };
    rows.sort_by(|(left, _), (right, _)| {
        (&left.scope, &left.option_id).cmp(&(&right.scope, &right.option_id))
    });
    rows.into_iter()
        .filter(|(key, _)| {
            key.option_id
                .strip_prefix("external:")
                .and_then(super::mod_translation_key)
                .is_some()
        })
        .map(|(key, record)| {
            let observation = record
                .observation
                .as_deref()
                .map(serde_json::from_str)
                .transpose()
                .inspect_err(|error| tracing::warn!(%error, scope = key.scope,
                    option_id = key.option_id,
                    "Game setting locales: stored observation could not be decoded"))?;
            let origin = record
                .origin
                .as_deref()
                .map(serde_json::from_str)
                .transpose()
                .inspect_err(|error| tracing::warn!(%error, scope = key.scope,
                    option_id = key.option_id,
                    "Game setting locales: stored origin could not be decoded"))?;
            Ok(OriginRow {
                scope: key.scope,
                option_id: key.option_id,
                source_instance_id: record.source_instance_id,
                source_game_version: record.source_game_version,
                backfilled: record.backfilled,
                observation,
                origin,
            })
        })
        .collect()
}

#[tracing::instrument(
    skip_all,
    fields(scope, option_id, instance_id, game_version),
    err
)]
pub(super) async fn observe(
    unit: &mut Atom,
    scope: &str,
    option_id: &str,
    instance_id: &str,
    game_version: &str,
    observation: &Observation,
) -> crate::Result<()> {
    if scope.is_empty()
        && SyncedGameOptionValues::atom_get(unit, &option_id.to_owned())
            .await?
            .is_none()
    {
        return Ok(());
    }
    let key = LocaleOriginKey {
        scope: scope.to_owned(),
        option_id: option_id.to_owned(),
    };
    let json = serde_json::to_string(observation)?;
    let Some(mut record) =
        GameOptionLocaleOrigins::atom_get(unit, &key).await?
    else {
        let record = LocaleOriginRecord {
            source_instance_id: Some(instance_id.to_owned()),
            source_game_version: Some(game_version.to_owned()),
            backfilled: false,
            observation: Some(json),
            origin: None,
        };
        return GameOptionLocaleOrigins::atom_set(unit, &key, Some(&record))
            .await;
    };
    if record.observation.is_some()
        || record.origin.is_some()
        || record.backfilled
        || record.source_instance_id.as_deref() != Some(instance_id)
        || record.source_game_version.as_deref() != Some(game_version)
    {
        return Ok(());
    }
    record.observation = Some(json);
    GameOptionLocaleOrigins::atom_set(unit, &key, Some(&record)).await
}

#[tracing::instrument(skip_all, fields(scope = row.scope, option_id = row.option_id), err)]
pub(super) async fn pin(
    handle: &Handle,
    row: &OriginRow,
    origin: &Origin,
) -> crate::Result<()> {
    let key = LocaleOriginKey {
        scope: row.scope.clone(),
        option_id: row.option_id.clone(),
    };
    let json = serde_json::to_string(origin)?;
    let mut unit = atom(handle).await?;
    let mut pinned = false;
    if let Some(mut record) =
        GameOptionLocaleOrigins::atom_get(&mut unit, &key).await?
        && record.origin.is_none()
    {
        record.origin = Some(json);
        GameOptionLocaleOrigins::atom_set(&mut unit, &key, Some(&record))
            .await?;
        unit.commit().await?;
        pinned = true;
    }
    tracing::debug!(
        pinned,
        key = origin.translation_key,
        "Game setting locales: origin pin completed"
    );
    Ok(())
}
