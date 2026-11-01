// Consistency criteria of spec §6.3.2 over the generated inventory files:
// 1 R ⊆ G and E ⊆ G at the same file:line; 2 |G| - |R| - |E| == 0;
// 4 every recall-gap entry carries a reason; 5 the scope equals the
// `cargo metadata --no-deps` members; 6 annotations name registered idents and
// the rank table matches the reviewed fingerprint. These prove the files agree
// with each other, not that they cover the code (that is the golden check).
//
// Usage: node scripts/axolotl/sched-resource-consistency-check.mjs [--dir <inventory dir>]

import {
    checkAnnotations,
    defaultOutDir,
    isMain,
    loadPolicy,
    outputPaths,
    parseArgs,
    rankFingerprint,
    readJson,
    scanOrderAnnotations,
    sourcesOf,
    workspaceCrates,
} from './sched-utils.mjs'

export const GAP_REASONS = ['cross-line-declaration', 'macro-generated', 're-exported-type', 'other']

export function checkConsistency(files, policy, crates, annotations) {
    const failures = []
    const key = (item) => `${item.ident}\0${item.file}\0${item.line}`
    const generated = new Set(files.inventory.items.map(key))
    const generatedIdents = new Set(files.inventory.items.map((item) => item.ident))

    for (const item of files.registry.items) if (!generated.has(key(item))) failures.push(`criterion 1: registered ${item.ident} (${item.file}:${item.line}) is not in G`)
    for (const item of files.exclusions.items) {
        const known = item.file ? generated.has(key(item)) : generatedIdents.has(item.ident)
        if (!known) failures.push(`criterion 1: excluded ${item.ident} is not in G`)
    }

    const registered = new Set(files.registry.items.map(key))
    const overlap = files.exclusions.items.filter((item) => item.file && registered.has(key(item)))
    for (const item of overlap) failures.push(`criterion 2: ${item.ident} is both registered and excluded`)
    const unclaimed = files.inventory.items.length - files.registry.items.length - files.exclusions.items.filter((item) => item.file).length
    if (unclaimed !== 0) failures.push(`criterion 2: |G| - |R| - |E| = ${unclaimed}`)

    for (const gap of files.recallGap.items) {
        if (!GAP_REASONS.includes(gap.reason)) failures.push(`criterion 4: recall-gap ${gap.key} has reason ${JSON.stringify(gap.reason)}`)
        if (gap.unconstrained !== true) failures.push(`criterion 4: recall-gap ${gap.key} is not marked unconstrained`)
    }

    const scope = new Set(files.inventory.scope.map((crate) => crate.dir))
    for (const crate of crates) if (!scope.has(crate.dir)) failures.push(`criterion 5: workspace member ${crate.dir} is missing from the scope`)
    for (const dir of scope) if (!crates.some((crate) => crate.dir === dir)) failures.push(`criterion 5: scope lists ${dir}, which is not a workspace member`)

    const { unreachable } = checkAnnotations(annotations, files.registry)
    for (const annotation of unreachable) failures.push(`criterion 6: ${annotation.file}:${annotation.line} sched-order names unregistered ${annotation.ident}`)
    const fingerprint = rankFingerprint(files.registry.items)
    if (fingerprint !== files.registry.fingerprint) failures.push('criterion 6: registry fingerprint does not match its own rank table')
    if (fingerprint !== policy.rankFingerprint) {
        failures.push(
            `criterion 6: rank table fingerprint ${fingerprint} differs from the reviewed ${policy.rankFingerprint}; ranks moved, so annotations must be re-synced and the fingerprint updated in review (never regenerated automatically, spec §11.8 R2)`,
        )
    }
    return failures
}

if (isMain(import.meta.url)) {
    const args = parseArgs(process.argv.slice(2))
    const paths = outputPaths(args.dir ?? defaultOutDir)
    const files = Object.fromEntries(Object.entries(paths).map(([name, file]) => [name, readJson(file)]))
    const crates = workspaceCrates()
    const failures = checkConsistency(files, loadPolicy(), crates, scanOrderAnnotations(sourcesOf(crates)))
    console.log(`scope: ${crates.map((crate) => crate.dir).join(', ')}`)
    if (failures.length > 0) {
        for (const failure of failures) console.error(failure)
        process.exit(1)
    }
    console.log('criteria 1, 2, 4, 5, 6: pass')
}
