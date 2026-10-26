use super::*;
use crate::mod_metadata::LocalModMetadata;
use crate::state::instances::DependencyRelation;
use crate::state::instances::{
    ContentDependencyEdge, ContentDependencyKind, ContentEntry, ContentSet,
};

#[derive(Default)]
pub(super) struct DependencyBackfill {
    pub(super) edges: Vec<ContentDependencyEdge>,
    reconciled: HashSet<(String, ContentProvider)>,
    rejected: HashSet<(String, String)>,
}

fn edge_fact(edge: &ContentDependencyEdge) -> String {
    serde_json::to_string(&(
        &edge.parent_entry_id,
        &edge.child_entry_id,
        edge.evidence_provider,
        edge.parent_provider,
        edge.child_provider,
        edge.dependency_kind,
        &edge.parent_project_id,
        &edge.parent_release_id,
        &edge.child_project_id,
        &edge.child_release_id,
    ))
    .unwrap()
}

impl DependencyBackfill {
    pub(super) fn is_empty(&self) -> bool {
        self.reconciled.is_empty() && self.rejected.is_empty()
    }

    fn retain_changes(&mut self, existing: &[ContentDependencyEdge]) {
        self.rejected.retain(|(parent, child)| {
            existing.iter().any(|edge| {
                &edge.parent_entry_id == parent
                    && &edge.child_entry_id == child
                    && edge.dependency_kind == ContentDependencyKind::Required
            })
        });
        self.reconciled.retain(|(parent, provider)| {
            let facts = |edges: &[ContentDependencyEdge]| {
                let mut facts: Vec<_> = edges
                    .iter()
                    .filter(|edge| {
                        &edge.parent_entry_id == parent
                            && &edge.evidence_provider == provider
                    })
                    .map(edge_fact)
                    .collect();
                facts.sort();
                facts.dedup();
                facts
            };
            facts(existing) != facts(&self.edges)
        });
        self.edges.retain(|edge| {
            self.reconciled.contains(&(
                edge.parent_entry_id.clone(),
                edge.evidence_provider,
            ))
        });
    }
}

pub(super) async fn persist_backfill(
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
        content_rows::upsert_content_dependency_edge_in_transaction(edge, tx)
            .await?;
    }
    Ok(())
}

fn provider_ref(
    references: &[ContentProviderRef],
    provider: ContentProvider,
) -> Option<&ContentProviderRef> {
    references.iter().find(|reference| {
        reference.provider() == provider
            && reference.database_release_id().is_some()
    })
}

fn append_edge(
    backfill: &mut DependencyBackfill,
    content_set: &ContentSet,
    parent: &ContentEntry,
    child: &ContentEntry,
    evidence: ContentProvider,
    kind: ContentDependencyKind,
    parent_ref: (ContentProvider, String, String),
    child_ref: (ContentProvider, String, String),
) {
    if parent.id == child.id {
        return;
    }
    let now = chrono::Utc::now();
    backfill.edges.push(ContentDependencyEdge {
        id: format!("content-dependency:{}", uuid::Uuid::new_v4()),
        content_set_id: content_set.id.clone(),
        parent_entry_id: parent.id.clone(),
        child_entry_id: child.id.clone(),
        evidence_provider: evidence,
        parent_provider: parent_ref.0,
        parent_project_id: parent_ref.1,
        parent_release_id: parent_ref.2,
        child_provider: child_ref.0,
        child_project_id: child_ref.1,
        child_release_id: child_ref.2,
        dependency_kind: kind,
        created_at: now,
        modified_at: now,
    });
}

fn remote_children<'a>(
    entries: &'a [ContentEntry],
    refs: &HashMap<String, Vec<ContentProviderRef>>,
    provider: ContentProvider,
    project: &str,
    release: Option<&str>,
) -> Vec<(&'a ContentEntry, ContentProviderRef)> {
    entries
        .iter()
        .filter_map(|child| {
            let reference = refs.get(&child.id)?.iter().find(|reference| {
                reference.provider() == provider
                    && reference.database_project_id() == project
                    && reference.database_release_id().is_some()
                    && release.is_none_or(|expected| {
                        reference.database_release_id().as_deref()
                            == Some(expected)
                    })
            })?;
            Some((child, reference.clone()))
        })
        .collect()
}

fn append_modrinth_edges(
    backfill: &mut DependencyBackfill,
    entries: &[ContentEntry],
    refs: &HashMap<String, Vec<ContentProviderRef>>,
    content_set: &ContentSet,
    parent: &ContentEntry,
    project_id: &str,
    version_id: &str,
    dependencies: &[crate::state::Dependency],
) {
    backfill
        .reconciled
        .insert((parent.id.clone(), ContentProvider::Modrinth));
    for dependency in dependencies.iter().filter(|dependency| {
        matches!(
            dependency.dependency_type,
            crate::state::DependencyType::Required
        )
    }) {
        let Some(project) = dependency.project_id.as_deref() else {
            continue;
        };
        let project = if content_set.loader == crate::state::ModLoader::Quilt
            && project == "P7dR8mSH"
        {
            "qvIfYCYJ"
        } else {
            project
        };
        for (child, reference) in remote_children(
            entries,
            refs,
            ContentProvider::Modrinth,
            project,
            dependency.version_id.as_deref(),
        ) {
            append_edge(
                backfill,
                content_set,
                parent,
                child,
                ContentProvider::Modrinth,
                ContentDependencyKind::Required,
                (
                    ContentProvider::Modrinth,
                    project_id.into(),
                    version_id.into(),
                ),
                (
                    ContentProvider::Modrinth,
                    project.into(),
                    reference.database_release_id().unwrap(),
                ),
            );
        }
    }
}

fn append_curseforge_edges(
    backfill: &mut DependencyBackfill,
    entries: &[ContentEntry],
    refs: &HashMap<String, Vec<ContentProviderRef>>,
    content_set: &ContentSet,
    parent: &ContentEntry,
    project_id: u32,
    file_id: u32,
    dependencies: &[crate::api::curseforge::CurseForgeFileDependency],
) {
    backfill
        .reconciled
        .insert((parent.id.clone(), ContentProvider::CurseForge));
    for dependency in dependencies {
        let kind = match dependency.relation() {
            DependencyRelation::Required => ContentDependencyKind::Required,
            DependencyRelation::Include => ContentDependencyKind::Include,
            _ => continue,
        };
        let project = dependency
            .target_project_id(
                content_set.loader == crate::state::ModLoader::Quilt,
            )
            .to_string();
        for (child, reference) in remote_children(
            entries,
            refs,
            ContentProvider::CurseForge,
            &project,
            None,
        ) {
            append_edge(
                backfill,
                content_set,
                parent,
                child,
                ContentProvider::CurseForge,
                kind,
                (
                    ContentProvider::CurseForge,
                    project_id.to_string(),
                    file_id.to_string(),
                ),
                (
                    ContentProvider::CurseForge,
                    project.clone(),
                    reference.database_release_id().unwrap(),
                ),
            );
        }
    }
}

fn local_endpoint(
    entry: &ContentEntry,
    metadata: &LocalModMetadata,
    refs: &HashMap<String, Vec<ContentProviderRef>>,
) -> (ContentProvider, String, String) {
    if let Some(reference) = refs.get(&entry.id).and_then(|refs| {
        refs.iter()
            .find(|reference| reference.database_release_id().is_some())
    }) {
        (
            reference.provider(),
            reference.database_project_id(),
            reference.database_release_id().unwrap(),
        )
    } else {
        (
            ContentProvider::Local,
            format!("local:{}", metadata.mod_id),
            metadata.version.clone().unwrap_or_default(),
        )
    }
}

fn append_local_edges(
    backfill: &mut DependencyBackfill,
    entries: &[ContentEntry],
    refs: &HashMap<String, Vec<ContentProviderRef>>,
    content_set: &ContentSet,
    local: &HashMap<String, LocalModMetadata>,
) {
    let mut by_mod_id: HashMap<&str, Vec<&ContentEntry>> = HashMap::new();
    for entry in entries {
        if let Some(metadata) = local.get(&entry.id) {
            by_mod_id.entry(&metadata.mod_id).or_default().push(entry);
        }
    }
    for parent in entries {
        let Some(metadata) = local.get(&parent.id) else {
            continue;
        };
        let Some(dependencies) = &metadata.dependencies else {
            continue;
        };
        backfill
            .reconciled
            .insert((parent.id.clone(), ContentProvider::Local));
        for dependency in dependencies {
            for child in by_mod_id
                .get(dependency.mod_id.as_str())
                .into_iter()
                .flatten()
            {
                let child_metadata = &local[&child.id];
                if !dependency.version_range.as_ref().is_none_or(|range| {
                    child_metadata.version.as_ref().is_some_and(|version| crate::resources::plans::requirements::mod_version_matches(range, version))
                }) {
                    backfill.edges.retain(|edge| edge.parent_entry_id != parent.id || edge.child_entry_id != child.id);
                    backfill.rejected.insert((parent.id.clone(), child.id.clone()));
                    continue;
                }
                if backfill.edges.iter().any(|edge| {
                    edge.parent_entry_id == parent.id
                        && edge.child_entry_id == child.id
                        && edge.dependency_kind
                            == ContentDependencyKind::Required
                }) {
                    continue;
                }
                append_edge(
                    backfill,
                    content_set,
                    parent,
                    child,
                    ContentProvider::Local,
                    ContentDependencyKind::Required,
                    local_endpoint(parent, metadata, refs),
                    local_endpoint(child, child_metadata, refs),
                );
            }
        }
    }
}

pub(super) async fn backfill_dependency_edges_from_metadata(
    entries: &[ContentEntry],
    refs: &HashMap<String, Vec<ContentProviderRef>>,
    content_set: &ContentSet,
    local: &HashMap<String, LocalModMetadata>,
    state: &State,
) -> crate::Result<DependencyBackfill> {
    let existing = content_rows::get_content_dependency_edges(
        &content_set.id,
        &state.pool,
    )
    .await?;
    let version_ids: Vec<_> = entries
        .iter()
        .filter_map(|entry| {
            match refs.get(&entry.id).and_then(|refs| {
                provider_ref(refs, ContentProvider::Modrinth)
            })? {
                ContentProviderRef::Modrinth { version_id, .. } => {
                    version_id.clone()
                }
                _ => None,
            }
        })
        .collect();
    let mut backfill = DependencyBackfill::default();
    if !version_ids.is_empty() {
        match CachedEntry::get_version_many(
            &version_ids,
            None,
            &state.pool,
            &state.api_semaphore,
        )
        .await
        {
            Ok(versions) => {
                for parent in entries {
                    let Some(ContentProviderRef::Modrinth {
                        project_id,
                        version_id: Some(version_id),
                    }) = refs.get(&parent.id).and_then(|refs| {
                        provider_ref(refs, ContentProvider::Modrinth)
                    })
                    else {
                        continue;
                    };
                    if let Some(version) = versions.iter().find(|version| {
                        version.id == version_id.as_str()
                            && version.project_id == project_id.as_str()
                    }) {
                        append_modrinth_edges(
                            &mut backfill,
                            entries,
                            refs,
                            content_set,
                            parent,
                            project_id.as_str(),
                            version_id.as_str(),
                            &version.dependencies,
                        );
                    }
                }
            }
            Err(error) => {
                tracing::warn!(%error, "Modrinth dependency reconciliation deferred")
            }
        }
    }
    let files: Vec<_> = entries
        .iter()
        .filter_map(|entry| {
            match refs.get(&entry.id).and_then(|refs| {
                provider_ref(refs, ContentProvider::CurseForge)
            })? {
                ContentProviderRef::CurseForge {
                    project_id,
                    file_id: Some(file_id),
                } => Some((entry, project_id.get(), file_id.get())),
                _ => None,
            }
        })
        .collect();
    if !files.is_empty() {
        match crate::api::curseforge::get_files_many(
            files.iter().map(|(_, _, id)| *id).collect(),
        )
        .await
        {
            Ok(documents) => {
                for (parent, project, id) in files {
                    if let Some(file) = documents
                        .iter()
                        .find(|file| file.id == id && file.mod_id == project)
                    {
                        append_curseforge_edges(
                            &mut backfill,
                            entries,
                            refs,
                            content_set,
                            parent,
                            project,
                            id,
                            &file.dependencies,
                        );
                    }
                }
            }
            Err(error) => {
                tracing::warn!(%error, "CurseForge dependency reconciliation deferred")
            }
        }
    }
    append_local_edges(&mut backfill, entries, refs, content_set, local);
    backfill.retain_changes(&existing);
    Ok(backfill)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry(id: &str) -> ContentEntry {
        ContentEntry {
            id: id.into(),
            instance_id: "instance".into(),
            content_set_id: "set".into(),
            file_id: Some(format!("file-{id}")),
            project_type: crate::state::ProjectType::Mod,
            source_kind: ContentSourceKind::Local,
            ownership_kind: ContentOwnershipKind::UserAdded,
            auto_dependency: false,
            server_requirement: ContentRequirement::Unknown,
            client_requirement: ContentRequirement::Unknown,
            enabled: true,
            added_at: chrono::Utc::now(),
            modified_at: chrono::Utc::now(),
        }
    }
    fn set() -> ContentSet {
        ContentSet {
            id: "set".into(),
            instance_id: "instance".into(),
            name: "test".into(),
            source_kind: ContentSourceKind::Local,
            status: crate::state::instances::ContentSetStatus::Available,
            game_version: "1.20.1".into(),
            protocol_version: None,
            loader: crate::state::ModLoader::Fabric,
            loader_version: None,
            revision: 0,
            created: chrono::Utc::now(),
            modified: chrono::Utc::now(),
        }
    }
    fn mr(project: &str, release: &str) -> ContentProviderRef {
        ContentProviderRef::from_database(
            "modrinth",
            project,
            Some(release),
            None,
        )
        .unwrap()
    }
    fn dependency(project: &str) -> crate::state::Dependency {
        serde_json::from_value(serde_json::json!({"project_id":project,"version_id":null,"file_name":null,"dependency_type":"required"})).unwrap()
    }
    #[test]
    fn remote_partial_graphs_fill_later_dependencies_and_unchanged_graphs_do_not_bump_revisions()
     {
        let entries = vec![entry("a"), entry("b"), entry("c")];
        let mut refs = HashMap::from([
            ("a".into(), vec![mr("parent00", "parent01")]),
            ("b".into(), vec![mr("child000", "child001")]),
        ]);
        let dependencies = [dependency("child000"), dependency("child100")];
        let mut first = DependencyBackfill::default();
        append_modrinth_edges(
            &mut first,
            &entries,
            &refs,
            &set(),
            &entries[0],
            "parent00",
            "parent01",
            &dependencies,
        );
        assert_eq!(first.edges.len(), 1);
        refs.insert("c".into(), vec![mr("child100", "child101")]);
        let mut second = DependencyBackfill::default();
        append_modrinth_edges(
            &mut second,
            &entries,
            &refs,
            &set(),
            &entries[0],
            "parent00",
            "parent01",
            &dependencies,
        );
        second.retain_changes(&first.edges);
        assert_eq!(second.edges.len(), 2);
        let mut unchanged = DependencyBackfill::default();
        append_modrinth_edges(
            &mut unchanged,
            &entries,
            &refs,
            &set(),
            &entries[0],
            "parent00",
            "parent01",
            &dependencies,
        );
        unchanged.retain_changes(&second.edges);
        assert!(unchanged.is_empty());
        let mut removed = DependencyBackfill::default();
        append_modrinth_edges(
            &mut removed,
            &entries,
            &refs,
            &set(),
            &entries[0],
            "parent00",
            "parent01",
            &[],
        );
        removed.retain_changes(&second.edges);
        assert!(!removed.is_empty());
        assert!(removed.edges.is_empty());
    }
    #[test]
    fn curseforge_partial_graphs_are_rechecked_without_trusting_the_old_backfilled_flag()
     {
        let entries = vec![entry("a"), entry("b"), entry("c")];
        let cf = |project, file| {
            ContentProviderRef::from_database(
                "curseforge",
                project,
                Some(file),
                None,
            )
            .unwrap()
        };
        let mut refs = HashMap::from([
            ("a".into(), vec![cf("100", "1001")]),
            ("b".into(), vec![cf("200", "2001")]),
        ]);
        let dependencies = [
            crate::api::curseforge::CurseForgeFileDependency::new(
                200,
                crate::state::instances::DependencyRelation::Required,
            ),
            crate::api::curseforge::CurseForgeFileDependency::new(
                300,
                crate::state::instances::DependencyRelation::Required,
            ),
        ];
        let mut first = DependencyBackfill::default();
        append_curseforge_edges(
            &mut first,
            &entries,
            &refs,
            &set(),
            &entries[0],
            100,
            1001,
            &dependencies,
        );
        assert_eq!(first.edges.len(), 1);
        refs.insert("c".into(), vec![cf("300", "3001")]);
        let mut second = DependencyBackfill::default();
        append_curseforge_edges(
            &mut second,
            &entries,
            &refs,
            &set(),
            &entries[0],
            100,
            1001,
            &dependencies,
        );
        second.retain_changes(&first.edges);
        assert_eq!(second.edges.len(), 2);
    }
    #[test]
    fn local_and_online_mods_share_jar_ids_but_invalid_versions_never_become_satisfied_edges()
     {
        let entries = vec![entry("parent"), entry("child"), entry("other")];
        let refs =
            HashMap::from([("child".into(), vec![mr("child000", "child001")])]);
        let metadata = |id, version, dependencies| {
            serde_json::from_value(serde_json::json!({"mod_id":id,"version":version,"dependencies":dependencies})).unwrap()
        };
        let mut local = HashMap::from([
            (
                "parent".into(),
                metadata(
                    "parent",
                    "1.0",
                    serde_json::json!([{"mod_id":"child","version_range":">=1 <2"}]),
                ),
            ),
            (
                "child".into(),
                metadata("child", "1.5", serde_json::json!([])),
            ),
            (
                "other".into(),
                metadata("other", "1", serde_json::json!([])),
            ),
        ]);
        let mut first = DependencyBackfill::default();
        append_local_edges(&mut first, &entries, &refs, &set(), &local);
        assert_eq!(first.edges.len(), 1);
        assert_eq!(first.edges[0].child_provider, ContentProvider::Modrinth);
        assert_eq!(first.edges[0].child_project_id, "child000");
        let mut previously_online = first.edges.clone();
        previously_online[0].evidence_provider = ContentProvider::Modrinth;

        local.get_mut("child").unwrap().version = Some("2.0".into());
        let mut changed = DependencyBackfill::default();
        append_local_edges(&mut changed, &entries, &refs, &set(), &local);
        changed.retain_changes(&first.edges);
        assert!(changed.edges.is_empty());
        assert!(!changed.is_empty());
        let mut offline = DependencyBackfill::default();
        append_local_edges(&mut offline, &entries, &refs, &set(), &local);
        offline.retain_changes(&previously_online);
        assert!(
            offline
                .rejected
                .contains(&("parent".into(), "child".into()))
        );
    }
    #[test]
    fn failed_remote_metadata_reads_keep_existing_provider_edges() {
        let mut existing = DependencyBackfill::default();
        append_modrinth_edges(
            &mut existing,
            &[entry("a"), entry("b")],
            &HashMap::from([("b".into(), vec![mr("child000", "child001")])]),
            &set(),
            &entry("a"),
            "parent00",
            "parent01",
            &[dependency("child000")],
        );
        let mut failed = DependencyBackfill::default();
        failed.retain_changes(&existing.edges);
        assert!(failed.is_empty());
    }
    #[tokio::test]
    async fn reconciled_edges_replace_only_verified_sources_and_keep_foreign_keys_valid()
     {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        sqlx::raw_sql("INSERT INTO instances(id,path,install_stage,launcher_feature_version,name,created,modified) VALUES ('instance','example','installed','1','Example',1,1); INSERT INTO instance_content_sets(id,instance_id,name,source_kind,status,game_version,loader,created,modified) VALUES ('set','instance','Example','local','available','1.20.1','fabric',1,1); UPDATE instances SET applied_content_set_id='set' WHERE id='instance';").execute(&pool).await.unwrap();
        for id in ["a", "b", "c"] {
            sqlx::query("INSERT INTO instance_content_entries(id,instance_id,content_set_id,project_type,source_kind,ownership_kind,auto_dependency,server_requirement,client_requirement,enabled,added_at,modified_at) VALUES (?,'instance','set','mod','local','user_added',0,'unknown','unknown',1,1,1)").bind(id).execute(&pool).await.unwrap();
        }
        let entries = vec![entry("a"), entry("b"), entry("c")];
        let mut refs =
            HashMap::from([("b".into(), vec![mr("child000", "child001")])]);
        let dependencies = [dependency("child000"), dependency("child100")];
        let mut first = DependencyBackfill::default();
        append_modrinth_edges(
            &mut first,
            &entries,
            &refs,
            &set(),
            &entries[0],
            "parent00",
            "parent01",
            &dependencies,
        );
        let mut tx = pool.begin().await.unwrap();
        persist_backfill(&first, "set", &mut tx).await.unwrap();
        tx.commit().await.unwrap();
        let initial = content_rows::get_content_dependency_edges("set", &pool)
            .await
            .unwrap();
        assert_eq!(initial.len(), 1);
        refs.insert("c".into(), vec![mr("child100", "child101")]);
        let mut second = DependencyBackfill::default();
        append_modrinth_edges(
            &mut second,
            &entries,
            &refs,
            &set(),
            &entries[0],
            "parent00",
            "parent01",
            &dependencies,
        );
        second.retain_changes(&initial);
        let mut tx = pool.begin().await.unwrap();
        persist_backfill(&second, "set", &mut tx).await.unwrap();
        tx.commit().await.unwrap();
        assert_eq!(
            content_rows::get_content_dependency_edges("set", &pool)
                .await
                .unwrap()
                .len(),
            2
        );
        let failed = DependencyBackfill::default();
        let mut tx = pool.begin().await.unwrap();
        persist_backfill(&failed, "set", &mut tx).await.unwrap();
        tx.commit().await.unwrap();
        assert_eq!(
            content_rows::get_content_dependency_edges("set", &pool)
                .await
                .unwrap()
                .len(),
            2
        );
        let mut removal = DependencyBackfill::default();
        append_modrinth_edges(
            &mut removal,
            &entries,
            &refs,
            &set(),
            &entries[0],
            "parent00",
            "parent01",
            &[],
        );
        let mut tx = pool.begin().await.unwrap();
        persist_backfill(&removal, "set", &mut tx).await.unwrap();
        tx.commit().await.unwrap();
        assert!(
            content_rows::get_content_dependency_edges("set", &pool)
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            sqlx::query("PRAGMA foreign_key_check")
                .fetch_all(&pool)
                .await
                .unwrap()
                .is_empty()
        );
        pool.close().await;
    }
}
