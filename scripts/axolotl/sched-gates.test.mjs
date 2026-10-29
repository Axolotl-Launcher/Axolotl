// Negative cases of the scheduler gates: each gate must fail on the input it
// exists to reject.

import assert from 'node:assert/strict'
import { test } from 'node:test'
import { checkSpawns, countForms } from './sched-bare-spawn-check.mjs'
import { enterBanViolations } from './sched-enter-ban-check.mjs'
import { l0FunctionViolations, l0Violations } from './sched-l0-resource-check.mjs'
import { checkLeaves } from './sched-leaf-primitive-check.mjs'
import { checkGovernance } from './sched-level-predicate-check.mjs'
import { evaluateOrder } from './sched-order-annotation-check.mjs'
import { evaluateGap } from './sched-recall-gap-gate.mjs'
import { checkConsistency } from './sched-resource-consistency-check.mjs'
import { tagViolations } from './sched-tag-check.mjs'
import { awaitedSendsInL0 } from './sched-unbounded-sender-check.mjs'
import { checkWhitelist } from './sched-whitelist-check.mjs'
import { missingGolden } from './sched-resource-golden-check.mjs'
import { buildInventory } from './sched-resource-inventory.mjs'
import { loadPolicy, maskRust, orderCoverage, rankFingerprint, scanOrderAnnotations, sourcesOf, stripTestItems, workspaceCrates } from './sched-utils.mjs'

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

// A registry of three S8 resources (a < b < c) and one S5 permit, with the
// reviewed fingerprint swapped for this table's own.
const familyRegistry = {
    items: [
        { ident: 't::a', rank: 1, scopeClass: 'S8' },
        { ident: 't::b', rank: 2, scopeClass: 'S8' },
        { ident: 't::c', rank: 3, scopeClass: 'S8' },
        { ident: 't::permit', rank: 4, scopeClass: 'S5' },
    ],
}
const familyPolicy = (exceptions = []) => ({ ...clone(policy), rankFingerprint: rankFingerprint(familyRegistry.items), orderViolations: [], orderExceptions: exceptions })
const annotated = (body) => scanOrderAnnotations([source('packages/app-lib/src/x.rs', `fn f() {\n${body}\n}\n`)])

test('order check compares ranks only within one family (K4.1, R5)', () => {
    const result = evaluateOrder(familyRegistry, familyPolicy(), annotated('    // sched-order: t::permit\n    let p = permit.acquire();\n    // sched-order: t::a\n    let a = a.lock();'))
    assert.deepEqual(result.descending, [])
    assert.equal(result.crossFamily, 1)
})

test('order check ends a hold at its block, its release, or its statement (K4.1)', () => {
    const block = annotated('    {\n        // sched-order: t::b\n        let b = b.lock();\n    }\n    // sched-order: t::a\n    let a = a.lock();')
    assert.deepEqual(evaluateOrder(familyRegistry, familyPolicy(), block).descending, [])
    const released = annotated('    // sched-order: t::b\n    let b = b.lock();\n    // sched-order-release: t::b\n    drop(b);\n    // sched-order: t::a\n    let a = a.lock();')
    assert.deepEqual(evaluateOrder(familyRegistry, familyPolicy(), released).descending, [])
    const temporary = annotated('    // sched-order: t::b temporary\n    b.lock().clear();\n    // sched-order: t::a\n    let a = a.lock();')
    assert.deepEqual(evaluateOrder(familyRegistry, familyPolicy(), temporary).descending, [])
    const nested = annotated('    // sched-order: t::b\n    let b = b.lock();\n    // sched-order: t::a\n    let a = a.lock();')
    assert.equal(evaluateOrder(familyRegistry, familyPolicy(), nested).descending.length, 1)
})

test('order check accepts a listed exception but not a cycle through it (K4.11)', () => {
    const exception = [{ held: 't::b', acquired: 't::a' }]
    const once = annotated('    // sched-order: t::b\n    let b = b.lock();\n    // sched-order: t::a\n    let a = a.lock();')
    const accepted = evaluateOrder(familyRegistry, familyPolicy(exception), once)
    assert.deepEqual([accepted.descending, accepted.errors], [[], []])
    assert.equal(accepted.exempted.length, 1)
    const cycle = annotated('    // sched-order: t::b\n    let b = b.lock();\n    // sched-order: t::a\n    let a = a.lock();\n    // sched-order: t::c\n    let c = c.lock();\n}\nfn g() {\n    // sched-order: t::a\n    let a = a.lock();\n    // sched-order: t::b\n    let b = b.lock();')
    assert.ok(evaluateOrder(familyRegistry, familyPolicy(exception), cycle).errors.some((error) => error.includes('cycle')))
})

test('order coverage asks for annotations only where a family has two resources (K4.1)', () => {
    const code = source('packages/app-lib/src/x.rs', 'fn one() {\n    let a = a.lock();\n}\nfn two() {\n    // sched-order: t::a\n    let a = a.lock();\n    let b = b.lock();\n}\n')
    const getpoints = {
        items: [
            { file: 'packages/app-lib/src/x.rs', line: 2, method: 'lock', idents: ['t::a'] },
            { file: 'packages/app-lib/src/x.rs', line: 6, method: 'lock', idents: ['t::a'] },
            { file: 'packages/app-lib/src/x.rs', line: 7, method: 'lock', idents: ['t::b'] },
        ],
    }
    const coverage = orderCoverage(getpoints, familyRegistry, scanOrderAnnotations([code]), [code])
    assert.deepEqual(coverage.exempt.map((point) => [point.line, point.rule]), [[2, 'F1']])
    assert.deepEqual(coverage.uncovered.map((point) => point.line), [7])
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
    assert.deepEqual(checkSpawns(counts, { entries: [{ file: 'packages/app-lib/src/new.rs', form: 'tokio::spawn', count: 1, reason: 'test' }] }).failures, [])
})

test('enter-ban rules 7 and 8 reject a restored gate (K2.9)', () => {
    const restored = source('packages/app-lib/src/install/critical_section.rs', 'pub fn enter() -> Gate { todo!() }\n')
    const wildcard = source('packages/app-lib/src/x.rs', 'fn f() {\n    match critical_section::enter() {\n        Gate::Protected(g) => {}\n        _ => {}\n    }\n}\n')
    const violations = enterBanViolations([restored, wildcard])
    assert.ok(violations.some((violation) => violation.includes('rule 7: fn enter')))
    assert.ok(violations.some((violation) => violation.includes('rule 7: critical_section::enter(')))
    assert.ok(violations.some((violation) => violation.includes('rule 8')))
    assert.deepEqual(enterBanViolations(sources), [])
})

test('whitelist check enforces the cap and the six criteria (K2.11, K2.12)', () => {
    assert.deepEqual(checkWhitelist(sources, policy, files.registry), [])
    const unprotected = clone(policy)
    unprotected.whitelist[0].bearer = 'theseus::nowhere::LOCK'
    unprotected.whitelist[1].worstCase = ''
    const failures = checkWhitelist(sources, unprotected, files.registry)
    assert.ok(failures.some((failure) => failure.includes('criterion 3')))
    assert.ok(failures.some((failure) => failure.includes('criterion 5')))
    const unguarded = clone(policy)
    unguarded.guardedWrites.push({ file: 'packages/app-lib/src/resources/execution.rs', fn: 'staging_prefix' })
    assert.ok(checkWhitelist(sources, unguarded, files.registry).some((failure) => failure.includes('without asking for WriteAccess')))
})

test('tag check rejects a forgeable tag (K2.1, K2.3)', () => {
    assert.deepEqual(tagViolations(sources), [])
    const tag = sources.find((candidate) => candidate.rel === 'packages/app-lib/src/scheduler/tag.rs')
    const convertible = source(tag.rel, `${tag.raw}\nimpl From<TagRecord> for Tag { fn from(r: TagRecord) -> Self { todo!() } }\n`)
    const forged = source('packages/app-lib/src/x.rs', 'fn f() { let _t = crate::scheduler::tag::Tag { id: 1, generation: 1, level: Level::L0, origin: String::new() }; }\n')
    const violations = tagViolations([...sources.filter((candidate) => candidate.rel !== tag.rel), convertible, forged])
    assert.ok(violations.some((violation) => violation.includes('implements From for Tag')))
    assert.ok(violations.some((violation) => violation.includes('builds a Tag outside tag.rs')))
})

test('the L0 process detector keeps its reactions dispatched (K3.3)', () => {
    assert.deepEqual(l0FunctionViolations(sources, policy), [])
    const detector = sources.find((candidate) => candidate.rel === 'packages/app-lib/src/state/process.rs')
    const leaky = source(
        detector.rel,
        detector.raw.replace(
            'let manually_killed = state.process_manager.was_manually_killed(uuid);',
            'let manually_killed = state.process_manager.was_manually_killed(uuid);\n        sqlx::query("SELECT 1").execute(&state.pool).await?;',
        ),
    )
    const violations = l0FunctionViolations([...sources.filter((candidate) => candidate.rel !== detector.rel), leaky], policy)
    assert.ok(violations.some((violation) => violation.includes('uses .pool')))
    assert.ok(violations.some((violation) => violation.includes('awaits')))
})

test('L0 never awaits a send (K3.4)', () => {
    const awaited = source('packages/app-lib/src/scheduler/l0/x.rs', 'async fn f(tx: Sender<u8>) {\n    tx.send(1).await.unwrap();\n}\n')
    assert.equal(awaitedSendsInL0([awaited], policy.l0Module).length, 1)
    assert.deepEqual(awaitedSendsInL0(sources, policy.l0Module), [])
})
