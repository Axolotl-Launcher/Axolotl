use super::*;

impl ResourceService {
    pub(crate) async fn confirm_counterpart(
        self: &std::sync::Arc<Self>,
        metadata: &SourceProjectMetadata,
        cancel: &CancellationToken,
    ) -> ResourceResult<()> {
        if self.store().is_none()
            || !matches!(
                metadata.project.source.as_str(),
                "modrinth" | "curseforge"
            )
        {
            return Ok(());
        }
        let Some(slug) = &metadata.slug else {
            return Ok(());
        };
        let source = &metadata.project.source;
        let lookup = crate::content_search::lookup_content_identities(
            &if *source == SourceId::modrinth() {
                vec![slug.clone()]
            } else {
                vec![]
            },
            &if *source == SourceId::curseforge() {
                vec![slug.clone()]
            } else {
                vec![]
            },
        );
        let records = if *source == SourceId::modrinth() {
            lookup.modrinth.get(slug)
        } else {
            lookup.curseforge.get(slug)
        };
        let Some(records) = records.filter(|records| records.len() == 1) else {
            return Ok(());
        };
        let opposite = if *source == SourceId::modrinth() {
            SourceId::curseforge()
        } else {
            SourceId::modrinth()
        };
        if !self
            .sources()
            .iter()
            .any(|candidate| candidate.available && candidate.id == opposite)
        {
            return Ok(());
        }
        let Some(hint) = records[0]
            .counterparts
            .iter()
            .find(|hint| hint.provider == opposite.as_str())
        else {
            return Ok(());
        };
        if let Some(store) = self.store()
            && let Some(id) = store.lookup(&metadata.project).await?
            && store
                .projects(&id)
                .await?
                .iter()
                .any(|project| project.source == opposite)
        {
            return Ok(());
        }
        let project = if opposite == SourceId::modrinth() {
            match self.locate(&opposite, hint.slug.clone(), cancel).await {
                Ok(document)
                    if document.metadata.slug.as_deref().is_some_and(
                        |slug| slug.eq_ignore_ascii_case(&hint.slug),
                    ) =>
                {
                    Some(document.metadata)
                }
                Err(ResourceError::Canceled) => {
                    return Err(ResourceError::Canceled);
                }
                _ => None,
            }
        } else {
            let mut found = None;
            for kind in &metadata.types {
                let query = ipc::ResourceSearchQuery {
                    filter: None,
                    text: hint.slug.clone(),
                    types: vec![*kind],
                    sources: vec![opposite.clone()],
                    target: ipc::ResourceTarget {
                        game_versions: vec![],
                        loaders: vec![],
                        channels: vec![],
                    },
                    categories: vec![],
                    sort: ipc::ResourceSearchSort::Relevance,
                    page_size: 50,
                };
                match self
                    .query(
                        &opposite,
                        ProviderQuery::Search {
                            query,
                            cursor: None,
                        },
                        CachePolicy::UseFresh,
                        cancel,
                    )
                    .await
                {
                    Ok(QueryOutcome {
                        value: ProviderValue::Search(page),
                        ..
                    }) => {
                        let candidates: Vec<_> = page
                            .entries
                            .into_iter()
                            .filter(|project| {
                                project.slug.as_deref().is_some_and(|slug| {
                                    slug.eq_ignore_ascii_case(&hint.slug)
                                })
                            })
                            .collect();
                        if candidates.len() == 1 {
                            found = candidates.into_iter().next();
                            break;
                        }
                    }
                    Err(ResourceError::Canceled) => {
                        return Err(ResourceError::Canceled);
                    }
                    _ => {}
                }
            }
            found
        };
        if let Some(other) = project {
            if cancel.is_cancelled() {
                return Err(ResourceError::Canceled);
            }
            if let Some(store) = self.store() {
                match store.confirm_curated_projects(metadata, &other).await {
                    Ok(_) => {}
                    Err(ResourceError::InvalidRequest(reason)) => {
                        tracing::debug!(%reason, "MCMOD counterpart is not the same subvariant");
                    }
                    Err(error) => return Err(error),
                }
            }
        }
        Ok(())
    }
}
