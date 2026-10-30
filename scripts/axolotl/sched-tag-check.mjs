// A scheduling tag must not be forgeable (spec §4.1, checklist K2.1/K2.3):
// `Tag` keeps every field private, derives neither `Deserialize` nor `Clone`,
// has no `From`/`TryFrom`/`Deserialize` impl, and is only built in tag.rs.
//
// Usage: node scripts/axolotl/sched-tag-check.mjs

import { isMain, lineOf, matchingBrace, sourcesOf, workspaceCrates } from './sched-utils.mjs'

const TAG = 'packages/app-lib/src/scheduler/tag.rs'

export function tagViolations(sources) {
    const violations = []
    const tag = sources.find((source) => source.rel === TAG)
    if (!tag) return [`${TAG} not found`]
    const definition = tag.code.match(/((?:#\[[^\]]*\]\s*)*)pub\s+struct\s+Tag\s*\{/)
    if (!definition) return [`${TAG}: struct Tag not found`]
    // Attributes come from the masked code, so doc comments cannot match.
    if (/\b(Deserialize|Clone|Copy)\b/.test(definition[1])) violations.push(`${TAG}: Tag derives Deserialize, Clone or Copy`)
    const open = definition.index + definition[0].length - 1
    const body = tag.code.slice(open + 1, matchingBrace(tag.code, open))
    if (/\bpub\b/.test(body)) violations.push(`${TAG}: Tag has a public field`)
    for (const source of sources) {
        for (const match of source.code.matchAll(/\bimpl\b[^{;]*\b(From|TryFrom|Deserialize|Clone)\b[^{;]*\bfor\s+(?:\w+::)*Tag\b(?!\w)/g)) {
            violations.push(`${source.rel}:${lineOf(source.starts, match.index)} implements ${match[1]} for Tag`)
        }
        if (source.rel === TAG) continue
        for (const match of source.code.matchAll(/(?<![\w:])(?:\w+::)*tag::Tag\s*\{|(?<![\w:.])Tag\s*\{\s*(?:id|generation|level|origin)\s*[:,]/g)) {
            violations.push(`${source.rel}:${lineOf(source.starts, match.index)} builds a Tag outside tag.rs`)
        }
    }
    return violations
}

if (isMain(import.meta.url)) {
    const violations = tagViolations(sourcesOf(workspaceCrates()))
    if (violations.length > 0) {
        for (const violation of violations) console.error(violation)
        process.exit(1)
    }
    console.log('Tag keeps private fields, no Deserialize/Clone/From/TryFrom, built only in tag.rs')
}
