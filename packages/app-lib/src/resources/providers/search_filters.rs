use super::*;

pub(crate) fn for_source(
    expression: &ResourceSearchExpression,
    source: &SourceId,
) -> ResourceResult<ResourceSearchExpression> {
    match expression {
        ResourceSearchExpression::All { terms } => {
            Ok(ResourceSearchExpression::All {
                terms: terms
                    .iter()
                    .map(|term| for_source(term, source))
                    .collect::<ResourceResult<_>>()?,
            })
        }
        ResourceSearchExpression::Any { terms } => {
            Ok(ResourceSearchExpression::Any {
                terms: terms
                    .iter()
                    .map(|term| for_source(term, source))
                    .collect::<ResourceResult<_>>()?,
            })
        }
        ResourceSearchExpression::Predicate {
            field,
            values,
            excluded,
        } => {
            let selected: Vec<_> = values
                .iter()
                .filter_map(|value| {
                    if field == "projectId"
                        && *excluded
                        && let Some((owner, id)) = value.split_once(':')
                    {
                        return (owner == source.as_str())
                            .then(|| id.to_string());
                    }
                    if field == "category"
                        && let Some((owner, value)) = value.split_once(':')
                    {
                        return (owner == source.as_str())
                            .then(|| value.to_string());
                    }
                    Some(value.clone())
                })
                .collect();
            if selected.is_empty() {
                if matches!(field.as_str(), "projectId" | "category")
                    && *excluded
                {
                    return Ok(ResourceSearchExpression::All { terms: vec![] });
                }
                return Err(ResourceError::Unsupported(format!(
                    "source cannot express {field} filter"
                )));
            }
            Ok(ResourceSearchExpression::Predicate {
                field: field.clone(),
                values: selected,
                excluded: *excluded,
            })
        }
    }
}

pub(crate) fn modrinth(
    expression: &ResourceSearchExpression,
    depth: usize,
) -> ResourceResult<String> {
    if depth > 16 {
        return Err(ResourceError::InvalidRequest(
            "search filter nesting exceeded".into(),
        ));
    }
    match expression {
        ResourceSearchExpression::All { terms }
        | ResourceSearchExpression::Any { terms } => {
            if terms.is_empty() {
                return Ok("project_id != \"\"".into());
            }
            if terms.len() > 4096 {
                return Err(ResourceError::InvalidRequest(
                    "invalid search filter group".into(),
                ));
            }
            let operator =
                if matches!(expression, ResourceSearchExpression::All { .. }) {
                    " AND "
                } else {
                    " OR "
                };
            Ok(format!(
                "({})",
                terms
                    .iter()
                    .map(|term| modrinth(term, depth + 1))
                    .collect::<ResourceResult<Vec<_>>>()?
                    .join(operator)
            ))
        }
        ResourceSearchExpression::Predicate {
            field,
            values,
            excluded,
        } => {
            let field = match field.as_str() {
                "type" => "project_types",
                "allTypes" => "all_project_types",
                "category" | "loader" => "categories",
                "gameVersion" => "game_versions",
                "environment" => "environment",
                "license" => "license",
                "openSource" => "open_source",
                "projectId" => "project_id",
                "dependencyProject" => "compatible_dependency_project_ids",
                "requiredDependency" => "required_dependency_project_ids",
                "optionalDependency" => "optional_dependency_project_ids",
                "incompatibleDependency" => {
                    "incompatible_dependency_project_ids"
                }
                "embeddedDependency" => "embedded_dependency_project_ids",
                _ => {
                    return Err(ResourceError::Unsupported(
                        "unknown search filter field".into(),
                    ));
                }
            };
            if values.is_empty()
                || values.len() > 100
                || values.iter().any(|value| {
                    value.len() > 512 || value.chars().any(char::is_control)
                })
            {
                return Err(ResourceError::InvalidRequest(
                    "invalid search filter values".into(),
                ));
            }
            let quoted = values
                .iter()
                .map(|value| {
                    if field == "open_source"
                        && matches!(value.as_str(), "true" | "false")
                    {
                        Ok(value.clone())
                    } else {
                        serde_json::to_string(value)
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(if quoted.len() == 1 {
                format!(
                    "{field} {} {}",
                    if *excluded { "!=" } else { "=" },
                    quoted[0]
                )
            } else {
                format!(
                    "{field} {} [{}]",
                    if *excluded { "NOT IN" } else { "IN" },
                    quoted.join(", ")
                )
            })
        }
    }
}

pub(crate) fn validate_curseforge(
    expression: &ResourceSearchExpression,
    depth: usize,
) -> ResourceResult<()> {
    if depth > 16 {
        return Err(ResourceError::InvalidRequest(
            "search filter nesting exceeded".into(),
        ));
    }
    match expression {
        ResourceSearchExpression::All { terms } => {
            for term in terms {
                validate_curseforge(term, depth + 1)?;
            }
            Ok(())
        }
        ResourceSearchExpression::Predicate {
            field,
            excluded: false,
            ..
        } if matches!(
            field.as_str(),
            "type" | "category" | "loader" | "gameVersion"
        ) =>
        {
            Ok(())
        }
        ResourceSearchExpression::Predicate {
            field,
            excluded: true,
            ..
        } if matches!(field.as_str(), "projectId" | "allTypes" | "type") => {
            Ok(())
        }
        _ => Err(ResourceError::Unsupported(
            "CurseForge cannot express these search filters".into(),
        )),
    }
}

pub(crate) fn merge_positive_filters(
    query: &ResourceSearchQuery,
    source: &SourceId,
) -> ResourceResult<ResourceSearchQuery> {
    fn merge(
        current: &mut Vec<String>,
        values: &[String],
    ) -> ResourceResult<()> {
        if current.is_empty() {
            *current = values.to_vec();
        } else {
            current.retain(|value| values.contains(value));
        }
        if current.is_empty() {
            return Err(ResourceError::Unsupported(
                "conflicting search constraints".into(),
            ));
        }
        Ok(())
    }
    fn visit(
        expression: &ResourceSearchExpression,
        query: &mut ResourceSearchQuery,
        depth: usize,
    ) -> ResourceResult<()> {
        if depth > 16 {
            return Err(ResourceError::InvalidRequest(
                "search filter nesting exceeded".into(),
            ));
        }
        match expression {
            ResourceSearchExpression::All { terms } => {
                for term in terms {
                    visit(term, query, depth + 1)?;
                }
            }
            ResourceSearchExpression::Predicate {
                field,
                values,
                excluded: false,
            } => match field.as_str() {
                "gameVersion" => {
                    merge(&mut query.target.game_versions, values)?
                }
                "loader" => merge(&mut query.target.loaders, values)?,
                "type" => {
                    let mut kinds: Vec<String> = query
                        .types
                        .iter()
                        .map(|kind| {
                            match kind {
                                ResourceType::Mod => "mod",
                                ResourceType::Plugin => "plugin",
                                ResourceType::ResourcePack => "resourcepack",
                                ResourceType::DataPack => "datapack",
                                ResourceType::Shader => "shader",
                                ResourceType::ModPack => "modpack",
                                ResourceType::World => "world",
                            }
                            .to_owned()
                        })
                        .collect();
                    let values: Vec<_> = values
                        .iter()
                        .map(|value| value.to_lowercase())
                        .collect();
                    merge(&mut kinds, &values)?;
                    query.types = kinds
                        .iter()
                        .map(|kind| match kind.as_str() {
                            "mod" => Ok(ResourceType::Mod),
                            "plugin" => Ok(ResourceType::Plugin),
                            "resourcepack" => Ok(ResourceType::ResourcePack),
                            "datapack" => Ok(ResourceType::DataPack),
                            "shader" => Ok(ResourceType::Shader),
                            "modpack" => Ok(ResourceType::ModPack),
                            "world" => Ok(ResourceType::World),
                            _ => Err(ResourceError::Unsupported(
                                "unknown content type".into(),
                            )),
                        })
                        .collect::<ResourceResult<_>>()?;
                }
                "category" => {
                    if !query.categories.is_empty()
                        && query.categories != *values
                    {
                        return Err(ResourceError::Unsupported("multiple category conjunctions cannot be expressed".into()));
                    }
                    query.categories = values.clone();
                }
                _ => {
                    return Err(ResourceError::Unsupported(
                        "source cannot express search predicate".into(),
                    ));
                }
            },
            ResourceSearchExpression::Predicate { excluded: true, .. } => {}
            _ => {
                return Err(ResourceError::Unsupported(
                    "source cannot express boolean search alternatives".into(),
                ));
            }
        }
        Ok(())
    }
    let mut merged = query.clone();
    if let Some(expression) = &query.filter {
        let expression = for_source(expression, source)?;
        visit(&expression, &mut merged, 0)?;
    }
    Ok(merged)
}
