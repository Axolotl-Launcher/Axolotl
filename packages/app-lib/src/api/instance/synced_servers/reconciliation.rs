use super::codec::{server_identity_address, server_identity_name};
use super::types::{
    CanonicalServer, LocalServer, ProjectionEntry, ProjectionOwner,
};
use quartz_nbt::NbtCompound;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Computes a change without touching disk. The caller commits it together with
/// a pending checkpoint before attempting any filesystem projection.
pub(super) fn reconcile_entries(
    current: Vec<NbtCompound>,
    projections: &[ProjectionEntry],
    mut canonical: Vec<CanonicalServer>,
    mut locals: Vec<LocalServer>,
) -> (Vec<CanonicalServer>, Vec<LocalServer>) {
    let projection_matches = match_projection_entries(&current, projections);
    let mut matched = HashSet::new();

    let projected_ids = projections
        .iter()
        .filter(|entry| entry.owner == ProjectionOwner::Synced)
        .map(|entry| entry.id.as_str())
        .collect::<HashSet<_>>();
    let mut unseen =
        HashMap::<(String, String), std::collections::VecDeque<String>>::new();
    for server in &canonical {
        if !projected_ids.contains(server.id.as_str()) {
            unseen
                .entry((
                    server_identity_address(&server.data),
                    server_identity_name(&server.data),
                ))
                .or_default()
                .push_back(server.id.clone());
        }
    }
    let canonical_indices = canonical
        .iter()
        .enumerate()
        .map(|(index, server)| (server.id.clone(), index))
        .collect::<HashMap<_, _>>();
    let local_indices = locals
        .iter()
        .enumerate()
        .map(|(index, server)| (server.id.clone(), index))
        .collect::<HashMap<_, _>>();
    let mut canonical_order = Vec::new();

    for ((position, data), projection) in
        current.into_iter().enumerate().zip(projection_matches)
    {
        if let Some(projection) = projection {
            matched.insert(projection.id.clone());
            match projection.owner {
                ProjectionOwner::Synced => {
                    canonical_order.push(projection.id.clone());
                    if let Some(server) = canonical_indices
                        .get(&projection.id)
                        .and_then(|index| canonical.get_mut(*index))
                        && data != projection.data
                    {
                        server.data = data;
                    }
                }
                ProjectionOwner::Instance => {
                    if let Some(server) = local_indices
                        .get(&projection.id)
                        .and_then(|index| locals.get_mut(*index))
                    {
                        if data != projection.data {
                            server.data = data;
                        }
                        server.position = position as i64;
                    }
                }
            }
        } else {
            let identity =
                (server_identity_address(&data), server_identity_name(&data));
            let existing = if identity.0.is_empty() {
                None
            } else {
                unseen.get_mut(&identity).and_then(|ids| ids.pop_front())
            };
            if let Some(id) = existing {
                // A sibling instance or a prior interrupted capture already imported it.
                canonical_order.push(id);
            } else {
                let id = Uuid::new_v4().to_string();
                canonical_order.push(id.clone());
                canonical.push(CanonicalServer { id, data });
            }
        }
    }

    let removed_synced = projections
        .iter()
        .filter(|entry| {
            entry.owner == ProjectionOwner::Synced
                && !matched.contains(&entry.id)
        })
        .map(|entry| entry.id.as_str())
        .collect::<HashSet<_>>();
    let removed_local = projections
        .iter()
        .filter(|entry| {
            entry.owner == ProjectionOwner::Instance
                && !matched.contains(&entry.id)
        })
        .map(|entry| entry.id.as_str())
        .collect::<HashSet<_>>();
    canonical.retain(|server| !removed_synced.contains(server.id.as_str()));
    locals.retain(|server| !removed_local.contains(server.id.as_str()));
    let by_id = canonical
        .iter()
        .cloned()
        .map(|server| (server.id.clone(), server))
        .collect::<std::collections::HashMap<_, _>>();
    let visible = canonical_order.iter().cloned().collect::<HashSet<_>>();
    let mut ordered = canonical_order
        .into_iter()
        .filter_map(|id| by_id.get(&id).cloned());
    for server in &mut canonical {
        if visible.contains(&server.id)
            && let Some(next) = ordered.next()
        {
            *server = next;
        }
    }
    (canonical, locals)
}

pub(super) fn match_projection_entries<'a>(
    current: &[NbtCompound],
    projections: &'a [ProjectionEntry],
) -> Vec<Option<&'a ProjectionEntry>> {
    let mut matches = vec![None; current.len()];
    let mut used = HashSet::new();
    // Index each pass instead of scanning every projection and repeatedly
    // allocating normalized addresses/names for every current entry.
    for pass in 0..3 {
        let key = |data: &NbtCompound| -> Option<(String, String)> {
            let address = server_identity_address(data);
            let name = server_identity_name(data);
            match pass {
                0 => Some((address, name)),
                1 if !address.is_empty() => Some((address, String::new())),
                2 if !name.is_empty() => Some((String::new(), name)),
                _ => None,
            }
        };
        let mut index = HashMap::<_, Vec<usize>>::new();
        for (position, projection) in projections.iter().enumerate() {
            if !used.contains(&position)
                && let Some(key) = key(&projection.data)
            {
                index.entry(key).or_default().push(position);
            }
        }
        for (position, data) in current.iter().enumerate() {
            if matches[position].is_some() {
                continue;
            }
            let Some(key) = key(data) else {
                continue;
            };
            let Some(candidates) = index.get_mut(&key) else {
                continue;
            };
            let candidate = candidates
                .iter()
                .enumerate()
                .filter(|(_, index)| {
                    pass != 0 || projections[**index].data == *data
                })
                .min_by_key(|(_, index)| {
                    (
                        projections[**index].position.abs_diff(position as i64),
                        **index,
                    )
                })
                .map(|(offset, _)| offset);
            if let Some(offset) = candidate {
                let index = candidates.swap_remove(offset);
                used.insert(index);
                matches[position] = Some(&projections[index]);
            }
        }
    }
    matches
}
