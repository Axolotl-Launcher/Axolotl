// Negative cases of the scheduler gates: each gate must fail on the input it
// exists to reject.

import assert from 'node:assert/strict'
import { test } from 'node:test'
import { checkSpawns, countForms, unboundedInL0 } from './sched-bare-spawn-check.mjs'
import { l0Violations } from './sched-l0-resource-check.mjs'
import { checkLeaves } from './sched-leaf-primitive-check.mjs'
import { checkGovernance } from './sched-level-predicate-check.mjs'
import { evaluateOrder } from './sched-order-annotation-check.mjs'
import { evaluateGap } from './sched-recall-gap-gate.mjs'
import { checkConsistency } from './sched-resource-consistency-check.mjs'
import { missingGolden } from './sched-resource-golden-check.mjs'
import { buildInventory } from './sched-resource-inventory.mjs'
import { loadPolicy, maskRust, rankFingerprint, scanMethodHits, sourcesOf, stripTestItems, workspaceCrates } from './sched-utils.mjs'

const policy = loadPolicy()
const crates = workspaceCrates()
const sources = sourcesOf(crates)
const files = buildInventory(policy)
const clone = (value) => structuredClone(value)

function source(rel, text) {
    const code = stripTestItems(maskRust(text))
    const starts = [0]
    for (let i = 0; i < code.length; i++) if (code[i] === '\n') starts.push(i + 1)
    return { crate: { dir: 'packages/app-lib' }, rel, raw: text, code, starts, lines: code.split('\n'), rawLines: text.split('\n') }
}

test('generator rejects an exclusion without a D1–D4 rule (K0.4)', () => {
    const broken = clone(policy)
    broken.exclusions.push({ ident: 'theseus::state::State.io_semaphore', reason: 'no rule cited' })
    assert.throws(() => buildInventory(broken), /must cite one of D1\/D2\/D3\/D4/)
})

test('golden check fails when the generator drops a golden item (K0.5)', () => {
    const inventory = clone(files.inventory)
    inventory.items = inventory.items.filter((item) => item.ident !== 'theseus_gui::api::files::LOCAL_ICON_CACHE')
    assert.deepEqual(
        missingGolden(inventory).map((entry) => entry.set),
        ['G-3', 'G-4'],
    )
    assert.deepEqual(missingGolden(files.inventory), [])
})

test('consistency passes on the generated files', () => {
    assert.deepEqual(checkConsistency(files, policy, crates, []), [])
})

test('criterion 1 fails on a registry entry absent from G (K0.8)', () => {
    const broken = clone(files)
    broken.registry.items.push({ ident: 'theseus::made::UP', rank: 0, file: 'x.rs', line: 1 })
    assert.ok(checkConsistency(broken, policy, crates, []).some((failure) => failure.startsWith('criterion 1')))
})

test('criterion 2 fails when an item is neither registered nor excluded (K0.8)', () => {
    const broken = clone(files)
    broken.registry.items.pop()
    assert.ok(checkConsistency(broken, policy, crates, []).some((failure) => failure.startsWith('criterion 2')))
})

test('criterion 5 fails when a workspace member is missing from the scope (K0.9)', () => {
    const broken = clone(files)
    broken.inventory.scope = broken.inventory.scope.filter((crate) => crate.dir !== 'packages/async-minecraft-ping')
    assert.ok(checkConsistency(broken, policy, crates, []).some((failure) => failure.includes('packages/async-minecraft-ping')))
})

test('criterion 6 fails on an annotation naming an unregistered ident (K0.10)', () => {
    const annotations = [{ file: 'x.rs', line: 3, ident: 'theseus::nowhere::LOCK', fn: 'f@1' }]
    assert.ok(checkConsistency(files, policy, crates, annotations).some((failure) => failure.startsWith('criterion 6')))
})

test('criterion 6 fails when ranks move without a reviewed fingerprint (K0.10)', () => {
    const broken = clone(files)
    ;[broken.registry.items[0].rank, broken.registry.items[1].rank] = [broken.registry.items[1].rank, broken.registry.items[0].rank]
    broken.registry.fingerprint = rankFingerprint(broken.registry.items)
    assert.ok(checkConsistency(broken, policy, crates, []).some((failure) => failure.includes('reviewed')))
})

test('recall-gap gate triggers on an oversized gap and on S5/S9 items (K0.27)', () => {
    const quiet = { items: [{ key: 'a::b', reason: 'other', classHint: null, sites: [], unconstrained: true }] }
    assert.equal(evaluateGap(files.inventory, quiet).fallback, false)
    const oversized = { items: Array.from({ length: Math.floor(files.inventory.items.length / 10) + 1 }, (_, i) => ({ ...quiet.items[0], key: `a::${i}` })) }
    assert.equal(evaluateGap(files.inventory, oversized).oversized, true)
    const permit = { items: [{ ...quiet.items[0], classHint: 'S5/S9' }] }
    assert.equal(evaluateGap(files.inventory, permit).fallback, true)
})

test('order check reports a descending pair inside one function (K0.25)', () => {
    const [low, high] = files.registry.items
    const annotations = [
        { file: 'x.rs', line: 2, ident: high.ident, fn: 'f@1' },
        { file: 'x.rs', line: 3, ident: low.ident, fn: 'f@1' },
    ]
    const result = evaluateOrder(files.registry, policy, annotations)
    assert.equal(result.errors.length, 0)
    assert.equal(result.descending.length, 1)
})

test('L0 check rejects a total-order acquisition inside the L0 module (K0.14)', () => {
    const l0 = source('packages/app-lib/src/scheduler/l0/probe.rs', 'async fn probe(state: &State) {\n    let _permit = state.install_db_semaphore.acquire().await;\n}\n')
    assert.equal(l0Violations([l0], files.registry, policy).length, 1)
    const leaf = source('packages/app-lib/src/scheduler/l0/leaf.rs', 'fn leaf(manager: &ProcessManager) {\n    let _ = manager.processes.read();\n}\n')
    assert.equal(l0Violations([leaf], files.registry, policy).length, 0)
})

test('leaf check rejects an unregistered lock call in an L0 member (K0.17)', () => {
    const broken = clone(policy)
    broken.l0PendingAccess = broken.l0PendingAccess.filter((entry) => entry.receiver !== 'pty_master')
    assert.ok(checkLeaves(broken, files.inventory, sources).failures.some((failure) => failure.includes('pty_master')))
    assert.deepEqual(checkLeaves(policy, files.inventory, sources).failures, [])
})

test('leaf check requires a worst case for unbounded leaves (K0.18)', () => {
    const broken = clone(policy)
    delete broken.leafPrimitives.find((leaf) => leaf.name === 'LOG_BUFFERS').worstCaseMs
    assert.ok(checkLeaves(broken, files.inventory, sources).failures.some((failure) => failure.includes('LOG_BUFFERS')))
})

test('level check rejects a predicate lock below L3 and a missing item (K0.15)', () => {
    const broken = clone(policy)
    broken.governance.find((entry) => entry.id === 'R23').level = 'L2'
    broken.governance = broken.governance.filter((entry) => entry.id !== 'R28')
    const failures = checkGovernance(broken, files.inventory)
    assert.ok(failures.some((failure) => failure.startsWith('R23')))
    assert.ok(failures.some((failure) => failure.startsWith('R28')))
    assert.deepEqual(checkGovernance(policy, files.inventory), [])
})

test('spawn check rejects an unregistered spawn (K0.20)', () => {
    const counts = countForms([source('packages/app-lib/src/new.rs', 'fn go() {\n    tokio::spawn(async {});\n}\n')])
    assert.ok(checkSpawns(counts, { entries: [] }).failures.some((failure) => failure.includes('unregistered tokio::spawn')))
    const entry = { file: 'packages/app-lib/src/new.rs', form: 'tokio::spawn', count: 1, reason: 'test' }
    assert.ok(checkSpawns(counts, { entries: [entry] }).failures.some((failure) => failure.includes('records no sites')))
    assert.deepEqual(checkSpawns(counts, { entries: [{ ...entry, sites: ['go: tokio::spawn(async {});'] }] }).failures, [])
})

test('spawn check rejects a replaced site even when the count holds (K0.20)', () => {
    const allowed = { file: 'packages/app-lib/src/new.rs', form: 'tokio::spawn', count: 1, reason: 'test', sites: ['go: tokio::spawn(async {});'] }
    const counts = countForms([source('packages/app-lib/src/new.rs', 'fn go() {}\nfn other() {\n    tokio::spawn(async { work().await });\n}\n')])
    assert.ok(checkSpawns(counts, { entries: [allowed] }).failures.some((failure) => failure.includes('site not on the allowlist: other: tokio::spawn')))
})

test('spawn check counts scoped thread spawns (K0.20)', () => {
    const counts = countForms([source('packages/app-lib/src/scoped.rs', 'fn go() {\n    std::thread::scope(|scope| {\n        scope.spawn(|| work());\n    });\n}\n')])
    assert.deepEqual(counts.get('packages/app-lib/src/scoped.rs\0thread::scope spawn')?.lines, [3])
    assert.ok(checkSpawns(counts, { entries: [] }).failures.some((failure) => failure.includes('unregistered thread::scope spawn')))
})

test('spawn check counts calls through imported std::fs names (K0.20)', () => {
    const text = 'use std::fs::OpenOptions;\nuse std::{fs, io};\nfn go() {\n    OpenOptions::new().open("a");\n    fs::write("b", "")?;\n    tokio::fs::write("c", "");\n}\n'
    const counts = countForms([source('packages/app-lib/src/files.rs', text)])
    assert.deepEqual(counts.get('packages/app-lib/src/files.rs\0std::fs import')?.lines, [4, 5])
    const allowed = { file: 'packages/app-lib/src/files.rs', form: 'std::fs import', count: 1, reason: 'test', sites: ['go: OpenOptions::new().open("a");'] }
    assert.ok(checkSpawns(counts, { entries: [allowed] }).failures.some((failure) => failure.includes('site not on the allowlist: go: fs::write')))
})

test('spawn check rejects an unbounded channel inside the L0 module (K0.20)', () => {
    const text = 'fn queue() {\n    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<u8>();\n}\n'
    const inside = unboundedInL0([source(`${policy.l0Module}/queue.rs`, text)], policy.l0Module)
    assert.deepEqual(inside.map((hit) => [hit.line, hit.form]), [[2, 'unbounded_channel']])
    assert.deepEqual(unboundedInL0([source('packages/app-lib/src/queue.rs', text)], policy.l0Module), [])
})

test('acquisition scan sees owned lock guards', () => {
    const text = 'async fn go(state: &State) {\n    let _guard = state.synced_options_lock.clone().lock_owned().await;\n    let _read = CACHE.clone().read_owned().await;\n}\n'
    const hits = scanMethodHits([source('packages/app-lib/src/owned.rs', text)])
    assert.deepEqual(
        hits.map((hit) => [hit.line, hit.method, hit.receiver, hit.via]),
        [
            [2, 'lock_owned', 'synced_options_lock', 'field'],
            [3, 'read_owned', 'CACHE', 'static'],
        ],
    )
})
