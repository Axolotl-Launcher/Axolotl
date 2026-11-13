// L0 posts without waiting (spec T3.3): inside the L0 module the event
// delivery path has no unbounded channel and no `.send(..).await`; delivery
// goes through the bounded `L0Queue` with `try_send`. (The executor's one-shot
// startup handshake uses `std::sync::mpsc`; it carries a handle once and is
// not an event path.) The ban is scoped to the L0 module only; the rest of the
// workspace keeps its existing unbounded channels.
//
// Usage: node scripts/axolotl/sched-unbounded-sender-check.mjs

import { unboundedInL0 } from './sched-bare-spawn-check.mjs'
import { isMain, lineOf, loadPolicy, sourcesOf, workspaceCrates } from './sched-utils.mjs'

// Method calls, turbofish (`tx.send::<u8>(..)`), and UFCS
// (`SinkExt::send(..)`) are all awaited sends.
const AWAITED_SEND = /(?:\b[A-Za-z_]\w*::)*\bsend\s*(?:::<[^;()]*>)?\s*\([^;]*?\)\s*\.await/g

export function awaitedSendsInL0(sources, l0Module) {
    return sources
        .filter((source) => source.rel.startsWith(`${l0Module}/`))
        .flatMap((source) => [...source.code.matchAll(AWAITED_SEND)].map((match) => ({ file: source.rel, line: lineOf(source.starts, match.index) })))
}

if (isMain(import.meta.url)) {
    const { l0Module } = loadPolicy()
    const sources = sourcesOf(workspaceCrates())
    const failures = [
        ...unboundedInL0(sources, l0Module).map((hit) => `${hit.file}:${hit.line} ${hit.form} in the L0 module`),
        ...awaitedSendsInL0(sources, l0Module).map((hit) => `${hit.file}:${hit.line} awaits a send in the L0 module; post with try_send`),
    ]
    const files = sources.filter((source) => source.rel.startsWith(`${l0Module}/`)).length
    console.log(`L0 module ${l0Module}: ${files} file(s)`)
    if (failures.length > 0) {
        for (const failure of failures) console.error(failure)
        process.exit(1)
    }
    console.log('no unbounded event channel and no awaited send inside L0')
}
