use super::Handle;
use crate::state::instances::commands::content_snapshot::dependencies::DependencyBackfill;

use crate::state::instances::adapters::sqlite::{
    content_entries, content_sets,
};
pub(crate) async fn persist_backfill(
    backfill: &DependencyBackfill,
    content_set_id: &str,
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
) -> crate::Result<()> {
    for (parent, provider) in &backfill.reconciled {
        sqlx::query("DELETE FROM instance_content_dependencies WHERE content_set_id = ? AND parent_entry_id = ? AND evidence_provider = ?")
            .bind(content_set_id).bind(parent).bind(provider.as_str()).execute(&mut **tx).await?;
    }
    for (parent, child) in &backfill.rejected {
        sqlx::query("DELETE FROM instance_content_dependencies WHERE content_set_id = ? AND parent_entry_id = ? AND child_entry_id = ? AND dependency_kind = 'required'")
            .bind(content_set_id).bind(parent).bind(child).execute(&mut **tx).await?;
    }
    for edge in &backfill.edges {
        content_entries::upsert_content_dependency_edge_in_transaction(
            edge, tx,
        )
        .await?;
    }
    Ok(())
}

pub(crate) async fn set_backfill(
    handle: &Handle,
    instance_id: &str,
    set_id: &str,
    revision: u64,
    backfill: &DependencyBackfill,
) -> crate::Result<()> {
    let mut tx = handle.pool().begin_with("BEGIN IMMEDIATE").await?;
    let current =
        content_sets::get_applied_content_set(instance_id, &mut *tx).await?;
    if !current.is_some_and(|set| set.id == set_id && set.revision == revision)
    {
        return Ok(());
    }
    persist_backfill(backfill, set_id, &mut tx).await?;
    tx.commit().await?;
    Ok(())
}
