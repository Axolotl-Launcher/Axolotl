// Golden-set regression for the managed-resource generator (spec §6.3.2
// criterion 3): every historical item must appear in the generated set G, so a
// generator that silently captures less fails instead of passing quietly.
//
// Usage: node scripts/axolotl/sched-resource-golden-check.mjs [--inventory <sched-resource-inventory.json>]
// Without --inventory the inventory is regenerated in-process.

import { readFileSync } from 'node:fs'
import path from 'node:path'
import { buildInventory } from './sched-resource-inventory.mjs'
import { isMain, loadPolicy, parseArgs, readJson, repoRoot, sha256 } from './sched-utils.mjs'

export const GOLDEN_FILE = 'scripts/axolotl/sched-resource-golden.json'
export const GOLDEN_SHA256 = 'scripts/axolotl/sched-resource-golden.sha256'

/** Whether the frozen file's bytes are the ones the recorded hash covers. */
export function frozenMismatch(goldenText, shaText) {
    const expected = shaText.trim().split(/\s+/)[0]
    return expected !== sha256(goldenText)
}

// FROZEN GOLDEN SET — items may be added; removing one needs a reviewed change
// to this file (its sha256 is recorded with the phase-0 evidence). Membership
// is checked by stable `module_path::IDENT`, never by item counts.
//
// G-1 and G-2 are reconstructed: the v6 §4.2 table (40 items) and the v7 §4.3
// table (54 items) are not part of the spec package, so these lists hold every
// item that t1 §0/§1.12/§3.7 and spec §6.3.1 (Tier A/B/C) name explicitly.
// G-3 holds the apps/app/src items named in design v8 §4.6; the five apps/app
// statics of v7 are not named anywhere in the package and are not listed.
export const GOLDEN = {
    'G-1': [
        'theseus::api::instance::backup::ACTIVE_INSTANCE_OPERATIONS',
        'theseus::api::instance::backup::BACKUP_CANCELLATIONS',
        'theseus::api::instance::backup::INSTANCE_MAINTENANCE_LOCKS',
        'theseus::api::instance::backup::REPOSITORY_LOCK',
        'theseus::api::servers::lifecycle::SERVER_PROCESSES',
        'theseus::api::servers::lifecycle::SERVER_STARTING',
        'theseus::state::InstanceLockManager.held_by_owner',
        'theseus::state::InstanceLockManager.locks',
        'theseus::state::MaintenanceGate.paused',
        'theseus::state::State.api_semaphore',
        'theseus::state::State.background_tasks',
        'theseus::state::State.download_semaphore',
        'theseus::state::State.fetch_semaphore',
        'theseus::state::State.install_admission_locks',
        'theseus::state::State.install_db_semaphore',
        'theseus::state::State.install_execution_locks',
        'theseus::state::State.install_job_cancellations',
        'theseus::state::State.install_job_operation_locks',
        'theseus::state::State.install_job_semaphore',
        'theseus::state::State.io_semaphore',
        'theseus::state::State.screenshot_locks',
        'theseus::state::State.synced_options_lock',
        'theseus::state::instances::config_sync::DIRTY_INSTANCES',
        'theseus::state::process::LOG_BUFFERS',
        'theseus::state::process::ProcessManager.processes',
        'theseus::util::single_flight::FLIGHTS',
    ],
    'G-2': [
        'theseus::api::server_address::SIMULTANEOUS_DNS_QUERIES',
        'theseus::state::State.configured_http_client',
        'theseus::state::State.configured_http_client_update',
        'theseus::state::State.curseforge_api_key',
        'theseus::state::State.curseforge_api_key_update',
        'theseus::util::download::endpoint_selection::coordinator::PROBE_ROUNDS',
        'theseus::util::download::h2_stream_budget::AUTHORITY_BUDGETS',
        'theseus::util::download::local_resources::FALLBACK_WRITES',
        'theseus::util::download::local_resources::WRITE_SAMPLES',
        'theseus::util::download::native_budget::AUTHORITY_BUDGETS',
        'theseus::util::download::native_budget::PHYSICAL_CONNECTIONS',
        'theseus::util::download::native_budget::REGULAR_BUDGETS',
        'theseus::util::fetch::throttle::FILE_VALIDATION_SEMAPHORE',
        'theseus::util::fetch::throttle::ROUTE_PROBE_SEMAPHORE',
        'theseus::util::fetch::throttle::TAIL_HEDGE_SEMAPHORE',
    ],
    'G-3': [
        'theseus_gui::api::files::LOCAL_ICON_CACHE',
        'theseus_gui::api::files::SCREENSHOT_THUMBNAILS',
        'theseus_gui::api::files::StudioWatchers.watchers',
        'theseus_gui::api::mod_translation::ACTIVE_LOCKS',
        'theseus_gui::api::mod_translation::ANALYSES',
        'theseus_gui::api::mod_translation::TASKS',
        'theseus_gui::api::instance::UPGRADE_PLANNING',
        'theseus_gui::api::instance::EARLY_UPGRADE_PLANNING_CANCELS',
        'theseus_gui::api::mod_translation::ANALYSIS_REQUESTS',
        'theseus_gui::api::seed_map::ORE_SCANS',
        'theseus_gui::api::mod_translation::TaskHandle.event_gate',
        'theseus_gui::api::mod_translation::TaskHandle.snapshot',
        'theseus_gui::api::resources::EARLY_CANCELLATIONS',
        'theseus_gui::api::resources::REQUESTS',
        'theseus_gui::api::schematic_preview::REQUEST_CANCELLATIONS',
        'theseus_gui::api::schematic_preview::SESSIONS',
        'theseus_gui::api::search_cancellation::SEARCH_CANCELLATIONS',
        'theseus_gui::macos::deep_link::InitialPayload.payload',
    ],
    // Cross-line declarations (design v8 §4.3); fixtures F1–F5 and F7 parse them.
    'G-4': [
        'theseus::util::download::h2_stream_budget::AUTHORITY_BUDGETS',
        'theseus::util::download::native_budget::AUTHORITY_BUDGETS',
        'theseus_gui::api::files::LOCAL_ICON_CACHE',
        'theseus_gui::api::files::SCREENSHOT_THUMBNAILS',
        'theseus_gui::api::schematic_preview::REQUEST_CANCELLATIONS',
        'theseus_gui::api::search_cancellation::SEARCH_CANCELLATIONS',
    ],
}

export function missingGolden(inventory) {
    const generated = new Set(inventory.items.map((item) => item.ident))
    return Object.entries(GOLDEN).flatMap(([set, idents]) => idents.filter((ident) => !generated.has(ident)).map((ident) => ({ set, ident })))
}

if (isMain(import.meta.url)) {
    const args = parseArgs(process.argv.slice(2))
    // K0.7: the golden set is frozen by a recorded hash. Editing it requires
    // updating both the file and the recorded hash in the same reviewed change.
    const goldenText = readFileSync(path.resolve(repoRoot, GOLDEN_FILE), 'utf8')
    if (goldenText !== `${JSON.stringify(GOLDEN, null, 2)}\n`) {
        console.error(`the inline golden set and ${GOLDEN_FILE} disagree; edit both in the same reviewed change`)
        process.exit(1)
    }
    const shaText = readFileSync(path.resolve(repoRoot, GOLDEN_SHA256), 'utf8')
    if (frozenMismatch(goldenText, shaText)) {
        console.error(`${GOLDEN_FILE} does not match ${GOLDEN_SHA256}; a frozen change needs the hash updated in review`)
        process.exit(1)
    }
    const inventory = args.inventory ? readJson(args.inventory) : buildInventory(loadPolicy()).inventory
    const missing = missingGolden(inventory)
    const apps = inventory.items.filter((item) => item.crate === 'apps/app' && item.kind === 'static' && item.status === 'registered')
    for (const [set, idents] of Object.entries(GOLDEN)) console.log(`${set}: ${idents.length - missing.filter((entry) => entry.set === set).length}/${idents.length} present`)
    console.log(`apps/app/src registered lock statics in G: ${apps.length} (design lower bound >= 15)`)
    if (apps.length < 15) {
        console.error(`apps/app/src registered lock statics fell to ${apps.length}; the design lower bound is 15`)
        process.exit(1)
    }
    if (missing.length > 0) {
        for (const { set, ident } of missing) console.error(`missing ${set}: ${ident}`)
        process.exit(1)
    }
}
