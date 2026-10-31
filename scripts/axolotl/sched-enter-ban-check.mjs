// Text rules ⑦ and ⑧ of design v8 §6.2.3. These are auxiliary: the guarantee
// comes from the API shape (`with_critical_section` takes three closures, so a
// missing branch is E0061, and the guard cannot be built outside it). The text
// rules only catch someone restoring a matchable gate.
//   ⑦ no `fn enter` in the critical-section module and no `critical_section::enter(` call;
//   ⑧ no `match` whose scrutinee mentions `Gate` or `enter(` may have a `_ =>` arm.
//
// Usage: node scripts/axolotl/sched-enter-ban-check.mjs

import { isMain, lineOf, matchingBrace, sourcesOf, workspaceCrates } from './sched-utils.mjs'

const CRITICAL_SECTION = 'packages/app-lib/src/install/critical_section.rs'

export function enterBanViolations(sources) {
    const violations = []
    for (const source of sources) {
        const { code, starts, rel } = source
        if (rel === CRITICAL_SECTION) {
            for (const match of code.matchAll(/\bfn\s+enter\s*\(/g)) violations.push(`${rel}:${lineOf(starts, match.index)} rule 7: fn enter is back`)
        }
        for (const match of code.matchAll(/critical_section::enter\s*\(/g)) violations.push(`${rel}:${lineOf(starts, match.index)} rule 7: critical_section::enter( call`)
        for (const match of code.matchAll(/\bmatch\s+([^{;]*?)\{/g)) {
            if (!/\bGate\b|\benter\s*\(/.test(match[1])) continue
            const open = match.index + match[0].length - 1
            const body = code.slice(open + 1, matchingBrace(code, open))
            let depth = 0
            for (let i = 0; i < body.length; i++) {
                const ch = body[i]
                if ('{([' .includes(ch)) depth++
                else if ('})]'.includes(ch)) depth--
                else if (depth === 0 && /(^|[\s,|])_\s*=>/.test(body.slice(Math.max(0, i - 1), i + 4)) && ch === '_') {
                    violations.push(`${rel}:${lineOf(starts, open + 1 + i)} rule 8: wildcard arm in a match on ${match[1].trim()}`)
                }
            }
        }
    }
    return violations
}

if (isMain(import.meta.url)) {
    const violations = enterBanViolations(sourcesOf(workspaceCrates()))
    if (violations.length > 0) {
        for (const violation of violations) console.error(violation)
        process.exit(1)
    }
    console.log('rules 7 and 8 (auxiliary text checks): no enter() gate and no wildcard arm over one')
}
