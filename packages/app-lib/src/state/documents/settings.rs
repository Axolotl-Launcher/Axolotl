//! The launcher's own settings live in eight documents rather than one: the
//! keys of `Settings` are split by `DOMAINS`, so `appearance.json` carries the
//! look of the launcher while `download.json` carries how it fetches.
//!
//! A read layers, bottom up:
//!
//! 1. the build defaults, which every key starts at;
//! 2. the settings row, for the domains whose document is missing or cannot be
//!    read;
//! 3. the deployment's own documents, which a build ships and never writes;
//! 4. the local documents, the only layer ever written.
//!
//! A domain that has a document is answered by that document alone: the row
//! does not contribute to it, or a key the document leaves out would come back
//! from a value the row froze. The row is only asked for a domain with no
//! document at all, so one damaged file costs that domain and nothing else.
//!
//! The deployment's layer answers above the row rather than below it: a value a
//! deployment pins must not be shadowed by a row value frozen before it was
//! deployed. It answers whatever the local documents do, since it carries keys
//! for domains that have a local document too, and a local key wins over it.
//!
//! The domains are written one at a time, so a launcher stopped between two of
//! them leaves the domains a generation apart. They carry unrelated settings,
//! which is why that is accepted rather than paid for with a cross-file write.
//!
//! Two keys are the row's alone. `personalized_ads` is in no document: it is
//! the one `Settings` field marked `#[serde(skip)]`, it stays a column of the
//! row, and listing it here would promise a document that never carries it.
//! `version` is in no document for the other direction: `Settings::migrate`
//! reads it to decide which migrations to run, so a document that could answer
//! it would let a file skip them. The row is therefore read on every read and
//! both keys always come from it: the layers above answer no part of them, so a
//! document that exists cannot lower `personalized_ads` to the default, no
//! document's absence is needed to reach it, and no document can raise `version`
//! past a migration (Q5).
//!
//! `Settings::get` composes these documents with the row once a start has named
//! the directory they live in (`identify`), and answers from the row alone until
//! it has. A read is cached, because a setting is read at every turn and
//! composing one reads eight documents and the row: `invalidate` is what a
//! writer says when what a read composes from has changed.
//!
//! A writer of the settings row calls `store_cut_over` once the row has landed,
//! and that is the only thing it has to call: the domains whose writes have
//! moved are stated from the row there, so a row write that reached no document
//! cannot happen through a caller forgetting one. The domains it states are the
//! ones carrying a key that write changed, so a domain beside it keeps the
//! document it has, and the keys a deployment answers stay out of the local one
//! unless the write named them.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::OnceLock;
#[cfg(not(test))]
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Map, Value};

use super::{Documents, Shipped, Stored, WRITES};
use crate::state::settings::Settings;
use crate::state::store::Handle;

/// The documents of this launch, once a start has named the directory they live
/// in. A read that has no root composes nothing and answers from the row, which
/// is what a unit test that builds its own handle sees.
static ROOT: OnceLock<PathBuf> = OnceLock::new();

#[cfg(test)]
thread_local! {
    /// The root one test gives itself. The tests of a crate run as one process,
    /// so a root set for one of them would answer the tests running beside it:
    /// the thread a test runs on keeps its own instead.
    static TEST_ROOT: std::cell::RefCell<Option<PathBuf>> =
        const { std::cell::RefCell::new(None) };
    /// The cache and version a test gives itself for the same reason as the
    /// root: shared entries would let a test running beside it overwrite the
    /// cache or invalidate it between two reads that assert it is reused.
    static TEST_CACHE: std::cell::RefCell<
        Option<(u64, PathBuf, Arc<Settings>)>,
    > = const { std::cell::RefCell::new(None) };
    static TEST_VERSION: std::cell::Cell<u64> =
        const { std::cell::Cell::new(0) };
}

/// Names the directory the documents of this launch live in: the settings
/// directory, whose `settings` subdirectory is the local layer.
///
/// Called once at startup, before the first setting is read. Until it is, no
/// document answers and every read is the row's.
pub(crate) fn identify(settings_dir: &Path) {
    let root = settings_dir.join(super::DIR_NAME);
    #[cfg(test)]
    TEST_ROOT.with(|test| *test.borrow_mut() = Some(root));
    #[cfg(not(test))]
    let _ = ROOT.set(root);
}

/// The documents of this launch, when a start has named their directory.
///
/// A test answers with the root of its own thread instead of the process-wide
/// one, so that one test naming a directory does not decide the documents of
/// every test running beside it.
pub(crate) fn root() -> Option<PathBuf> {
    #[cfg(test)]
    if let Some(root) = TEST_ROOT.with(|test| test.borrow().clone()) {
        return Some(root);
    }
    ROOT.get().cloned()
}

/// What the last read composed, with what it was composed from: the directory
/// its documents live in, and the version everything that could change them was
/// at when it was composed.
#[cfg(not(test))]
static CACHE: tokio::sync::RwLock<Option<(u64, PathBuf, Arc<Settings>)>> =
    tokio::sync::RwLock::const_new(None);

/// Moves whenever something a read composes from changes: a document is
/// written, or the row is.
///
/// A count rather than a lock, which is what keeps the order `WRITES → cache`
/// acyclic: every document writer holds `WRITES` while it says that a document
/// changed, and a lock taken there would have it wait on a read that composes
/// for the very documents it is replacing. `invalidate` therefore takes no lock
/// at all, and the cache's own lock is only ever taken by a read, which takes no
/// `WRITES`.
#[cfg(not(test))]
static VERSION: AtomicU64 = AtomicU64::new(0);

fn current_version() -> u64 {
    #[cfg(not(test))]
    {
        VERSION.load(Ordering::SeqCst)
    }
    #[cfg(test)]
    {
        TEST_VERSION.with(|version| version.get())
    }
}

/// Says that what a read composes from has changed, so the next read composes
/// again.
///
/// A writer calls it once its change is in place: the document writers below,
/// `Settings::update` and the row writers for the row. A writer that changes the
/// row inside a unit asks the unit to say it instead, so the invalidation lands
/// when the commit does — see `Atom::invalidate_settings_after_commit`.
pub(crate) fn invalidate() {
    #[cfg(not(test))]
    {
        VERSION.fetch_add(1, Ordering::SeqCst);
    }
    #[cfg(test)]
    {
        TEST_VERSION.with(|version| {
            version.set(version.get().wrapping_add(1));
        });
    }
}

/// The domains with the keys their document carries. A key belongs to exactly
/// one of them, and a key none of them lists is not stored in a document at
/// all.
///
/// The version travels with the domain and is bumped when that domain's key set
/// changes, including when a key is dropped: it versions the shape of a
/// document, unlike the `version` the row carries, which says which settings
/// migrations have run and is therefore in no document.
const DOMAINS: &[(&str, &[&str], u32)] = &[
    (
        "appearance",
        &[
            "accent_color",
            "advanced_rendering",
            "auto_hide_downloads_button",
            "close_behavior",
            "collapsed_navigation",
            "custom_background_blur",
            "custom_background_component_opacity",
            "custom_background_opacity",
            "custom_background_path",
            "custom_window_title_enabled",
            "default_page",
            "default_window_title",
            "developer_mode",
            "feature_flags",
            "hidden_nav_items",
            "hide_nametag_skins_page",
            "home_layout",
            "home_widget_background_opacity",
            "home_widgets",
            "locale",
            "log_level",
            "minimal_home_instance_id",
            "mono_font",
            "nametag_uses_app_font",
            "native_decorations",
            "show_files_tab_in_instances",
            "show_screenshots_tab_in_instances",
            "show_skin_selector_in_sidebar",
            "show_worlds_tab_in_instances",
            "sidebar_instance_count",
            "sync_features_across_devices",
            "theme",
            "toggle_sidebar",
            "transparent_background",
            "transparent_background_blur",
            "transparent_background_opacity",
            "ui_font",
        ],
        1,
    ),
    (
        "download",
        &[
            "auto_concurrent_downloads",
            "bypass_curseforge_download_restrictions",
            "curseforge_source",
            "doh_enabled",
            "max_concurrent_downloads",
            "max_concurrent_writes",
            "minecraft_file_source",
            "minecraft_metadata_source",
            "modrinth_source",
            "mojang_auth_source",
            "resource_chinese_filenames",
            "resource_preserve_filenames",
        ],
        1,
    ),
    (
        "game",
        &[
            "auto_set_java_high_performance_mode",
            "custom_env_vars",
            "default_game_language",
            "enter_lightweight_mode_on_game_launch",
            "extra_launch_args",
            "force_fullscreen",
            "game_resolution",
            "hide_on_process_start",
            "hooks",
            "maximize_window",
            "memory",
        ],
        1,
    ),
    ("backup", &["backup_repository_path"], 1),
    ("bootstrap", &["custom_dir", "prev_custom_dir"], 1),
    (
        "network",
        &[
            "allow_external_scheme",
            "allow_privileged_scheme",
            "ignore_ssl_errors",
            "terracotta_public_nodes",
        ],
        1,
    ),
    (
        "privacy",
        &["discord_rpc", "telemetry", "telemetry_consent_version"],
        1,
    ),
    (
        "state",
        &[
            "auto_download_updates",
            "migrated",
            "onboarded",
            "onboarding_instance_tour_completed",
            "onboarding_version",
            "pending_update_toast_for_version",
            "skipped_update",
        ],
        // `version` was dropped from the key set: a document that could answer
        // it would let a file skip the settings migrations.
        2,
    ),
];

/// The domains whose writes have moved to their document.
///
/// A write of the row reaches the domains listed here by stating each of the
/// ones that carries a key it changed from the row as it stands, which is what
/// `store_cut_over` does. A domain not listed here is still answered by the row
/// alone, so stating it would write a file nothing reads yet.
pub(crate) const CUT_OVER: &[&str] = &[
    "appearance",
    "download",
    "game",
    "backup",
    "state",
    "bootstrap",
    "privacy",
    "network",
];

/// One document as it stands on disk. Its keys are not interpreted yet, so a
/// value this build cannot read costs only that key.
type Document = Map<String, Value>;

/// The settings a read composes, reusing the last composition while nothing
/// that goes into it has changed.
///
/// This is the entry `Settings::get` takes. A read is not cheap — eight
/// documents and the row — and it is made at every turn, so an answer nothing
/// has moved under is kept until a writer says otherwise.
pub(crate) async fn cached(
    documents: &Documents<'_>,
    handle: &Handle,
) -> crate::Result<Settings> {
    let (version, cached) = cached_for(&documents.root).await;
    if let Some(settings) = cached {
        return Ok((*settings).clone());
    }
    let settings = Arc::new(read(documents, handle).await?);
    // A write that landed while this read composed moved the version, and the
    // row it read may be the one that write replaced: what was composed is kept
    // only when nothing moved under it.
    remember(&documents.root, version, &settings).await;
    Ok((*settings).clone())
}

/// The last composition for `root`, with the version it was checked against.
///
/// In production the version is read with the cache's own lock held rather than
/// before it, so nothing can move between reading it and comparing it: no await
/// sits between the two, and a writer's `invalidate` can therefore not land in
/// the middle and be missed. It narrows the window rather than closing it — a
/// writer takes no cache lock, so one that moved just after this load is still
/// answered from the entry this read saw. What guarantees a read after a write
/// is fresh is the write's own invalidation, which lands once its change is in
/// place.
async fn cached_for(root: &Path) -> (u64, Option<Arc<Settings>>) {
    #[cfg(not(test))]
    {
        let cached = CACHE.read().await;
        let version = current_version();
        let settings =
            cached.as_ref().and_then(|(stored, cached_root, settings)| {
                (*stored == version && cached_root == root)
                    .then(|| Arc::clone(settings))
            });
        (version, settings)
    }
    #[cfg(test)]
    {
        let version = current_version();
        let settings = TEST_CACHE.with(|cached| {
            cached.borrow().as_ref().and_then(
                |(stored, cached_root, settings)| {
                    (*stored == version && cached_root == root)
                        .then(|| Arc::clone(settings))
                },
            )
        });
        (version, settings)
    }
}

/// Keeps `settings` for `root` when nothing that composes it moved during the
/// read.
async fn remember(root: &Path, version: u64, settings: &Arc<Settings>) {
    #[cfg(not(test))]
    {
        let mut cached = CACHE.write().await;
        if current_version() == version {
            *cached = Some((version, root.to_path_buf(), Arc::clone(settings)));
        }
    }
    #[cfg(test)]
    {
        if current_version() == version {
            TEST_CACHE.with(|cached| {
                *cached.borrow_mut() =
                    Some((version, root.to_path_buf(), Arc::clone(settings)));
            });
        }
    }
}

/// The settings the documents and the row together describe.
pub(crate) async fn read(
    documents: &Documents<'_>,
    handle: &Handle,
) -> crate::Result<Settings> {
    // Layer 1: the build defaults, which every key starts at.
    let mut values = default_values()?;
    // The layers above the row are laid down after it, so what they carry is
    // collected first: the row is read once, and only for the keys that need
    // it.
    let mut without_a_document: Vec<&[&str]> = Vec::new();
    let mut shipped: Vec<(&str, &[&str], Document)> = Vec::new();
    let mut local: Vec<(&str, &[&str], Document)> = Vec::new();

    for &(name, keys, schema_version) in DOMAINS {
        // Layer 3 contributes to every domain it carries a document for, not
        // only the ones with no local document.
        if let Some(document) = documents.shipped(name, schema_version).await {
            shipped.push((name, keys, document));
        }
        match documents.stored::<Document>(name, schema_version).await {
            // Layer 4: a document is the authority for the domain it exists
            // for, whichever build wrote it.
            Stored::Ready { document, .. } | Stored::Newer { document, .. } => {
                local.push((name, keys, document));
            }
            // Layer 2 reaches the result here: a domain with no document is the
            // only kind the row answers, and only for its own keys.
            Stored::Missing | Stored::Unreadable => {
                without_a_document.push(keys);
            }
        }
    }

    // The row is read whether or not a domain needs it, because
    // `personalized_ads` is in no document and only it can answer that key.
    let (row_settings, row) = row_values(handle).await?;
    for keys in without_a_document {
        contribute(&mut values, keys, &row);
    }

    // The deployment's documents sit above the row and below the local ones: a
    // value it pins must not be shadowed by a value the row froze before the
    // deployment existed, and the user's own document is the last word.
    for (name, keys, document) in shipped {
        overlay(&mut values, keys, &document, name);
    }
    for (name, keys, document) in local {
        overlay(&mut values, keys, &document, name);
    }

    let mut settings: Settings = serde_json::from_value(Value::Object(values))?;
    // The two keys the row answers alone get the one mechanism they rest on here:
    // this assignment from the row, made after every layer has been laid down and
    // answering whatever they say. What lets it be the only mechanism is the
    // shape of the settings rather than a second guard beside it —
    // `personalized_ads` is `#[serde(skip)]`, so the map above cannot carry it
    // back, and `version` belongs to no domain of `DOMAINS`, so no document can
    // answer it. Those two are the premises: together they make any other answer
    // impossible. `version` is what `migrate` reads to choose its migrations, so
    // a document able to raise it would let a file skip them; the ads preference
    // is a column of the row and nothing else, so no document can lower it to the
    // default either.
    settings.personalized_ads = row_settings.personalized_ads;
    settings.version = row_settings.version;
    // A document is text the user can edit by hand, so a value out of range is
    // pulled back into it rather than handed on as it stands.
    Ok(settings.normalize())
}

/// The settings a build starts from, keyed the way a document keys them.
fn default_values() -> crate::Result<Map<String, Value>> {
    let default = serde_json::to_value(Settings::default())?;
    Ok(serde_json::from_value(default)?)
}

/// The value a key has before any local document is asked: what the deployment
/// ships for it, and what this build defaults to when the deployment ships
/// nothing.
///
/// This is what a local document is compared against, because a key holding
/// exactly this value says nothing: leaving it out of the local document reads
/// the same value and lets a later change of the deployment's document arrive.
fn provided<'a>(
    key: &str,
    config: &'a Map<String, Value>,
    defaults: &'a Map<String, Value>,
) -> Option<&'a Value> {
    config.get(key).or_else(|| defaults.get(key))
}

/// The value a key reads as, rather than the one a file happens to say.
///
/// A document is read through `Settings`, and every read normalizes: a count
/// written as `0` where its range starts at `1` reads as `1`, so a file saying
/// `0` and a file saying `1` are the same setting and must compare equal here.
/// The projection is `Settings`' own shape, so the value is judged by the type
/// the key belongs to; a key the type does not carry, or a value it cannot
/// take, keeps the value it came in with.
///
/// This costs one deserialization and one normalization per value that is
/// compared, which a start pays a couple of dozen times over its documents.
fn effective(
    key: &str,
    value: &Value,
    projection: &Map<String, Value>,
) -> Value {
    if !projection.contains_key(key) {
        return value.clone();
    }
    let mut values = projection.clone();
    values.insert(key.to_string(), value.clone());
    let Ok(settings) =
        serde_json::from_value::<Settings>(Value::Object(values))
    else {
        return value.clone();
    };
    let Ok(Value::Object(mut normalized)) =
        serde_json::to_value(settings.normalize())
    else {
        return value.clone();
    };
    normalized.remove(key).unwrap_or_else(|| value.clone())
}

/// Turns the settings row over to the documents of its domains, and reports
/// whether the handover is done.
///
/// Every domain is decided on its own: one that already carries a document was
/// handed over by a start before this one and is left as it stands, and one that
/// carries none is written from the row. The handover is only finished when
/// every domain has reached one of the two, so a domain a later start still has
/// to write is not mistaken for one that is already in place.
///
/// The draft is the row, stated the way `store` states it: a key holding what
/// the domain is provided with is left out, because leaving it out reads the
/// same value and lets a later change of that value still arrive. The keys the
/// deployment answers are left out as well and not merely compared: a local copy
/// of one would be the last word for that key, so the deployment's own change
/// would never reach the user. A deployment document this build cannot read
/// leaves those keys unknown, so its domain is skipped and counts as unfinished.
pub(crate) async fn hand_over_if_missing(documents: &Documents<'_>) -> bool {
    // Taken before the row is read, and not after: a commit that lands between
    // the two would otherwise be overwritten by the older row this call read.
    // The lock order stays `WRITES → cache`, because nothing here takes the
    // settings cache — `write` only says that it moved.
    let _writing = WRITES.lock().await;
    let handle = Handle::new(documents.pool().clone());
    let row = match Settings::read_row(&handle).await {
        Ok(row) => row,
        // Nothing to hand over: a database whose row cannot be read has no
        // values a document could state, and a start reaches this after the row
        // exists.
        Err(error) => {
            tracing::warn!(
                %error,
                "Skipping the settings handover: the row cannot be read"
            );
            return true;
        }
    };
    let values = match serde_json::to_value(row.normalize())
        .and_then(serde_json::from_value::<Document>)
    {
        Ok(values) => values,
        Err(error) => {
            tracing::warn!(
                %error,
                "Skipping the settings handover: the row does not read as a document"
            );
            return false;
        }
    };
    let defaults = match default_values() {
        Ok(defaults) => defaults,
        Err(error) => {
            tracing::warn!(
                %error,
                "Skipping the settings handover: the defaults cannot be read"
            );
            return false;
        }
    };
    let mut ready = true;
    for &(name, keys, schema_version) in DOMAINS {
        // A document of its own is the sign a start has already handed this
        // domain over: stating the row above it would undo what the user set
        // since.
        if documents
            .stored::<Document>(name, schema_version)
            .await
            .is_present()
        {
            continue;
        }
        let shipped = match documents.shipped_keys(name, schema_version).await {
            Shipped::Keys(shipped) => shipped,
            Shipped::Unreadable => {
                tracing::warn!(
                    document = name,
                    "Skipping a settings domain: the deployment's own document cannot be read, so the keys it answers are unknown"
                );
                ready = false;
                continue;
            }
            Shipped::Missing => Document::new(),
        };
        let target = domain_values(keys, &values, &shipped, &defaults, None);
        // A domain with nothing of its own to say keeps being answered by the
        // row: an empty document would take that answer away before any writer
        // fills it.
        if target.is_empty() {
            continue;
        }
        // A domain that cannot be written costs that domain and no other, and
        // keeps the start from calling the handover finished.
        if let Err(error) =
            write(documents, name, schema_version, &target).await
        {
            tracing::warn!(
                document = name,
                %error,
                "Failed to hand a settings domain over to its document"
            );
            ready = false;
        }
    }
    ready
}

/// Drops what a local document repeats of a value the deployment or the build
/// already provides. Writing a value into a deployment's own document is how
/// its owner says a key should follow it, so the local copy stops shadowing it
/// as soon as the two agree, and a later change of that value still arrives.
///
/// Only a domain the deployment ships a readable document for is touched: with
/// no document of its own a domain has nothing a later change could be
/// shadowed from, and a local entry there is the user pinning the value, even
/// when what it pins is the build's default.
///
/// A start runs this once, after the handover. It is not part of a read: a read
/// never writes to disk.
pub(crate) async fn prune_redundant(
    documents: &Documents<'_>,
) -> crate::Result<()> {
    // With no deployment there is nothing a local copy could shadow: what it
    // repeats is the value the build default gives it anyway.
    if !documents.ships_documents() {
        return Ok(());
    }
    let defaults = default_values()?;
    let _writing = WRITES.lock().await;
    for &(name, keys, schema_version) in DOMAINS {
        let Some(config) =
            documents.shipped::<Document>(name, schema_version).await
        else {
            continue;
        };
        // A domain with no local document has nothing to drop, and one a newer
        // build wrote is not this build's to rewrite.
        let Stored::Ready { document } =
            documents.stored::<Document>(name, schema_version).await
        else {
            continue;
        };
        let mut pruned = document;
        let before = pruned.len();
        for &key in keys {
            let Some(value) = pruned.get(key).cloned() else {
                continue;
            };
            let Some(provided) = provided(key, &config, &defaults) else {
                continue;
            };
            // Two values that are equal before normalisation are equal after
            // it too, and that is the common case, so the projection is only
            // built when the documents disagree.
            let redundant = provided == &value
                || effective(key, provided, &defaults)
                    == effective(key, &value, &defaults);
            if redundant {
                pruned.remove(key);
            }
        }
        if pruned.len() == before {
            continue;
        }
        // A domain whose document this build cannot replace costs that domain
        // and no other: the rest still lose their repeated entries, and a
        // failed cleanup never keeps a start from coming up.
        if let Err(error) =
            write(documents, name, schema_version, &pruned).await
        {
            tracing::warn!(
                document = name,
                %error,
                "Failed to drop the settings entries a provided value covers"
            );
            continue;
        }
    }
    Ok(())
}

/// Writes a document for every domain from `settings`, leaving out the keys
/// that hold the value the domain is provided with.
///
/// A document is the authority for its domain, so a write states the whole
/// domain rather than the keys that changed: a key equal to the provided value
/// is dropped instead of written, because leaving it out reads the same value
/// and lets a later change of a provided value still arrive. A key the
/// deployment answers is dropped whatever value it holds: a local copy would be
/// the last word for it, so the deployment's own change would never reach the
/// user. Only a write that names that key may take it over, which `store_domain`
/// does.
///
/// A domain's target is compared against what its document already holds
/// before that document is serialized, so a store that changes nothing in it
/// touches no file: storing the same settings twice leaves every document as
/// the first store left it, down to the version it records.
///
/// A domain whose document cannot be replaced costs that domain and no other:
/// the rest are still written, and a failed write is left in the log rather
/// than handed to the caller, which is what a start can act on.
pub(crate) async fn store(
    documents: &Documents<'_>,
    settings: &Settings,
) -> crate::Result<()> {
    let defaults = default_values()?;
    let values: Document = serde_json::from_value(serde_json::to_value(
        settings.clone().normalize(),
    )?)?;
    let _writing = WRITES.lock().await;
    for &(name, keys, schema_version) in DOMAINS {
        let config = documents
            .shipped::<Document>(name, schema_version)
            .await
            .unwrap_or_default();
        let target = domain_values(keys, &values, &config, &defaults, None);
        let existing =
            match documents.stored::<Document>(name, schema_version).await {
                Stored::Ready { document } => Some(document),
                // Nothing to compare against is a document the write creates.
                Stored::Missing => None,
                // Writing either would lose what it holds: a shape a newer build
                // wrote is never lowered, and a file this build cannot read is
                // left for its owner rather than replaced.
                Stored::Newer { .. } => {
                    tracing::warn!(
                        document = name,
                        "Leaving a settings document a newer build wrote alone"
                    );
                    continue;
                }
                Stored::Unreadable => {
                    tracing::warn!(
                        document = name,
                        "Leaving a settings document that cannot be read alone"
                    );
                    continue;
                }
            };
        if existing.as_ref() == Some(&target) {
            continue;
        }
        // A domain with nothing to record and no document of its own keeps being
        // answered by the row: creating an empty one would take that answer away
        // before the domain has a writer that fills it.
        if existing.is_none() && target.is_empty() {
            continue;
        }
        // A domain this build cannot write costs that domain and no other.
        if let Err(error) =
            write(documents, name, schema_version, &target).await
        {
            tracing::warn!(
                document = name,
                %error,
                "Failed to write a settings document"
            );
            continue;
        }
    }
    Ok(())
}

/// Writes the document of one domain from the row, and no other domain's.
///
/// This is what a writer calls for a domain whose writes have moved to its
/// document: the domains still answered by the row have the row as their only
/// place, so stating them here as well would write files nothing reads yet.
///
/// The row is read here, inside `WRITES`, rather than handed in: two writers of
/// this process reach it in either order, and settings read before the lock are
/// a snapshot the other one may already have replaced — the older of the two
/// would then be the one the file states. What is written is the row as it
/// stands at the write, so the file states the latest committed settings
/// whatever order the writers arrived in.
///
/// One launcher runs at a time: the shell refuses a second instance, so nothing
/// here takes a lock against another process.
///
/// The rules are `store`'s, applied to one domain rather than to all of them: a
/// key holding what the domain is provided with is left out, and a domain with
/// nothing to say and no document of its own keeps being answered by the row.
/// Whether the document on disk already holds what would be written is `write`'s
/// question: it reads the file again and leaves alone one that changes nothing.
///
/// `changed` names the keys the row write carried, and a key the deployment
/// answers is stated only when it is one of them: the user changing a key is the
/// one way the local document may take it over.
///
/// A document this build cannot read is left as it stands and reported in the
/// log: what it holds is data a write would overwrite. One a newer build wrote is
/// not exempted here — `write` is where that file is refused.
///
/// A write that fails takes the domain's document with it, and the failure is
/// reported in the log rather than to the caller: the row is already committed,
/// so the change has landed, and a document left behind would go on answering
/// the domain with the value it froze. Removing it gives the domain back to the
/// row, which is the value that just landed.
///
/// The name is a domain `DOMAINS` lists, so one it does not is a mistake in the
/// caller rather than something a user can cause, and it is answered as one.
pub(crate) async fn store_domain(
    documents: &Documents<'_>,
    name: &str,
    handle: &Handle,
    changed: Option<&BTreeSet<String>>,
) -> crate::Result<()> {
    let Some(&(domain, keys, schema_version)) =
        DOMAINS.iter().find(|(domain, _, _)| *domain == name)
    else {
        return Err(crate::ErrorKind::OtherError(format!(
            "No settings domain is named {name}"
        ))
        .into());
    };
    let defaults = default_values()?;
    let _writing = WRITES.lock().await;
    let config = match documents.shipped_keys(domain, schema_version).await {
        Shipped::Keys(config) => config,
        // Which keys the deployment answers is unknown while its own document
        // cannot be read, so the domain is skipped rather than stated: a copy of
        // one of them would shadow it once the file could be read again. The
        // failure is worth reporting, so the caller counts it as unfinished.
        Shipped::Unreadable => {
            return Err(crate::ErrorKind::OtherError(format!(
                "The deployment's document for {domain} cannot be read"
            ))
            .into());
        }
        Shipped::Missing => Document::new(),
    };
    let target =
        row_values_of_domain(handle, keys, &config, &defaults, changed).await?;
    // Only whether a document is there matters here: a shape a newer build wrote
    // and a file this build cannot read are both `write`'s to leave alone, and it
    // reads the file again, which is the answer that still holds when another
    // writer of this process has replaced it since this read.
    let existing =
        match documents.stored::<Document>(domain, schema_version).await {
            Stored::Ready { document } | Stored::Newer { document } => {
                Some(document)
            }
            Stored::Missing | Stored::Unreadable => None,
        };
    // A domain with nothing to record and no document of its own keeps being
    // answered by the row: creating an empty one would take that answer away
    // before the domain's values have all been written to it.
    if existing.is_none() && target.is_empty() {
        return Ok(());
    }
    write_or_discard(
        documents,
        domain,
        write(documents, domain, schema_version, &target),
    )
    .await
}

/// States the document of the domains whose writes have moved, from the row as
/// it stands.
///
/// This is the one thing a writer of the settings row has to call once the row
/// has landed: the domains in `CUT_OVER` are answered by their documents, so a
/// row write that did not reach them would show the value before it. The row is
/// read again by each domain rather than handed through, so a writer that
/// committed in between is the newer of the two and the file it wrote stands.
///
/// `changed` names the keys the row write carried, and only the domains carrying
/// one of them are stated: a domain a write did not touch keeps the document it
/// has, which is what a hand edit of a value beside the one that changed
/// survives. `None` states every domain, which is what a writer that cannot say
/// which keys it changed states — and even then the keys a deployment answers
/// are left out of the local documents.
///
/// A domain that cannot be stated costs itself and no other: the failure is
/// reported in the log and the loop goes on, so one damaged deployment document
/// does not leave the domains after it behind. What a failed write leaves is not
/// a stale document either — `store_domain` removes it, so the domain is
/// answered by the row that just landed.
pub(crate) async fn store_cut_over(
    handle: &Handle,
    changed: Option<&BTreeSet<String>>,
) {
    // With no document directory named, a start reads the row alone, which is
    // what a unit test that builds its own handle does: there is no document to
    // state and nothing is missing.
    let Some(documents) = handle.settings_documents() else {
        return;
    };
    for &name in CUT_OVER {
        if changed.is_some_and(|changed| !carries(name, changed)) {
            continue;
        }
        if let Err(error) =
            store_domain(&documents, name, handle, changed).await
        {
            tracing::warn!(
                document = name,
                %error,
                "Failed to state the settings document of a domain whose writes have moved"
            );
        }
    }
}

/// Whether a domain of `CUT_OVER` carries one of the keys a row write changed.
fn carries(name: &str, changed: &BTreeSet<String>) -> bool {
    DOMAINS
        .iter()
        .find(|(domain, _, _)| *domain == name)
        .is_some_and(|(_, keys, _)| {
            keys.iter().any(|key| changed.contains(*key))
        })
}

/// A domain's write along with what a failed one leaves behind.
async fn write_or_discard(
    documents: &Documents<'_>,
    name: &str,
    write: impl Future<Output = crate::Result<()>>,
) -> crate::Result<()> {
    match write.await {
        Ok(()) => Ok(()),
        Err(error) => {
            discard(documents, name, &error).await;
            Ok(())
        }
    }
}

/// Removes a domain's document after the write meant to replace it failed, so
/// the domain is answered by the row again rather than by the file the failed
/// write left behind.
///
/// The row is already committed when this runs, so the value that just landed is
/// the one the read has to answer with. A document that cannot be removed
/// either is left where it is and reported: what it holds is in the log, and the
/// change itself is in the row, which the next write of the domain states again.
async fn discard(documents: &Documents<'_>, name: &str, error: &crate::Error) {
    let path = documents.path(name);
    match tokio::fs::remove_file(&path).await {
        Ok(()) => invalidate(),
        Err(remove) if remove.kind() == std::io::ErrorKind::NotFound => {}
        Err(remove) => tracing::warn!(
            path = %path.display(),
            %remove,
            "Failed to remove a settings document whose write failed"
        ),
    }
    tracing::warn!(
        document = name,
        path = %path.display(),
        %error,
        "Failed to write a settings document, so its domain is answered by the row"
    );
}

/// The keys of one domain as its document should carry them, taken from a
/// whole `Settings` keyed the way a document keys them.
///
/// A key the deployment answers is not written at all: a local copy would be the
/// last word for it, so the deployment's own later change would never reach the
/// user. `changed` names the keys a write carries, and naming such a key is the
/// one way it is stated — the user changing it is the override no other path
/// makes.
///
/// A key the deployment says nothing about is left out while it holds the value
/// the build defaults to: leaving it out reads the same value and lets a later
/// change of that value arrive.
fn domain_values(
    keys: &[&str],
    values: &Document,
    config: &Document,
    defaults: &Document,
    changed: Option<&BTreeSet<String>>,
) -> Document {
    let mut target = Document::new();
    for &key in keys {
        let Some(value) = values.get(key) else {
            continue;
        };
        let written = if let Some(shipped) = config.get(key) {
            // Naming the key is what lets a local copy outrank a deployment's,
            // and a copy that says what the deployment already says buys nothing
            // while it keeps the deployment's later change from arriving.
            changed.is_some_and(|changed| changed.contains(key))
                && !same_setting(key, shipped, value, defaults)
        } else {
            !defaults.get(key).is_some_and(|default| {
                same_setting(key, default, value, defaults)
            })
        };
        if written {
            target.insert(key.to_string(), value.clone());
        }
    }
    target
}

/// Writes one key into the document of the domain that lists it, leaving the
/// rest of that document as it stands.
///
/// A domain with no document yet has been answered by the row, so the first
/// write that gives it one starts from what the row says for the rest of the
/// domain: writing the single key into an empty document would make the domain
/// that document's alone and drop every key beside it to the build default.
pub(crate) async fn store_key(
    documents: &Documents<'_>,
    handle: &Handle,
    key: &str,
    value: Value,
) -> crate::Result<()> {
    let Some((name, keys, schema_version)) = domain_of(key) else {
        // A key no domain lists is not stored in a document at all: the row is
        // the only place it can live, so there is nothing to write here.
        tracing::debug!(key, "Ignoring a settings key no domain lists");
        return Ok(());
    };
    let defaults = default_values()?;
    let _writing = WRITES.lock().await;
    let config = documents
        .shipped::<Document>(name, schema_version)
        .await
        .unwrap_or_default();
    let Some((mut document, existed)) =
        writable_document(documents, name, schema_version).await
    else {
        return Ok(());
    };
    if !existed {
        // The key this write names is the change it carries, so it is the one a
        // deployment's own key may be stated for.
        let named = BTreeSet::from([key.to_string()]);
        document = row_values_of_domain(
            handle,
            keys,
            &config,
            &defaults,
            Some(&named),
        )
        .await?;
    }
    let says_nothing = provided(key, &config, &defaults)
        .is_some_and(|provided| same_setting(key, provided, &value, &defaults));
    let changed = if says_nothing {
        document.remove(key).is_some()
    } else {
        let previous = document.insert(key.to_string(), value.clone());
        previous.as_ref() != Some(&value)
    };
    if !changed {
        return Ok(());
    }
    write(documents, name, schema_version, &document).await
}

/// The keys of one domain as the row holds them, read afresh rather than handed
/// in: both the single-key writer of a domain with no document and the writer
/// that states a whole domain start from it.
///
/// It goes through `domain_values` rather than copying the row: a key holding
/// what the domain is provided with is left out, exactly as `store` leaves it
/// out, so the document this seeds is the one a full store would write.
///
/// A key the deployment answers is left out as well unless `changed` names it:
/// the local copy would be the last word for that key, so the deployment's own
/// later change would never arrive. A key the write does name is the user
/// overriding the deployment, which no other path does.
async fn row_values_of_domain(
    handle: &Handle,
    keys: &[&str],
    config: &Document,
    defaults: &Document,
    changed: Option<&BTreeSet<String>>,
) -> crate::Result<Document> {
    let row: Document = serde_json::from_value(serde_json::to_value(
        Settings::read_row(handle).await?,
    )?)?;
    Ok(domain_values(keys, &row, config, defaults, changed))
}

/// Clears `key` while the document still holds exactly `expected`, which is
/// what keeps a setting from referring to something that no longer exists.
///
/// A document carries no foreign key to null the reference the way the row's
/// `minimal_home_instance_id` column does, so the writer that removes the
/// thing a key names clears it here. The value is compared rather than simply
/// dropped: a reference the user has already pointed elsewhere is theirs to
/// keep, and clearing it would undo their choice.
pub(crate) async fn clear_key_if(
    documents: &Documents<'_>,
    key: &str,
    expected: &str,
) -> crate::Result<()> {
    let Some((name, _, schema_version)) = domain_of(key) else {
        tracing::debug!(key, "Ignoring a settings key no domain lists");
        return Ok(());
    };
    let _writing = WRITES.lock().await;
    // Only a document this build can rewrite is cleared: one it cannot read
    // and one a newer build wrote are both read-only here.
    let Some((mut document, _)) =
        writable_document(documents, name, schema_version).await
    else {
        return Ok(());
    };
    if document.get(key).and_then(Value::as_str) != Some(expected) {
        return Ok(());
    }
    document.remove(key);
    write(documents, name, schema_version, &document).await
}

/// The domain whose document lists `key`, with that document's shape version.
/// A key no domain lists is not stored in a document at all.
fn domain_of(
    key: &str,
) -> Option<(&'static str, &'static [&'static str], u32)> {
    DOMAINS
        .iter()
        .find(|(_, keys, _)| keys.contains(&key))
        .copied()
}

/// Writes one document, and says that what a read composes from has changed.
///
/// Every writer of a settings document goes through here, so a change one
/// carries reaches the next read without a caller having to say so.
async fn write(
    documents: &Documents<'_>,
    name: &str,
    schema_version: u32,
    document: &Document,
) -> crate::Result<()> {
    documents.write(name, schema_version, document).await?;
    invalidate();
    Ok(())
}

/// The local document a single-key writer starts from, and whether it was
/// already there, or nothing when the document must not be written.
async fn writable_document(
    documents: &Documents<'_>,
    name: &str,
    schema_version: u32,
) -> Option<(Document, bool)> {
    match documents.stored::<Document>(name, schema_version).await {
        Stored::Ready { document } => Some((document, true)),
        // The write of a domain with no document creates it.
        Stored::Missing => Some((Document::new(), false)),
        // Writing either would lose what it holds: a shape a newer build wrote
        // is never lowered, and a file this build cannot read is left for its
        // owner rather than replaced.
        Stored::Newer { .. } => {
            tracing::warn!(
                document = name,
                "Leaving a settings document a newer build wrote alone"
            );
            None
        }
        Stored::Unreadable => {
            tracing::warn!(
                document = name,
                "Leaving a settings document that cannot be read alone"
            );
            None
        }
    }
}

/// Whether two documents say the same thing about `key`.
///
/// Two values that differ as written can still be one setting: a count of `0`
/// reads as the lower bound its type holds, so a document saying `0` and one
/// saying that bound are the same answer. This is the comparison
/// `prune_redundant` makes, which keeps a writer from storing a key a start
/// would then drop as redundant.
fn same_setting(
    key: &str,
    left: &Value,
    right: &Value,
    defaults: &Map<String, Value>,
) -> bool {
    left == right
        || effective(key, left, defaults) == effective(key, right, defaults)
}

/// The settings as the row holds them, and the same ones keyed the way a
/// document keys them.
///
/// The row is read directly rather than through `Settings::get`, which composes
/// these documents and would come back here.
async fn row_values(
    handle: &Handle,
) -> crate::Result<(Settings, Map<String, Value>)> {
    let settings = Settings::read_row(handle).await?;
    let values = serde_json::from_value(serde_json::to_value(&settings)?)?;
    Ok((settings, values))
}

/// Lays the row's values for `keys` over `values`.
///
/// Unlike a document, nothing here can be unreadable: these values come from a
/// `Settings`, so they already carry the types their keys take.
fn contribute(
    values: &mut Map<String, Value>,
    keys: &[&str],
    row: &Map<String, Value>,
) {
    for &key in keys {
        if let Some(value) = row.get(key) {
            values.insert(key.to_string(), value.clone());
        }
    }
}

/// Lays one layer over `values`, for the keys `keys` lists only: a key outside
/// that list is ignored, and so is a value the type cannot take, which costs
/// that key and leaves what the layer below gave it. A negative number is
/// retried as zero first, so it costs no key at all.
fn overlay(
    values: &mut Map<String, Value>,
    keys: &[&str],
    source: &Document,
    name: &str,
) {
    for &key in keys {
        let Some(value) = source.get(key) else {
            continue;
        };
        let kept = values.insert(key.to_string(), value.clone());
        let mut unreadable = settings_error(values);

        // A negative number is the one value the type cannot take that still
        // has a nearest value inside its range. Read as zero it lands on that
        // field's lower bound once `normalize` runs, where dropping the key
        // would put back the default, which is not the nearest value to it.
        if unreadable.is_some() && is_negative_integer(value) {
            values.insert(key.to_string(), Value::from(0));
            unreadable = settings_error(values);
            if unreadable.is_none() {
                tracing::warn!(
                    document = name,
                    key,
                    "Reading a negative settings value as its lower bound"
                );
                continue;
            }
        }

        if let Some(error) = unreadable {
            if let Some(kept) = kept {
                values.insert(key.to_string(), kept);
            } else {
                values.remove(key);
            }
            tracing::warn!(
                document = name,
                key,
                %error,
                "Ignoring a settings value that cannot be read"
            );
        }
    }
}

/// Why `values` does not read as `Settings`, when they do not.
fn settings_error(values: &Map<String, Value>) -> Option<serde_json::Error> {
    serde_json::from_value::<Settings>(Value::Object(values.clone())).err()
}

/// Whether a document holds a signed integer below zero. `as_i64` also rules
/// out every other kind of value, so a string or a bool is not retried.
fn is_negative_integer(value: &Value) -> bool {
    value.as_i64().is_some_and(|number| number < 0)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::api::settings::patch_with;
    use crate::state::documents::{
        DIR_NAME, ai,
        test_support::{create_ai_tables, deploy, pool as empty_pool},
    };
    use crate::state::settings::Theme;
    use crate::state::store::settings::Settings as StoredSettings;
    use crate::state::store::tests::support::migrated_pool;

    fn document_keys() -> BTreeSet<&'static str> {
        DOMAINS
            .iter()
            .flat_map(|(_, keys, _)| keys.iter().copied())
            .collect()
    }

    /// The keys no document carries: the row answers them alone, and a document
    /// that could answer one would answer something the row decides. `version`
    /// is what `Settings::migrate` reads to choose its migrations, so a document
    /// carrying it would let a file skip them.
    const ROW_ONLY: &[&str] = &["version"];

    /// The pair of directories a launcher reads: the one it writes, and the one
    /// a deployment ships.
    fn layered<'a>(
        local: &tempfile::TempDir,
        deployment: &tempfile::TempDir,
        pool: &'a sqlx::SqlitePool,
    ) -> Documents<'a> {
        Documents::at(local.path().to_path_buf(), pool)
            .with_config_root(Some(deployment.path().to_path_buf()))
    }

    #[tokio::test]
    async fn the_domains_carry_every_key_the_settings_serialize() {
        let pool = migrated_pool().await;
        let settings = Settings::get(&Handle::new(pool)).await.unwrap();
        let serialized: BTreeSet<String> = match serde_json::to_value(&settings)
            .unwrap()
        {
            Value::Object(map) => map.into_iter().map(|(key, _)| key).collect(),
            other => panic!("the settings serialize as {other:?}"),
        };
        let documents = document_keys();

        let unlisted: Vec<&String> = serialized
            .iter()
            .filter(|key| !documents.contains(key.as_str()))
            .filter(|key| !ROW_ONLY.contains(&key.as_str()))
            .collect();
        assert!(unlisted.is_empty(), "keys no domain carries: {unlisted:?}");

        let absent: Vec<&str> = ROW_ONLY
            .iter()
            .copied()
            .filter(|key| !serialized.contains(*key))
            .collect();
        assert!(
            absent.is_empty(),
            "keys left out of every domain that the settings do not carry either: {absent:?}"
        );

        let unknown: Vec<&str> = documents
            .iter()
            .filter(|key| !serialized.contains(**key))
            .copied()
            .collect();
        assert!(
            unknown.is_empty(),
            "keys the settings do not serialize: {unknown:?}"
        );
    }

    #[test]
    fn no_key_belongs_to_two_domains() {
        let mut carried = BTreeSet::new();
        let mut repeated = Vec::new();
        for (name, keys, _) in DOMAINS {
            for key in *keys {
                if !carried.insert(*key) {
                    repeated.push(format!("{key} of {name}"));
                }
            }
        }
        assert!(repeated.is_empty(), "keys two domains carry: {repeated:?}");
    }

    #[tokio::test]
    async fn a_stored_key_is_read_and_an_unreadable_value_costs_one_key() {
        let pool = migrated_pool().await;
        let handle = Handle::new(pool.clone());
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("backup.json"),
            r#"{"schema_version":1,"data":{"backup_repository_path":"/backups","nonsense":1}}"#,
        )
        .unwrap();
        std::fs::write(
            root.path().join("appearance.json"),
            r#"{"schema_version":1,"data":{"custom_background_blur":"lots","theme":"light"}}"#,
        )
        .unwrap();

        let documents = Documents::at(root.path().to_path_buf(), &pool);
        let settings = read(&documents, &handle).await.unwrap();

        assert_eq!(
            settings.backup_repository_path.as_deref(),
            Some("/backups"),
            "a key the document carries is read from it"
        );
        assert_eq!(
            settings.theme.as_str(),
            "light",
            "a key beside an unreadable one still comes from the same document"
        );
        assert_ne!(
            settings.theme.as_str(),
            Settings::default().theme.as_str(),
            "the document answers that key, not the default"
        );
        assert_eq!(
            settings.custom_background_blur,
            Settings::default().custom_background_blur,
            "a value the type cannot take costs its own key only"
        );
    }

    /// A key whose value the type cannot take is not a document the read
    /// cannot use: the read is a whole `Settings`, so a value of the wrong type
    /// where a struct belongs would fail it outright if the layers were laid
    /// down without checking each key. `overlay` checks one key at a time and
    /// drops the one that costs, so the read answers and the rest of the
    /// document still counts.
    #[tokio::test]
    async fn a_value_of_the_wrong_type_costs_only_its_key() {
        let pool = migrated_pool().await;
        let handle = Handle::new(pool.clone());
        let settings_dir = tempfile::tempdir().unwrap();
        identify(settings_dir.path());
        // `memory` is a nested struct, so a number here is legal JSON that no
        // `MemorySettings` can be read from.
        write_document(
            settings_dir.path(),
            "game",
            r#""memory":5,"force_fullscreen":true"#,
        );

        let settings = Settings::get(&handle)
            .await
            .expect("a value of the wrong type costs its key, not the read");

        assert_eq!(
            settings.memory.maximum,
            Settings::default().memory.maximum,
            "the key whose value cannot be read keeps the default"
        );
        assert!(
            settings.force_fullscreen,
            "a key beside the one that costs still comes from the document"
        );
    }

    /// A document is text the user can edit by hand, so a number outside the
    /// range its setting holds reads back as the nearest value inside it rather
    /// than as what the file says.
    #[tokio::test]
    async fn a_hand_edited_out_of_range_value_reads_as_the_bounded_one() {
        let pool = migrated_pool().await;
        let handle = Handle::new(pool.clone());
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("appearance.json"),
            r#"{"schema_version":1,"data":{"custom_background_blur":999,"sidebar_instance_count":999}}"#,
        )
        .unwrap();
        std::fs::write(
            root.path().join("download.json"),
            r#"{"schema_version":1,"data":{"max_concurrent_downloads":0}}"#,
        )
        .unwrap();

        let documents = Documents::at(root.path().to_path_buf(), &pool);
        let settings = read(&documents, &handle).await.unwrap();

        assert_eq!(
            settings.custom_background_blur, 40,
            "a blur past its highest value reads as that value"
        );
        assert_eq!(
            settings.sidebar_instance_count, 50,
            "a sidebar count past its highest value reads as that value"
        );
        assert_eq!(
            settings.max_concurrent_downloads, 1,
            "a download count below its lowest value reads as that value"
        );
    }

    /// A negative number is the one out-of-range value a document can hold that
    /// has a nearest value inside its field: it reads as that field's lower
    /// bound, not as the default the key would fall back to if it were dropped.
    #[tokio::test]
    async fn a_negative_document_value_reads_as_the_lower_bound() {
        let pool = migrated_pool().await;
        let handle = Handle::new(pool.clone());
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("download.json"),
            r#"{"schema_version":1,"data":{"max_concurrent_downloads":-5}}"#,
        )
        .unwrap();

        let documents = Documents::at(root.path().to_path_buf(), &pool);
        let settings = read(&documents, &handle).await.unwrap();

        assert_eq!(
            settings.max_concurrent_downloads, 1,
            "a negative download count reads as its lowest value"
        );
        assert_ne!(
            settings.max_concurrent_downloads,
            Settings::default().max_concurrent_downloads,
            "the key is read rather than dropped back to its default"
        );
    }

    /// A document that leaves `data` out is a file this build cannot read, not
    /// an empty authority: the rows keep answering the domain.
    #[tokio::test]
    async fn a_document_without_data_is_unreadable() {
        let pool = migrated_pool().await;
        let handle = Handle::new(pool.clone());
        let mut row = Settings::get(&handle).await.unwrap();
        row.locale = "zh_cn".to_string();
        row.update(&handle).await.unwrap();

        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("appearance.json");
        let without_data = br#"{"schema_version":1}"#;
        std::fs::write(&path, without_data).unwrap();

        let documents = Documents::at(root.path().to_path_buf(), &pool);
        let settings = read(&documents, &handle).await.unwrap();

        assert_eq!(
            settings.locale, "zh_cn",
            "a document without `data` is unreadable, so the row answers the domain"
        );
        assert_ne!(
            settings.locale,
            Settings::default().locale,
            "the row answers that key rather than the default"
        );
        assert_eq!(
            std::fs::read(&path).unwrap().as_slice(),
            without_data.as_slice(),
            "a document that cannot be read is left alone"
        );
    }

    /// `personalized_ads` is in no document, so the row answers it even when
    /// every domain has one: the layering the other keys follow never reaches
    /// it (Q5).
    #[tokio::test]
    async fn the_ads_preference_comes_from_the_row() {
        let pool = migrated_pool().await;
        let handle = Handle::new(pool.clone());
        let mut row = Settings::get(&handle).await.unwrap();
        row.personalized_ads = true;
        row.update(&handle).await.unwrap();

        let root = tempfile::tempdir().unwrap();
        for (name, _, _) in DOMAINS {
            std::fs::write(
                root.path().join(format!("{name}.json")),
                r#"{"schema_version":1,"data":{}}"#,
            )
            .unwrap();
        }

        let documents = Documents::at(root.path().to_path_buf(), &pool);
        let settings = read(&documents, &handle).await.unwrap();

        assert!(
            settings.personalized_ads,
            "every domain has a document, and the ads preference still comes from the row"
        );
    }

    /// `version` is the other key the row answers alone, and for the opposite
    /// reason: `Settings::migrate` reads it to decide which migrations to run,
    /// so a document that could answer it would let a hand-edited file skip
    /// them. It is read from the row whatever every document says, and a store
    /// writes no document carrying it.
    #[tokio::test]
    async fn a_document_cannot_skip_the_settings_migrations() {
        let pool = migrated_pool().await;
        let handle = Handle::new(pool.clone());
        let row = Settings::read_row(&handle).await.unwrap();
        let settings_dir = tempfile::tempdir().unwrap();
        identify(settings_dir.path());
        write_document(settings_dir.path(), "state", r#""version":999"#);

        let settings = Settings::get(&handle).await.unwrap();
        assert_eq!(
            settings.version, row.version,
            "the version a read answers is the row's, whatever the document says"
        );
        assert_ne!(
            settings.version, 999,
            "a document cannot raise the settings past the migrations it would run"
        );

        let root = settings_dir.path().join(DIR_NAME);
        let documents = Documents::at(root.clone(), &pool);
        store(&documents, &settings).await.unwrap();

        let data = data(&root, "state");
        assert!(
            !data.contains_key("version"),
            "a store leaves the key the row owns out of the document: {data:?}"
        );
    }

    /// The row reaches the domains that have no document and stops there, so a
    /// key a document exists for cannot come back from a value the row froze.
    #[tokio::test]
    async fn the_row_answers_only_the_domains_without_a_document() {
        let pool = migrated_pool().await;
        let handle = Handle::new(pool.clone());
        let mut row = Settings::get(&handle).await.unwrap();
        row.locale = "zh_cn".to_string();
        row.backup_repository_path = Some("/from-the-row".to_string());
        row.update(&handle).await.unwrap();

        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("appearance.json"),
            r#"{"schema_version":1,"data":{"theme":"light"}}"#,
        )
        .unwrap();

        let documents = Documents::at(root.path().to_path_buf(), &pool);
        let settings = read(&documents, &handle).await.unwrap();

        assert_eq!(settings.theme.as_str(), "light", "the document answers");
        assert_eq!(
            settings.locale,
            Settings::default().locale,
            "appearance has a document, so a key it leaves out reads the \
             default rather than the row"
        );
        assert_eq!(
            settings.backup_repository_path.as_deref(),
            Some("/from-the-row"),
            "backup has no document, so the row answers for it"
        );
    }

    /// A deployment's document answers a domain no local document covers, and
    /// it answers above the row: a value the row froze cannot shadow it.
    #[tokio::test]
    async fn a_deployment_document_applies_without_a_local_one() {
        let pool = migrated_pool().await;
        let handle = Handle::new(pool.clone());
        let mut row = Settings::get(&handle).await.unwrap();
        row.theme = Theme::System;
        row.update(&handle).await.unwrap();

        let local = tempfile::tempdir().unwrap();
        let deployment = tempfile::tempdir().unwrap();
        std::fs::write(
            deployment.path().join("appearance.json"),
            r#"{"schema_version":1,"data":{"theme":"light"}}"#,
        )
        .unwrap();

        let documents = layered(&local, &deployment, &pool);
        let settings = read(&documents, &handle).await.unwrap();

        assert_eq!(
            settings.theme.as_str(),
            "light",
            "the deployment's document answers the domain the row would"
        );
        assert!(
            !local.path().join("appearance.json").exists(),
            "a read must not write the local directory"
        );
    }

    /// The local document is the last word for the keys it carries: what the
    /// deployment ships is a value below it, per key rather than per domain.
    #[tokio::test]
    async fn a_local_document_wins_over_the_deployment_one() {
        let pool = migrated_pool().await;
        let handle = Handle::new(pool.clone());
        let local = tempfile::tempdir().unwrap();
        let deployment = tempfile::tempdir().unwrap();
        std::fs::write(
            local.path().join("appearance.json"),
            r#"{"schema_version":1,"data":{"theme":"oled"}}"#,
        )
        .unwrap();
        std::fs::write(
            deployment.path().join("appearance.json"),
            r#"{"schema_version":1,"data":{"theme":"light","locale":"zh_cn"}}"#,
        )
        .unwrap();

        let documents = layered(&local, &deployment, &pool);
        let settings = read(&documents, &handle).await.unwrap();

        assert_eq!(
            settings.theme.as_str(),
            "oled",
            "the key the local document carries overrides the deployment's"
        );
        assert_eq!(
            settings.locale, "zh_cn",
            "a key it leaves out still comes from the deployment"
        );
    }

    /// A deployment's document this build cannot read is a layer that is not
    /// there: the row answers that domain and the file is left as it stands.
    #[tokio::test]
    async fn a_corrupt_deployment_document_is_ignored() {
        let pool = migrated_pool().await;
        let handle = Handle::new(pool.clone());
        let row = Settings::get(&handle).await.unwrap();
        let local = tempfile::tempdir().unwrap();
        let deployment = tempfile::tempdir().unwrap();
        let without_data = br#"{"schema_version":1,"theme":"light"}"#;
        std::fs::write(deployment.path().join("appearance.json"), without_data)
            .unwrap();
        std::fs::write(
            deployment.path().join("download.json"),
            r#"{"schema_version":1,"data":"nonsense"}"#,
        )
        .unwrap();
        std::fs::write(
            deployment.path().join("backup.json"),
            r#"{"schema_version":1,"data":{"backup_repository_path":"/shipped"}}"#,
        )
        .unwrap();

        let documents = layered(&local, &deployment, &pool);
        let settings = read(&documents, &handle).await.unwrap();

        assert_eq!(
            settings.backup_repository_path.as_deref(),
            Some("/shipped"),
            "one document this build cannot read costs its own domain only"
        );
        assert_eq!(
            settings.theme.as_str(),
            row.theme.as_str(),
            "a document without `data` is unreadable, so the row answers"
        );
        assert_eq!(
            settings.max_concurrent_downloads, row.max_concurrent_downloads,
            "`data` that is not an object is unreadable too"
        );
        assert_eq!(
            std::fs::read(deployment.path().join("appearance.json"))
                .unwrap()
                .as_slice(),
            without_data.as_slice(),
            "a read leaves a document it cannot interpret as it stands"
        );
    }

    /// The value a key reads before a local document is asked: what the
    /// deployment ships, and the build's own default when it ships nothing.
    #[test]
    fn provided_prefers_the_deployment_value() {
        let defaults = default_values().unwrap();
        let config =
            Map::from_iter([("theme".to_string(), Value::from("light"))]);

        assert_ne!(
            config.get("theme"),
            defaults.get("theme"),
            "the two layers disagree about the key"
        );
        assert_eq!(
            provided("theme", &config, &defaults),
            config.get("theme"),
            "the deployment's value comes first"
        );
        assert_eq!(
            provided("locale", &config, &defaults),
            defaults.get("locale"),
            "a key it ships nothing for falls back to the build's default"
        );
        assert_eq!(
            provided("no_such_key", &config, &defaults),
            None,
            "a key no layer carries has no provided value"
        );
    }

    /// The deployment's directory belongs to the deployment: a write lands in
    /// the local one, even for a domain the deployment ships a document for.
    #[cfg(unix)]
    #[tokio::test]
    async fn writing_never_touches_the_deployment_directory() {
        use std::os::unix::fs::PermissionsExt;

        let pool = empty_pool().await;
        create_ai_tables(&pool).await;
        let local = tempfile::tempdir().unwrap();
        let deployment = tempfile::tempdir().unwrap();
        let shipped = deployment.path().join("ai.json");
        // A key the write below does not name: the deployment's document then
        // answers beside the local one, which is what shows the write landing
        // in the local directory rather than merely shadowing the deployment.
        let contents = br#"{"schema_version":1,"data":{"legacy_openai_credential_cleanup":true}}"#;
        std::fs::write(&shipped, contents).unwrap();
        let modified = std::fs::metadata(&shipped).unwrap().modified().unwrap();
        std::fs::set_permissions(
            deployment.path(),
            std::fs::Permissions::from_mode(0o555),
        )
        .unwrap();

        let documents = layered(&local, &deployment, &pool);
        let written = ai::set_enabled(&documents, true).await;

        // Restored before anything is asserted: a directory left read-only
        // would keep the temporary one from being removed afterwards.
        std::fs::set_permissions(
            deployment.path(),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();

        written.unwrap();
        assert!(
            local.path().join("ai.json").is_file(),
            "the write lands in the local directory"
        );
        let read = ai::read(&documents).await.unwrap();
        assert!(
            read.enabled,
            "the local document holds the domain, so the write's value reads back"
        );
        assert!(
            read.legacy_openai_credential_cleanup,
            "the deployment's document still answers the key the local one leaves out"
        );
        assert_eq!(
            std::fs::read(&shipped).unwrap().as_slice(),
            contents.as_slice(),
            "a write must leave the deployment's document byte for byte"
        );
        assert_eq!(
            std::fs::metadata(&shipped).unwrap().modified().unwrap(),
            modified,
            "a write must not touch the deployment's document at all"
        );
        let listed = std::fs::read_dir(deployment.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        assert_eq!(listed, [std::ffi::OsString::from("ai.json")]);
    }

    /// A start drops the local entries a provided value already covers, so a
    /// change of that value reaches the settings again.
    #[tokio::test]
    async fn startup_drops_the_entries_a_deployment_value_provides() {
        let pool = migrated_pool().await;
        let local = tempfile::tempdir().unwrap();
        let deployment = tempfile::tempdir().unwrap();
        let written = local.path().join("appearance.json");
        std::fs::write(
            &written,
            r#"{"schema_version":1,"data":{"theme":"light","locale":"zh_cn"}}"#,
        )
        .unwrap();
        std::fs::write(
            deployment.path().join("appearance.json"),
            r#"{"schema_version":1,"data":{"theme":"light"}}"#,
        )
        .unwrap();

        let documents = layered(&local, &deployment, &pool);
        prune_redundant(&documents).await.unwrap();

        let stored: Value =
            serde_json::from_slice(&std::fs::read(&written).unwrap()).unwrap();
        let data = stored["data"].as_object().unwrap();
        assert!(
            !data.contains_key("theme"),
            "the key the deployment provides is dropped from the local document"
        );
        assert_eq!(
            data.get("locale"),
            Some(&Value::from("zh_cn")),
            "a key only the user set stays where it is"
        );

        let settings =
            read(&documents, &Handle::new(pool.clone())).await.unwrap();
        assert_eq!(
            settings.theme.as_str(),
            "light",
            "the deployment still answers the key the local copy stopped carrying"
        );
        assert_eq!(settings.locale, "zh_cn");
    }

    /// A local entry is only redundant against a document the deployment
    /// really ships: a domain it ships nothing for has nothing a later change
    /// could be shadowed from, so what the local document carries there is the
    /// user pinning the value, even when the value is the build's own default.
    #[tokio::test]
    async fn a_domain_without_a_shipped_document_keeps_its_local_entries() {
        let pool = migrated_pool().await;
        let local = tempfile::tempdir().unwrap();
        let deployment = tempfile::tempdir().unwrap();
        let written = local.path().join("appearance.json");
        let contents = br#"{"schema_version":1,"data":{"theme":"dark"}}"#;
        std::fs::write(&written, contents).unwrap();
        // One document is enough for the deployment to count as one, and it
        // belongs to another domain.
        std::fs::write(
            deployment.path().join("backup.json"),
            r#"{"schema_version":1,"data":{"backup_repository_path":"/shipped"}}"#,
        )
        .unwrap();

        let documents = layered(&local, &deployment, &pool);
        prune_redundant(&documents).await.unwrap();

        assert_eq!(
            Settings::default().theme.as_str(),
            "dark",
            "the entry pins the value the build defaults to, so only the \
             shipped document can make it redundant"
        );
        assert_eq!(
            std::fs::read(&written).unwrap().as_slice(),
            contents.as_slice(),
            "a domain the deployment ships no document for keeps its local entries"
        );
    }

    /// Two files that say different numbers can be the same setting: a value
    /// is read through `normalize`, so a download count of `0` reads as the
    /// lower bound of `1`. A local `1` therefore repeats what the deployment
    /// provides and has to go, or it would keep answering after the deployment
    /// raises its own value.
    #[tokio::test]
    async fn a_redundant_entry_is_pruned_after_normalisation() {
        let pool = migrated_pool().await;
        let local = tempfile::tempdir().unwrap();
        let deployment = tempfile::tempdir().unwrap();
        let written = local.path().join("download.json");
        std::fs::write(
            &written,
            r#"{"schema_version":1,"data":{"max_concurrent_downloads":1}}"#,
        )
        .unwrap();
        std::fs::write(
            deployment.path().join("download.json"),
            r#"{"schema_version":1,"data":{"max_concurrent_downloads":0}}"#,
        )
        .unwrap();

        let documents = layered(&local, &deployment, &pool);
        prune_redundant(&documents).await.unwrap();

        let stored: Value =
            serde_json::from_slice(&std::fs::read(&written).unwrap()).unwrap();
        assert!(
            !stored["data"]
                .as_object()
                .unwrap()
                .contains_key("max_concurrent_downloads"),
            "the entry repeats a value the deployment provides once it is read"
        );
        let settings =
            read(&documents, &Handle::new(pool.clone())).await.unwrap();
        assert_eq!(
            settings.max_concurrent_downloads, 1,
            "the key reads the same without the local copy"
        );
    }

    /// A document a start cannot replace costs that domain and no other, and
    /// never keeps the launcher from coming up.
    ///
    /// A write replaces a document with a file it creates beside it, so a
    /// document's own mode does not stop it: the directory is what can refuse
    /// the write, and it then refuses every domain at once. The failure is
    /// therefore checked where it is observable — the call reports no error,
    /// leaves both documents as they stand, and the next start prunes as if
    /// nothing had happened.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_failed_prune_does_not_stop_the_other_domains() {
        use std::os::unix::fs::PermissionsExt;

        let pool = migrated_pool().await;
        let local = tempfile::tempdir().unwrap();
        let deployment = tempfile::tempdir().unwrap();
        let appearance = local.path().join("appearance.json");
        let download = local.path().join("download.json");
        let theme = br#"{"schema_version":1,"data":{"theme":"light"}}"#;
        let writes =
            br#"{"schema_version":1,"data":{"max_concurrent_writes":3}}"#;
        std::fs::write(&appearance, theme).unwrap();
        std::fs::write(&download, writes).unwrap();
        std::fs::write(
            deployment.path().join("appearance.json"),
            br#"{"schema_version":1,"data":{"theme":"light"}}"#,
        )
        .unwrap();
        std::fs::write(
            deployment.path().join("download.json"),
            br#"{"schema_version":1,"data":{"max_concurrent_writes":3}}"#,
        )
        .unwrap();

        let documents = layered(&local, &deployment, &pool);
        std::fs::set_permissions(
            local.path(),
            std::fs::Permissions::from_mode(0o555),
        )
        .unwrap();
        let pruned = prune_redundant(&documents).await;
        // Restored before anything is asserted: a directory left read-only
        // would keep the temporary one from being removed afterwards.
        std::fs::set_permissions(
            local.path(),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();

        pruned.expect(
            "a start is not stopped by a document it could not drop entries from",
        );
        assert_eq!(
            std::fs::read(&appearance).unwrap().as_slice(),
            theme.as_slice(),
            "a document that could not be replaced is left as it stands"
        );
        assert_eq!(
            std::fs::read(&download).unwrap().as_slice(),
            writes.as_slice(),
            "the domains after it are left as they stand too"
        );

        prune_redundant(&documents).await.unwrap();

        let stored: Value =
            serde_json::from_slice(&std::fs::read(&download).unwrap()).unwrap();
        assert!(
            !stored["data"]
                .as_object()
                .unwrap()
                .contains_key("max_concurrent_writes"),
            "the second domain is pruned once a write can land"
        );
        let stored: Value =
            serde_json::from_slice(&std::fs::read(&appearance).unwrap())
                .unwrap();
        assert!(
            !stored["data"].as_object().unwrap().contains_key("theme"),
            "and so is the domain whose write failed before it"
        );
    }

    /// With no deployment there is nothing a local copy could shadow, so a
    /// start rewrites nothing: the value it repeats is the default's own.
    #[tokio::test]
    async fn startup_leaves_the_documents_alone_without_a_deployment() {
        let pool = migrated_pool().await;
        let local = tempfile::tempdir().unwrap();
        let written = local.path().join("appearance.json");
        let contents = br#"{"schema_version":1,"data":{"theme":"dark"}}"#;
        std::fs::write(&written, contents).unwrap();

        let documents = Documents::at(local.path().to_path_buf(), &pool);
        prune_redundant(&documents).await.unwrap();

        assert_eq!(
            std::fs::read(&written).unwrap().as_slice(),
            contents.as_slice(),
            "no deployment means no repeated value to drop"
        );
    }

    /// The `data` a document a test reads back carries.
    fn data(root: &std::path::Path, name: &str) -> Map<String, Value> {
        let contents =
            std::fs::read(root.join(format!("{name}.json"))).unwrap();
        let stored: Value = serde_json::from_slice(&contents).unwrap();
        stored["data"].as_object().unwrap().clone()
    }

    /// Every domain document as it stands: its bytes, and when they were last
    /// written, so a rewrite that happened to produce the same bytes shows too.
    ///
    /// A domain without a document is part of the picture rather than an error:
    /// a store leaves a domain that has nothing to say without one.
    fn snapshot(
        root: &std::path::Path,
    ) -> Vec<(String, Option<Vec<u8>>, Option<std::time::SystemTime>)> {
        DOMAINS
            .iter()
            .map(|(name, _, _)| {
                let path = root.join(format!("{name}.json"));
                let contents = std::fs::read(&path).ok();
                let modified = std::fs::metadata(&path)
                    .ok()
                    .and_then(|metadata| metadata.modified().ok());
                (name.to_string(), contents, modified)
            })
            .collect()
    }

    /// A key holding the value the build defaults to says nothing: the read
    /// already answers it, so the document leaves it out even when another key
    /// beside it is written.
    #[tokio::test]
    async fn a_key_that_matches_the_baseline_leaves_the_document() {
        let pool = migrated_pool().await;
        let local = tempfile::tempdir().unwrap();
        let documents = Documents::at(local.path().to_path_buf(), &pool);
        let mut settings = Settings::default();
        settings.locale = "zh_cn".to_string();
        assert_eq!(
            settings.theme.as_str(),
            Settings::default().theme.as_str(),
            "the key under test holds the value the build defaults to"
        );

        store(&documents, &settings).await.unwrap();

        let data = data(local.path(), "appearance");
        assert!(
            !data.contains_key("theme"),
            "a key equal to the default is left out of the document"
        );
        assert_eq!(
            data.get("locale"),
            Some(&Value::from("zh_cn")),
            "a key the user set beside it is written"
        );
    }

    /// A key the deployment ships is a value the deployment decides, so the
    /// local document must not carry it: leaving it out is what lets a later
    /// change of the deployment's value reach the user.
    #[tokio::test]
    async fn a_key_the_deployment_ships_stays_out_of_the_document() {
        let pool = migrated_pool().await;
        let local = tempfile::tempdir().unwrap();
        let deployment = tempfile::tempdir().unwrap();
        std::fs::write(
            deployment.path().join("appearance.json"),
            r#"{"schema_version":1,"data":{"theme":"light"}}"#,
        )
        .unwrap();
        let mut settings = Settings::default();
        settings.theme = Theme::Light;
        settings.locale = "zh_cn".to_string();

        let documents = layered(&local, &deployment, &pool);
        store(&documents, &settings).await.unwrap();

        let data = data(local.path(), "appearance");
        assert!(
            !data.contains_key("theme"),
            "a key equal to the value the deployment ships stays out"
        );
        assert_eq!(
            data.get("locale"),
            Some(&Value::from("zh_cn")),
            "the deployment ships nothing for the key beside it"
        );
    }

    /// A domain with nothing of its own to say is left without a document, so
    /// the row keeps answering it. Creating an empty one would end that answer
    /// for a domain whose write has not moved to documents yet.
    #[tokio::test]
    async fn a_store_creates_no_document_for_a_domain_with_nothing_to_say() {
        let pool = migrated_pool().await;
        let local = tempfile::tempdir().unwrap();
        let documents = Documents::at(local.path().to_path_buf(), &pool);

        store(&documents, &Settings::default()).await.unwrap();

        let written: Vec<String> = std::fs::read_dir(local.path())
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .collect();
        assert!(
            written.is_empty(),
            "a default settings left documents behind: {written:?}"
        );
    }

    /// A store whose document already holds what it would write touches no
    /// file, so storing the same settings twice leaves the documents as the
    /// first store left them.
    #[tokio::test]
    async fn a_store_with_nothing_to_change_writes_nothing() {
        let pool = migrated_pool().await;
        let local = tempfile::tempdir().unwrap();
        let documents = Documents::at(local.path().to_path_buf(), &pool);
        let mut settings = Settings::default();
        settings.locale = "zh_cn".to_string();
        settings.theme = Theme::Light;
        store(&documents, &settings).await.unwrap();

        // Rewritten the way another build could have left it: the same values
        // in another shape, so a rewrite shows as different bytes although it
        // stores the same settings.
        let appearance = local.path().join("appearance.json");
        let stored: Value =
            serde_json::from_slice(&std::fs::read(&appearance).unwrap())
                .unwrap();
        std::fs::write(&appearance, serde_json::to_vec(&stored).unwrap())
            .unwrap();
        let before = snapshot(local.path());

        store(&documents, &settings).await.unwrap();

        assert_eq!(
            snapshot(local.path()),
            before,
            "a store with nothing to change must not rewrite or touch a document"
        );
        assert_eq!(
            data(local.path(), "appearance").get("theme"),
            Some(&Value::from("light")),
            "the document a store left alone still carries what it was given"
        );
    }

    /// A single-key write reads the document it changes and writes it back
    /// with that one key changed: the rest of the document is not this
    /// writer's to lower.
    #[tokio::test]
    async fn a_single_key_write_keeps_the_rest_of_the_document() {
        let pool = migrated_pool().await;
        let local = tempfile::tempdir().unwrap();
        std::fs::write(
            local.path().join("appearance.json"),
            r#"{"schema_version":1,"data":{"locale":"zh_cn","theme":"oled"}}"#,
        )
        .unwrap();
        let documents = Documents::at(local.path().to_path_buf(), &pool);

        store_key(
            &documents,
            &Handle::new(pool.clone()),
            "theme",
            Value::from("light"),
        )
        .await
        .unwrap();

        let data = data(local.path(), "appearance");
        assert_eq!(
            data.get("theme"),
            Some(&Value::from("light")),
            "the key the write names carries the value it was given"
        );
        assert_eq!(
            data.get("locale"),
            Some(&Value::from("zh_cn")),
            "a key beside it keeps what the document held"
        );
    }

    /// A domain with no document is answered by the row until the first
    /// single-key write gives it one, so that write has to carry the row's
    /// values for the rest of the domain: the document the write creates is the
    /// authority for the whole domain from then on, and a sibling the write did
    /// not name would otherwise drop to the build default.
    #[tokio::test]
    async fn a_single_key_write_keeps_the_row_values_of_its_siblings() {
        let pool = migrated_pool().await;
        let handle = Handle::new(pool.clone());
        let mut row = Settings::read_row(&handle).await.unwrap();
        row.skipped_update = Some("1.20.1".to_string());
        row.auto_download_updates = Some(true);
        row.update(&handle).await.unwrap();

        let settings_dir = tempfile::tempdir().unwrap();
        identify(settings_dir.path());
        let root = settings_dir.path().join(DIR_NAME);
        let documents = Documents::at(root.clone(), &pool);

        store_key(&documents, &handle, "skipped_update", Value::from("9.9.9"))
            .await
            .unwrap();

        let data = data(&root, "state");
        assert_eq!(
            data.get("skipped_update"),
            Some(&Value::from("9.9.9")),
            "the key the write names carries the value it was given"
        );
        assert_eq!(
            data.get("auto_download_updates"),
            Some(&Value::from(true)),
            "a sibling the write did not name keeps the value the row held"
        );

        let settings = Settings::get(&handle).await.unwrap();
        assert_eq!(
            settings.skipped_update.as_deref(),
            Some("9.9.9"),
            "the write is what the key reads"
        );
        assert_eq!(
            settings.auto_download_updates,
            Some(true),
            "and the row value beside it survives the document that now answers the domain"
        );
    }

    /// A document carries no foreign key, so the writer that removes the thing
    /// a reference names clears it here — and only while the document still
    /// holds the value that was seen, or a reference the user has already
    /// pointed elsewhere would be lost.
    #[tokio::test]
    async fn a_reference_to_something_removed_is_cleared() {
        let pool = migrated_pool().await;
        let local = tempfile::tempdir().unwrap();
        std::fs::write(
            local.path().join("appearance.json"),
            r#"{"schema_version":1,"data":{"locale":"zh_cn","minimal_home_instance_id":"gone"}}"#,
        )
        .unwrap();
        let documents = Documents::at(local.path().to_path_buf(), &pool);

        clear_key_if(&documents, "minimal_home_instance_id", "gone")
            .await
            .unwrap();

        let data = data(local.path(), "appearance");
        assert!(
            !data.contains_key("minimal_home_instance_id"),
            "the reference to what was removed is cleared"
        );
        assert_eq!(
            data.get("locale"),
            Some(&Value::from("zh_cn")),
            "the rest of the document is left as it stands"
        );
    }

    #[tokio::test]
    async fn a_reference_to_something_else_stays() {
        let pool = migrated_pool().await;
        let local = tempfile::tempdir().unwrap();
        let written = local.path().join("appearance.json");
        let contents = br#"{"schema_version":1,"data":{"minimal_home_instance_id":"kept"}}"#;
        std::fs::write(&written, contents).unwrap();
        let documents = Documents::at(local.path().to_path_buf(), &pool);

        clear_key_if(&documents, "minimal_home_instance_id", "gone")
            .await
            .unwrap();

        assert_eq!(
            data(local.path(), "appearance").get("minimal_home_instance_id"),
            Some(&Value::from("kept")),
            "a reference that already points elsewhere is the user's choice"
        );
        assert_eq!(
            std::fs::read(&written).unwrap().as_slice(),
            contents.as_slice(),
            "a document with nothing to clear is not written at all"
        );
    }

    /// A document this build cannot read costs its own domain and no other: it
    /// is left byte for byte, and the domains after it are still written.
    ///
    /// The failure is put on the first domain a store reaches, so a store that
    /// stopped at the first one it could not write would leave every document
    /// behind it unwritten.
    #[tokio::test]
    async fn a_failed_domain_does_not_stop_the_others() {
        let pool = migrated_pool().await;
        let local = tempfile::tempdir().unwrap();
        let unreadable = br#"{"schema_version":1,"data":{"theme":}"#;
        std::fs::write(local.path().join("appearance.json"), unreadable)
            .unwrap();
        let documents = Documents::at(local.path().to_path_buf(), &pool);
        let mut settings = Settings::default();
        settings.backup_repository_path = Some("/from-the-row".to_string());

        let stored = store(&documents, &settings).await;

        stored.expect(
            "a document that cannot be read is reported in the log, not to the caller",
        );
        assert_eq!(
            std::fs::read(local.path().join("appearance.json"))
                .unwrap()
                .as_slice(),
            unreadable.as_slice(),
            "a document this build cannot read is left as it stands"
        );
        assert_eq!(
            data(local.path(), "backup").get("backup_repository_path"),
            Some(&Value::from("/from-the-row")),
            "the domains after the one that could not be written are still written"
        );
    }

    /// A directory that refuses every write costs every domain and panics
    /// nowhere: a write replaces a document with a file it creates beside it,
    /// so the directory is what refuses it, and it then refuses every domain at
    /// once. What the caller can observe is checked — the call reports no error
    /// and leaves no document behind, and the same call writes every document
    /// once the directory accepts them.
    #[cfg(unix)]
    #[tokio::test]
    async fn an_unwritable_directory_leaves_every_document_unwritten() {
        use std::os::unix::fs::PermissionsExt;

        let pool = migrated_pool().await;
        let local = tempfile::tempdir().unwrap();
        let documents = Documents::at(local.path().to_path_buf(), &pool);
        let mut settings = Settings::default();
        settings.locale = "zh_cn".to_string();

        std::fs::set_permissions(
            local.path(),
            std::fs::Permissions::from_mode(0o555),
        )
        .unwrap();
        let stored = store(&documents, &settings).await;
        // Restored before anything is asserted: a directory left read-only
        // would keep the temporary one from being removed afterwards.
        std::fs::set_permissions(
            local.path(),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();

        stored.expect(
            "a document that could not be written is reported in the log, not to the caller",
        );
        assert_eq!(
            std::fs::read_dir(local.path()).unwrap().count(),
            0,
            "no document is left behind while the directory refuses one"
        );

        store(&documents, &settings).await.unwrap();

        assert_eq!(
            data(local.path(), "appearance").get("locale"),
            Some(&Value::from("zh_cn")),
            "the same store writes its documents once the directory accepts them"
        );
    }

    /// The document a test leaves behind for a domain, in the directory a start
    /// would have named.
    fn write_document(settings_dir: &std::path::Path, name: &str, data: &str) {
        let root = settings_dir.join(DIR_NAME);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join(format!("{name}.json")),
            format!(r#"{{"schema_version":1,"data":{{{data}}}}}"#),
        )
        .unwrap();
    }

    /// A start names the document directory before it reads anything, and from
    /// then on the documents answer: a key one of them carries comes from it
    /// rather than from the row, a domain they do not cover still comes from the
    /// row, and a document that goes away gives its domain back to it.
    #[tokio::test]
    async fn a_document_answers_before_the_row() {
        let pool = migrated_pool().await;
        let handle = Handle::new(pool.clone());
        let mut row = Settings::read_row(&handle).await.unwrap();
        row.theme = Theme::Light;
        row.backup_repository_path = Some("/from-the-row".to_string());
        row.update(&handle).await.unwrap();

        let settings_dir = tempfile::tempdir().unwrap();
        identify(settings_dir.path());
        write_document(settings_dir.path(), "appearance", r#""theme":"oled""#);

        let answered = Settings::get(&handle).await.unwrap();
        assert_eq!(
            answered.theme.as_str(),
            "oled",
            "the document answers the domain the row would"
        );
        assert_eq!(
            answered.backup_repository_path.as_deref(),
            Some("/from-the-row"),
            "a domain with no document is still answered by the row"
        );

        std::fs::remove_file(
            settings_dir.path().join(DIR_NAME).join("appearance.json"),
        )
        .unwrap();
        // Removing a file is something no writer of the documents here does, so
        // nothing has said that the settings changed: a start would compose them
        // afresh, and the removal is told to the cache the same way.
        invalidate();

        let fell_back = Settings::get(&handle).await.unwrap();
        assert_eq!(
            fell_back.theme.as_str(),
            "light",
            "a domain whose document is gone is answered by the row again"
        );
    }

    /// A handle that no start has named a document directory for — which is
    /// every unit test that builds its own — reads the row, whatever is on disk
    /// where a document would be.
    #[tokio::test]
    async fn without_a_root_the_row_still_answers() {
        let pool = migrated_pool().await;
        let handle = Handle::new(pool.clone());
        let mut row = Settings::read_row(&handle).await.unwrap();
        row.theme = Theme::Light;
        row.update(&handle).await.unwrap();

        let settings_dir = tempfile::tempdir().unwrap();
        write_document(settings_dir.path(), "appearance", r#""theme":"oled""#);

        assert_eq!(
            root(),
            None,
            "a root is named by a start, and this test names none"
        );
        let settings = Settings::get(&handle).await.unwrap();
        assert_eq!(
            settings.theme.as_str(),
            "light",
            "with no root the row answers, as it always did"
        );
    }

    /// A read is composed once and kept: a document changed behind the cache's
    /// back — by a hand edit, or another build — is not seen until a writer says
    /// the settings changed.
    #[tokio::test]
    async fn a_cached_read_does_not_re_read_the_documents() {
        let pool = migrated_pool().await;
        let handle = Handle::new(pool.clone());
        let settings_dir = tempfile::tempdir().unwrap();
        identify(settings_dir.path());
        write_document(settings_dir.path(), "appearance", r#""theme":"oled""#);

        assert_eq!(
            Settings::get(&handle).await.unwrap().theme.as_str(),
            "oled",
            "the first read composes the documents"
        );

        write_document(settings_dir.path(), "appearance", r#""theme":"light""#);
        assert_eq!(
            Settings::get(&handle).await.unwrap().theme.as_str(),
            "oled",
            "a cached read does not go back to the documents"
        );

        invalidate();
        assert_eq!(
            Settings::get(&handle).await.unwrap().theme.as_str(),
            "light",
            "a read after an invalidation composes them again"
        );
    }

    /// A document write is one of the two things a read composes from, so the
    /// read after it composes again without anything else saying so.
    #[tokio::test]
    async fn writing_through_store_refreshes_the_next_read() {
        let pool = migrated_pool().await;
        let handle = Handle::new(pool.clone());
        let settings_dir = tempfile::tempdir().unwrap();
        identify(settings_dir.path());
        let documents =
            Documents::at(settings_dir.path().join(DIR_NAME), &pool);

        assert_eq!(
            Settings::get(&handle).await.unwrap().locale,
            Settings::default().locale,
            "the row answers before a document exists"
        );

        let mut settings = Settings::default();
        settings.locale = "zh_cn".to_string();
        store(&documents, &settings).await.unwrap();

        assert_eq!(
            Settings::get(&handle).await.unwrap().locale,
            "zh_cn",
            "a store says the documents changed, so the next read composes again"
        );
    }

    /// The row is the other thing a read composes from, and the writers of it are
    /// what say so: `personalized_ads` is in no document and is read from the row
    /// on every read, so it shows a stale row at once.
    #[tokio::test]
    async fn writing_the_row_refreshes_the_next_read() {
        let pool = migrated_pool().await;
        let handle = Handle::new(pool.clone());
        let settings_dir = tempfile::tempdir().unwrap();
        identify(settings_dir.path());

        let mut settings = Settings::get(&handle).await.unwrap();
        assert!(
            !settings.personalized_ads,
            "the preference is off in a fresh row"
        );

        settings.personalized_ads = true;
        settings.update(&handle).await.unwrap();

        assert!(
            Settings::get(&handle).await.unwrap().personalized_ads,
            "the row changed, so the next read composes again"
        );
    }

    /// A unit's write is not the row until the unit commits, so a read between
    /// the two composes the row before it. Said at the write rather than at the
    /// commit, the invalidation would have that read cached under the version
    /// the write moved, and nothing would move it again once the commit landed:
    /// every later read would be answered with the row from before the commit
    /// for the rest of the launch.
    #[tokio::test]
    async fn a_read_between_invalidate_and_commit_is_not_cached() {
        let directory = tempfile::tempdir().unwrap();
        // Two connections: the read below runs while the unit holds the other,
        // which an in-memory database could not give it.
        let pool = crate::state::store::tests::support::migrated_file_pool(
            directory.path(),
        )
        .await;
        let handle = Handle::new(pool.clone());
        let settings_dir = tempfile::tempdir().unwrap();
        identify(settings_dir.path());

        let mut unit = crate::state::store::atom(&handle).await.unwrap();
        StoredSettings::update_in(&mut unit, |record| {
            record.personalized_ads = true;
        })
        .await
        .unwrap();

        assert!(
            !Settings::get(&handle).await.unwrap().personalized_ads,
            "the unit has not committed, so a read between the two composes the row before it"
        );

        unit.commit().await.unwrap();

        assert!(
            Settings::get(&handle).await.unwrap().personalized_ads,
            "the read after the commit composes the row the unit wrote"
        );
    }

    /// The `network` document as it stands: its bytes, and when they were last
    /// written, so a rewrite that happened to produce the same bytes shows too.
    fn network_document(
        root: &std::path::Path,
    ) -> (Option<Vec<u8>>, Option<std::time::SystemTime>) {
        let path = root.join("network.json");
        (
            std::fs::read(&path).ok(),
            std::fs::metadata(&path)
                .ok()
                .and_then(|metadata| metadata.modified().ok()),
        )
    }

    /// Rewrites a document the way another build could have left it: the same
    /// values in another shape, so a rewrite of it shows as different bytes
    /// although it stores the same settings.
    fn reshape(path: &std::path::Path) {
        let stored: Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        std::fs::write(path, serde_json::to_vec(&stored).unwrap()).unwrap();
    }

    /// The write a setting goes through is the patch, and the domain it names is
    /// answered by its document from the patch that changes it.
    #[tokio::test]
    async fn a_network_patch_writes_the_document() {
        let pool = migrated_pool().await;
        let handle = Handle::new(pool.clone());
        let settings_dir = tempfile::tempdir().unwrap();
        identify(settings_dir.path());
        let root = settings_dir.path().join(DIR_NAME);

        patch_with(&handle, serde_json::json!({ "ignore_ssl_errors": true }))
            .await
            .unwrap();

        assert_eq!(
            data(&root, "network").get("ignore_ssl_errors"),
            Some(&Value::from(true)),
            "the key the patch named is in the document of its domain"
        );
        assert!(
            Settings::get(&handle).await.unwrap().ignore_ssl_errors,
            "and the read answers it from there"
        );
    }

    /// A patch states the domain it names and no other: a domain with nothing
    /// to say keeps being answered by the row rather than being given an empty
    /// document, and one that already holds what the patch computes is left
    /// byte for byte.
    #[tokio::test]
    async fn a_patch_outside_the_network_leaves_its_document_alone() {
        let pool = migrated_pool().await;
        let handle = Handle::new(pool.clone());
        let settings_dir = tempfile::tempdir().unwrap();
        identify(settings_dir.path());
        let root = settings_dir.path().join(DIR_NAME);

        patch_with(&handle, serde_json::json!({ "theme": "light" }))
            .await
            .unwrap();

        assert!(
            !root.join("network.json").exists(),
            "a domain with nothing to record is left without a document"
        );

        patch_with(&handle, serde_json::json!({ "ignore_ssl_errors": true }))
            .await
            .unwrap();
        reshape(&root.join("network.json"));
        let before = network_document(&root);

        patch_with(&handle, serde_json::json!({ "theme": "oled" }))
            .await
            .unwrap();

        assert_eq!(
            network_document(&root),
            before,
            "a patch outside the domain must not rewrite or touch its document"
        );
    }

    /// A deployment pins a key, and the value the row froze before it must not
    /// be restated into the local document: the local copy is the last word for
    /// every key it carries, so the deployment's own later change of that key
    /// would never arrive again.
    ///
    /// The patch names a key of the same domain, so the domain is stated at all:
    /// what it must not carry is the pinned key beside it.
    #[tokio::test]
    async fn a_deployment_key_is_not_restated_into_the_local_document() {
        let pool = migrated_pool().await;
        let handle = Handle::new(pool.clone());
        let settings_dir = tempfile::tempdir().unwrap();
        identify(settings_dir.path());
        let root = settings_dir.path().join(DIR_NAME);
        let deployment = tempfile::tempdir().unwrap();
        std::fs::write(
            deployment.path().join("network.json"),
            r#"{"schema_version":1,"data":{"ignore_ssl_errors":false}}"#,
        )
        .unwrap();
        deploy(&deployment);
        // The row says the opposite, as a value frozen before the deployment
        // arrived would.
        let mut unit = crate::state::store::atom(&handle).await.unwrap();
        StoredSettings::update_in(&mut unit, |record| {
            record.ignore_ssl_errors = true;
        })
        .await
        .unwrap();
        unit.commit().await.unwrap();
        assert!(
            Settings::read_row(&handle).await.unwrap().ignore_ssl_errors,
            "the row holds the opposite of the value the deployment ships"
        );

        // A sibling of the pinned key, and one whose stored value differs from
        // the build default: a patch that changed nothing would leave the
        // domain with nothing to state.
        patch_with(
            &handle,
            serde_json::json!({ "allow_privileged_scheme": true }),
        )
        .await
        .unwrap();

        let stored = data(&root, "network");
        assert!(
            !stored.contains_key("ignore_ssl_errors"),
            "a key the deployment answers is not copied into the local document: {stored:?}"
        );
        assert_eq!(
            stored.get("allow_privileged_scheme"),
            Some(&Value::from(true)),
            "the key the patch named is stated, so the domain was written"
        );
        assert!(
            !Settings::get(&handle).await.unwrap().ignore_ssl_errors,
            "the deployment's value is the one the key reads"
        );
    }

    /// The key the user changes themselves is the one case the local document
    /// takes a deployment's key over: the file is the last word from then on,
    /// which is what makes an explicit write the only way to override it.
    #[tokio::test]
    async fn an_explicit_change_of_a_pinned_key_reaches_the_local_document() {
        let pool = migrated_pool().await;
        let handle = Handle::new(pool.clone());
        let settings_dir = tempfile::tempdir().unwrap();
        identify(settings_dir.path());
        let root = settings_dir.path().join(DIR_NAME);
        let deployment = tempfile::tempdir().unwrap();
        std::fs::write(
            deployment.path().join("network.json"),
            r#"{"schema_version":1,"data":{"ignore_ssl_errors":false}}"#,
        )
        .unwrap();
        deploy(&deployment);
        assert!(
            !Settings::get(&handle).await.unwrap().ignore_ssl_errors,
            "the key reads the deployment's value before the patch"
        );

        patch_with(&handle, serde_json::json!({ "ignore_ssl_errors": true }))
            .await
            .unwrap();

        assert_eq!(
            data(&root, "network").get("ignore_ssl_errors"),
            Some(&Value::from(true)),
            "a key the patch names is stated although the deployment answers it"
        );
        assert!(
            Settings::get(&handle).await.unwrap().ignore_ssl_errors,
            "and the user's own document is the last word for it"
        );
    }

    /// The domain is switched to its document rather than moved: the row keeps
    /// the value, so a document that goes away gives the domain back to the row
    /// and the setting still reads as the user chose.
    #[tokio::test]
    async fn the_row_keeps_the_value_as_a_fallback() {
        let pool = migrated_pool().await;
        let handle = Handle::new(pool.clone());
        let settings_dir = tempfile::tempdir().unwrap();
        identify(settings_dir.path());
        let root = settings_dir.path().join(DIR_NAME);

        patch_with(&handle, serde_json::json!({ "ignore_ssl_errors": true }))
            .await
            .unwrap();

        assert!(
            Settings::read_row(&handle).await.unwrap().ignore_ssl_errors,
            "the row a domain without a document is answered by carries the value"
        );

        std::fs::remove_file(root.join("network.json")).unwrap();
        // Removing a file is something no writer of the documents here does, so
        // nothing has said that the settings changed: a start would compose them
        // afresh, and the removal is told to the cache the same way.
        invalidate();

        assert!(
            Settings::get(&handle).await.unwrap().ignore_ssl_errors,
            "a domain whose document is gone is answered by the row again, \
             with the value the user set rather than the default"
        );
    }

    /// A patch that changes nothing changes no document: the second of two
    /// identical patches computes the same domain and leaves the file alone.
    #[tokio::test]
    async fn a_second_patch_without_changes_does_not_rewrite() {
        let pool = migrated_pool().await;
        let handle = Handle::new(pool.clone());
        let settings_dir = tempfile::tempdir().unwrap();
        identify(settings_dir.path());
        let root = settings_dir.path().join(DIR_NAME);
        let patch = serde_json::json!({ "ignore_ssl_errors": true });

        patch_with(&handle, patch.clone()).await.unwrap();
        reshape(&root.join("network.json"));
        let before = network_document(&root);

        patch_with(&handle, patch).await.unwrap();

        assert_eq!(
            network_document(&root),
            before,
            "a patch with nothing to change must not rewrite or touch the document"
        );
    }

    /// `update` is the whole-row write a start reaches through the directory
    /// move and the settings migrations, so the domain that reads from its
    /// document is written from what it stores: a value changed there is in the
    /// document too, and not only in a row that domain no longer reads.
    #[tokio::test]
    async fn an_update_writes_the_document_too() {
        let pool = migrated_pool().await;
        let handle = Handle::new(pool.clone());
        let settings_dir = tempfile::tempdir().unwrap();
        identify(settings_dir.path());
        let root = settings_dir.path().join(DIR_NAME);

        let mut settings = Settings::read_row(&handle).await.unwrap();
        settings.ignore_ssl_errors = true;
        settings.update(&handle).await.unwrap();

        assert_eq!(
            data(&root, "network").get("ignore_ssl_errors"),
            Some(&Value::from(true)),
            "a whole-row write reaches the document of the domain it carries"
        );
        assert!(
            Settings::get(&handle).await.unwrap().ignore_ssl_errors,
            "and the read answers the value that write carried"
        );
    }

    /// The store states the row as it stands when it writes rather than what a
    /// writer read before it: two writers reach the store in either order, and
    /// the older of the two would otherwise be the value the file is left with.
    #[tokio::test]
    async fn a_domain_write_states_the_row_as_it_stands_then() {
        let pool = migrated_pool().await;
        let handle = Handle::new(pool.clone());
        let settings_dir = tempfile::tempdir().unwrap();
        identify(settings_dir.path());
        let root = settings_dir.path().join(DIR_NAME);

        let before_the_other_writer =
            Settings::read_row(&handle).await.unwrap();
        assert!(
            !before_the_other_writer.ignore_ssl_errors,
            "the settings this test holds are the ones before the writer below"
        );
        patch_with(&handle, serde_json::json!({ "ignore_ssl_errors": true }))
            .await
            .unwrap();

        let documents = Documents::at(root.clone(), &pool);
        store_domain(&documents, "network", &handle, None)
            .await
            .unwrap();

        assert_eq!(
            data(&root, "network").get("ignore_ssl_errors"),
            Some(&Value::from(true)),
            "the store states the row, not the snapshot its caller took before it"
        );
        assert!(
            Settings::get(&handle).await.unwrap().ignore_ssl_errors,
            "and the domain still reads the value the newer writer stored"
        );
    }

    /// A write that cannot land takes the domain's document with it: the row is
    /// already committed, so the file would go on answering with the value it
    /// froze rather than with the change that landed. Removing it gives the
    /// domain back to the row, and the failure stays in the log.
    ///
    /// The write is handed in so the failure can be one: a directory refuses a
    /// document and its removal alike, so no file system of this test would be
    /// the state a committed row beside a document that could not be replaced
    /// describes.
    #[tokio::test]
    async fn a_failed_domain_write_leaves_the_domain_to_the_row() {
        let pool = migrated_pool().await;
        let handle = Handle::new(pool.clone());
        let settings_dir = tempfile::tempdir().unwrap();
        identify(settings_dir.path());
        let root = settings_dir.path().join(DIR_NAME);

        let mut unit = crate::state::store::atom(&handle).await.unwrap();
        let mut row = Settings::load_in(&mut unit).await.unwrap();
        row.ignore_ssl_errors = true;
        row.save_in(&mut unit).await.unwrap();
        unit.commit().await.unwrap();
        write_document(
            settings_dir.path(),
            "network",
            r#""ignore_ssl_errors":false"#,
        );
        let documents = Documents::at(root.clone(), &pool);
        assert_eq!(
            data(&root, "network").get("ignore_ssl_errors"),
            Some(&Value::from(false)),
            "the document the failed write leaves behind states the old value"
        );

        write_or_discard(&documents, "network", async {
            Err(crate::ErrorKind::OtherError(
                "a document that cannot be written".to_string(),
            )
            .into())
        })
        .await
        .expect("a failed write is reported in the log, not to its caller");

        assert!(
            !root.join("network.json").exists(),
            "the document is removed rather than left answering for the domain"
        );
        assert!(
            Settings::get(&handle).await.unwrap().ignore_ssl_errors,
            "the domain falls back to the row, which holds the change that landed"
        );
    }
}
