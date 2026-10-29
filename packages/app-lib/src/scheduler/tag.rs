//! Scheduling tags. What a task may do is decided by the tag the supervisor
//! minted for it, so a tag cannot be forged: its fields are private, it has
//! no `Deserialize`, `From` or `TryFrom`, and the only constructor is
//! [`TagRegistry::mint`], reached through `Supervisor::mint`. What survives a
//! restart is a [`TagRecord`], which is data, never authority.

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

/// The five scheduling levels; a lower level is more privileged.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    Serialize,
    Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    L0,
    L1,
    L2,
    L3,
    L4,
}

/// Tags one user operation may mint before it is reported as too fine
/// grained (`mint_count_per_user_operation`).
pub const MAX_MINTS_PER_OPERATION: usize = 10;

/// A live scheduling tag. Not `Clone`: whoever holds it holds the level.
#[derive(Debug)]
pub struct Tag {
    registry: u64,
    id: u64,
    generation: u64,
    level: Level,
    origin: String,
}

impl Tag {
    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn level(&self) -> Level {
        self.level
    }

    pub fn origin(&self) -> &str {
        &self.origin
    }

    /// The persistable description of this tag.
    pub fn record(&self) -> TagRecord {
        TagRecord {
            id: self.id,
            generation: self.generation,
            level: self.level,
            origin: self.origin.clone(),
        }
    }
}

/// What is written to disk for a tag. Reading one back grants nothing: a
/// restart re-mints tags through [`TagRegistry::reconcile`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagRecord {
    pub id: u64,
    pub generation: u64,
    pub level: Level,
    pub origin: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum TagError {
    /// The tag was minted by another registry or an earlier generation.
    Stale { tag: u64, generation: u64 },
}

/// Distinguishes registries within one process, so a tag minted by one is
/// never accepted by another even when their generations and ids match.
static NEXT_REGISTRY: AtomicU64 = AtomicU64::new(1);

/// Live tags of one supervisor generation.
pub struct TagRegistry {
    id: u64,
    generation: u64,
    next: AtomicU64,
    live: DashMap<u64, TagRecord>,
    mints: DashMap<String, usize>,
}

impl TagRegistry {
    pub(crate) fn new(generation: u64) -> Self {
        Self {
            id: NEXT_REGISTRY.fetch_add(1, Ordering::Relaxed),
            generation,
            next: AtomicU64::new(1),
            live: DashMap::new(),
            mints: DashMap::new(),
        }
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// The only way to obtain a [`Tag`]; scheduler code reaches it through
    /// `Supervisor::mint`.
    pub(super) fn mint(&self, level: Level, origin: &str) -> Tag {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let tag = Tag {
            registry: self.id,
            id,
            generation: self.generation,
            level,
            origin: origin.to_owned(),
        };
        self.live.insert(id, tag.record());
        let mut count = self.mints.entry(origin.to_owned()).or_default();
        *count += 1;
        if *count > MAX_MINTS_PER_OPERATION {
            tracing::warn!(
                origin,
                mint_count_per_user_operation = *count,
                limit = MAX_MINTS_PER_OPERATION,
                "One operation minted more scheduling tags than the granularity rule allows"
            );
        }
        tag
    }

    /// A child tag for work spawned by `parent`. Derivation never raises the
    /// privilege: asking for a more privileged level keeps the parent's.
    pub fn derive(&self, parent: &Tag, level: Level) -> Result<Tag, TagError> {
        if !self.is_live(parent) {
            return Err(TagError::Stale {
                tag: parent.id,
                generation: parent.generation,
            });
        }
        Ok(self.mint(level.max(parent.level), &parent.origin))
    }

    /// Ends a tag's life; its record leaves the registry.
    pub fn release(&self, tag: Tag) {
        if self.owns(&tag) {
            self.live.remove(&tag.id);
        }
    }

    pub fn is_live(&self, tag: &Tag) -> bool {
        self.owns(tag) && self.live.contains_key(&tag.id)
    }

    /// Tags minted and not yet released.
    pub fn live_count(&self) -> usize {
        self.live.len()
    }

    fn owns(&self, tag: &Tag) -> bool {
        tag.registry == self.id && tag.generation == self.generation
    }

    /// Live tags per level, for the read-only scheduler view.
    pub fn live_by_level(&self) -> std::collections::BTreeMap<Level, usize> {
        let mut counts = std::collections::BTreeMap::new();
        for record in self.live.iter() {
            *counts.entry(record.level).or_default() += 1;
        }
        counts
    }

    /// `mint_count_per_user_operation` for one origin.
    pub fn mint_count(&self, origin: &str) -> usize {
        self.mints.get(origin).map(|count| *count).unwrap_or(0)
    }

    /// Restart reconciliation: every surviving record gets a fresh tag of this
    /// generation, keyed by the record's old id. The level comes from
    /// `level_of` — the persisted level is never trusted. Only scheduler code
    /// reconciles: a record is data, and minting from one is a trusted act.
    // No tags are persisted yet, so only the tests call this until the
    // supervisor gains a restart path.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn reconcile(
        &self,
        records: impl IntoIterator<Item = TagRecord>,
        level_of: impl Fn(&TagRecord) -> Level,
    ) -> HashMap<u64, Tag> {
        records
            .into_iter()
            .map(|record| {
                let tag = self.mint(level_of(&record), &record.origin);
                (record.id, tag)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::marker::PhantomData;

    /// `Probe::<T>::X` is true only when `T` implements the trait behind `X`.
    struct Probe<T>(PhantomData<T>);
    trait Fallback {
        const DESERIALIZE: bool = false;
        const FROM_RECORD: bool = false;
        const TRY_FROM_RECORD: bool = false;
        const FROM_ID: bool = false;
        const CLONE: bool = false;
    }
    impl<T> Fallback for Probe<T> {}
    impl<T: serde::de::DeserializeOwned> Probe<T> {
        const DESERIALIZE: bool = true;
    }
    impl<T: From<TagRecord>> Probe<T> {
        const FROM_RECORD: bool = true;
    }
    impl<T: TryFrom<TagRecord>> Probe<T> {
        const TRY_FROM_RECORD: bool = true;
    }
    impl<T: From<u64>> Probe<T> {
        const FROM_ID: bool = true;
    }
    impl<T: Clone> Probe<T> {
        const CLONE: bool = true;
    }

    #[test]
    fn a_tag_cannot_be_read_back_converted_or_copied() {
        assert!(!Probe::<Tag>::DESERIALIZE);
        assert!(!Probe::<Tag>::FROM_RECORD);
        assert!(!Probe::<Tag>::TRY_FROM_RECORD);
        assert!(!Probe::<Tag>::FROM_ID);
        assert!(!Probe::<Tag>::CLONE);
        // The probe itself works on types that do implement each trait.
        assert!(Probe::<TagRecord>::DESERIALIZE);
        assert!(Probe::<TagRecord>::FROM_RECORD);
        assert!(Probe::<TagRecord>::TRY_FROM_RECORD);
        assert!(Probe::<u64>::FROM_ID);
        assert!(Probe::<TagRecord>::CLONE);
    }

    #[test]
    fn minted_tags_are_registered_and_released() {
        let registry = TagRegistry::new(1);
        let tag = registry.mint(Level::L3, "install:job-1");
        assert_eq!(tag.level(), Level::L3);
        assert!(registry.is_live(&tag));
        let id = tag.id();
        registry.release(tag);
        assert!(!registry.live.contains_key(&id));
    }

    #[test]
    fn derivation_never_raises_the_level() {
        let registry = TagRegistry::new(1);
        let parent = registry.mint(Level::L3, "install:job-1");
        assert_eq!(
            registry.derive(&parent, Level::L0).unwrap().level(),
            Level::L3
        );
        assert_eq!(
            registry.derive(&parent, Level::L4).unwrap().level(),
            Level::L4
        );
    }

    #[test]
    fn tags_of_an_earlier_generation_cannot_derive() {
        let before = TagRegistry::new(1);
        let after = TagRegistry::new(2);
        let tag = before.mint(Level::L2, "refresh");
        assert_eq!(
            after.derive(&tag, Level::L2).unwrap_err(),
            TagError::Stale {
                tag: tag.id(),
                generation: 1,
            }
        );
        assert!(!after.is_live(&tag));
    }

    #[test]
    fn a_tag_from_another_registry_is_never_accepted() {
        let first = TagRegistry::new(1);
        let second = TagRegistry::new(1);
        let foreign = first.mint(Level::L0, "a");
        let own = second.mint(Level::L3, "b");
        assert_eq!(foreign.id(), own.id(), "same generation and id");
        assert!(!second.is_live(&foreign));
        assert!(second.derive(&foreign, Level::L0).is_err());
        second.release(foreign);
        assert!(second.is_live(&own), "releasing a foreign tag keeps ours");
    }

    #[test]
    fn mint_count_per_user_operation_is_tracked() {
        let registry = TagRegistry::new(1);
        for _ in 0..=MAX_MINTS_PER_OPERATION {
            registry.mint(Level::L2, "search:1");
        }
        assert_eq!(
            registry.mint_count("search:1"),
            MAX_MINTS_PER_OPERATION + 1
        );
        assert_eq!(registry.mint_count("search:2"), 0);
    }
}

#[cfg(test)]
mod reconcile_tests {
    use super::*;

    #[test]
    fn persisted_level_is_not_trusted() {
        let before = TagRegistry::new(1);
        let record = before.mint(Level::L3, "install:job-7").record();
        let mut forged = record.clone();
        forged.level = Level::L0;

        let after = TagRegistry::new(2);
        let mapped = after.reconcile([forged], |_| Level::L3);
        let tag = &mapped[&record.id];
        assert_eq!(tag.level(), Level::L3, "the stored L0 is ignored");
        assert_eq!(tag.origin(), "install:job-7");
        assert!(after.is_live(tag));
    }

    #[test]
    fn every_old_record_maps_to_a_new_tag() {
        let before = TagRegistry::new(1);
        let records: Vec<_> = (0..3)
            .map(|index| {
                before.mint(Level::L2, &format!("op:{index}")).record()
            })
            .collect();
        let after = TagRegistry::new(2);
        let mapped = after.reconcile(records.clone(), |_| Level::L2);
        assert_eq!(mapped.len(), 3);
        for record in &records {
            assert_eq!(mapped[&record.id].record().generation, 2);
        }
    }
}
