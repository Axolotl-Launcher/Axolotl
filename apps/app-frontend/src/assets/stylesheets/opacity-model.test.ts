import { readdirSync, readFileSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

import { expect, it } from 'vitest'

/**
 * Guards the invariants of the signed-ratio opacity model (see
 * `packages/assets/styles/opacity.scss`) that are cheap to break and expensive to
 * notice:
 *
 *  - a `calc()` inside a `color-mix()` percentage silently invalidates the whole
 *    declaration, so the surface drops out and paints transparent;
 *  - a rule that declares a baseline without publishing the scale leaves every
 *    rung resolved against the wrong baseline;
 *  - a `keep-NN` rung a stylesheet references but the scale never declares
 *    resolves to nothing, so the mix is invalid.
 *
 * The stylesheets are read from disk, so the check is deterministic and
 * independent of the built output.
 */

const appStylesheetsDir = fileURLToPath(new URL('.', import.meta.url))
const assetsStylesDir = fileURLToPath(
    new URL('../../../../../packages/assets/styles/', import.meta.url),
)

type Stylesheet = {
    readonly name: string
    readonly path: string
    readonly text: string
}

function readStylesheets(dir: string): Stylesheet[] {
    return readdirSync(dir)
        .filter((name) => name.endsWith('.scss'))
        .sort()
        .map((name) => {
            const file = path.join(dir, name)
            return { name, path: file, text: readFileSync(file, 'utf8') }
        })
}

const stylesheets = [...readStylesheets(assetsStylesDir), ...readStylesheets(appStylesheetsDir)]

const opacitySheet = stylesheets.find((sheet) => sheet.name === 'opacity.scss')

// A `//` line comment is stripped naively; the only `//` in these files that is
// not a comment is inside a `url('https://…')`, which has no braces and so cannot
// unbalance the block tracking below.
function stripComments(source: string): string {
    return source.replace(/\/\*[\s\S]*?\*\//g, '').replace(/\/\/[^\n]*/g, '')
}

/**
 * The argument text of every `color-mix(<here>)`, matched on balanced
 * parentheses so a nested function does not end the argument early.
 */
function colorMixArguments(source: string): string[] {
    const needle = 'color-mix('
    const args: string[] = []
    let from = 0
    for (;;) {
        const start = source.indexOf(needle, from)
        if (start === -1) break
        let depth = 1
        let index = start + needle.length
        while (index < source.length && depth > 0) {
            const char = source[index]
            if (char === '(') depth += 1
            else if (char === ')') depth -= 1
            index += 1
        }
        args.push(source.slice(start + needle.length, index - 1))
        from = index
    }
    return args
}

/** The inner text of every balanced `{…}` block, including nested ones. */
function ruleBlocks(source: string): string[] {
    const blocks: string[] = []
    const stack: number[] = []
    for (let index = 0; index < source.length; index += 1) {
        const char = source[index]
        if (char === '{') {
            stack.push(index)
        } else if (char === '}') {
            const start = stack.pop()
            if (start !== undefined) blocks.push(source.slice(start + 1, index))
        }
    }
    return blocks
}

function capture(source: string, pattern: RegExp): string[] {
    const values: string[] = []
    for (const match of source.matchAll(pattern)) {
        if (match[1] !== undefined) values.push(match[1])
    }
    return values
}

it('the scan sees both stylesheet directories', () => {
    const names = stylesheets.map((sheet) => sheet.name)
    expect(
        names.includes('opacity.scss'),
        `opacity.scss not found in ${names.join(', ')}`,
    ).toBeTruthy()
    expect(
        names.includes('global.scss'),
        `global.scss not found in ${names.join(', ')}`,
    ).toBeTruthy()
    expect(
        names.includes('variables.scss'),
        `variables.scss not found in ${names.join(', ')}`,
    ).toBeTruthy()
})

it('no color-mix() percentage is built from a calc()', () => {
    for (const sheet of stylesheets) {
        for (const argument of colorMixArguments(stripComments(sheet.text))) {
            expect(
                !argument.includes('calc('),
                `${sheet.name}: calc() inside color-mix() -> color-mix(${argument})`,
            ).toBeTruthy()
        }
    }
})

it('every rule that declares a baseline also publishes the scale', () => {
    let declarations = 0
    for (const sheet of stylesheets) {
        for (const block of ruleBlocks(stripComments(sheet.text))) {
            if (!/--opacity-baseline\s*:/.test(block)) continue
            declarations += 1
            // A declaration (`--opacity-ratio-3: …`), not a reference
            // (`var(--opacity-ratio-3)`): only the declaration publishes the rung in
            // this rule, which is what `var()` substitution at the declaring element
            // requires. Matching the reference form let a removed `@include` pass.
            expect(
                block,
                `${sheet.name}: a rule declares --opacity-baseline without publishing the scale`,
            ).toMatch(/--opacity-ratio-[a-z0-9-]+\s*:|@include\s+opacity-scale/)
        }
    }
    expect(declarations > 0, 'expected at least one --opacity-baseline declaration').toBeTruthy()
})

it('every keep-NN rung a stylesheet references is declared by opacity-scale', () => {
    expect(opacitySheet, 'opacity.scss must be part of the scan').toBeTruthy()
    const declared = new Set(
        capture(stripComments(opacitySheet.text), /--opacity-ratio-keep-(\d+)\s*:/g),
    )
    expect(
        declared.size > 0,
        'expected opacity-scale to declare at least one keep-NN rung',
    ).toBeTruthy()

    const missing = new Set<string>()
    for (const sheet of stylesheets) {
        for (const rung of capture(stripComments(sheet.text), /var\(--opacity-ratio-keep-(\d+)/g)) {
            if (!declared.has(rung)) missing.add(rung)
        }
    }

    expect(
        [...missing].sort(),
        `keep-NN rungs referenced but not declared in opacity-scale: ${[...missing].sort().join(', ')}`,
    ).toEqual([])
})
