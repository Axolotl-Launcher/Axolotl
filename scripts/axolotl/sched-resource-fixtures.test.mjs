// Fixtures F1–F7 of design v8 §4.7: declarations the inventory generator must
// parse from the real sources (cross-line statics, same-name statics in two
// modules, bare newtype fields that a type-shaped pattern cannot see).

import assert from 'node:assert/strict'
import { test } from 'node:test'
import { primitiveNames, scanDeclarations, scanMethodHits, sourcesOf, workspaceCrates } from './sched-utils.mjs'

const sources = sourcesOf(workspaceCrates())
const primitives = primitiveNames(sources)
const generated = scanDeclarations(sources, primitives)

function declared(file, ident) {
    return generated.find((item) => item.file === file && item.ident === ident)
}

const crossLine = [
    ['F1', 'apps/app/src/api/files.rs', 'theseus_gui::api::files::SCREENSHOT_THUMBNAILS'],
    ['F2', 'apps/app/src/api/search_cancellation.rs', 'theseus_gui::api::search_cancellation::SEARCH_CANCELLATIONS'],
    ['F3', 'apps/app/src/api/schematic_preview.rs', 'theseus_gui::api::schematic_preview::REQUEST_CANCELLATIONS'],
    ['F4', 'packages/app-lib/src/util/download/native_budget.rs', 'theseus::util::download::native_budget::AUTHORITY_BUDGETS'],
    ['F5', 'packages/app-lib/src/util/download/h2_stream_budget.rs', 'theseus::util::download::h2_stream_budget::AUTHORITY_BUDGETS'],
    ['F7', 'apps/app/src/api/files.rs', 'theseus_gui::api::files::LOCAL_ICON_CACHE'],
]

for (const [fixture, file, ident] of crossLine) {
    test(`${fixture}: ${ident} is parsed across lines`, () => {
        const item = declared(file, ident)
        assert.ok(item, `${ident} missing from ${file}`)
        const source = sources.find((candidate) => candidate.rel === file)
        assert.match(source.rawLines[item.line - 1], /static\s+\w+\s*:\s*\w+<\s*$/, 'the fixture must stay a declaration whose type continues on the next line')
        assert.match(item.type, /Mutex/)
    })
}

test('F4/F5: same-name statics stay distinct by module path', () => {
    const budgets = generated.filter((item) => item.name === 'AUTHORITY_BUDGETS').map((item) => item.module)
    assert.deepEqual(budgets.sort(), ['theseus::util::download::h2_stream_budget', 'theseus::util::download::native_budget'])
})

test('F6 (negative): bare newtype fields are found by method name, not by a type pattern', () => {
    const file = 'packages/app-lib/src/state/mod.rs'
    const fields = ['fetch_semaphore', 'download_semaphore', 'io_semaphore', 'api_semaphore', 'install_job_semaphore', 'install_db_semaphore']
    const source = sources.find((candidate) => candidate.rel === file)
    const typeShaped = /: (Arc<)?(tokio::sync::)?(Async)?(Mutex|RwLock|Semaphore|DashMap|DashSet)</
    const constructed = new Set(scanMethodHits([source]).filter((hit) => hit.method === 'const_new' || hit.method === 'Semaphore::new').map((hit) => hit.receiver))
    for (const field of fields) {
        const item = declared(file, `theseus::state::State.${field}`)
        assert.ok(item, `State.${field} missing`)
        assert.doesNotMatch(source.rawLines[item.line - 1], typeShaped, `State.${field} would be visible to the v7 type pattern`)
        assert.ok(constructed.has(field), `no Semaphore constructor attributed to State.${field}`)
    }
})
