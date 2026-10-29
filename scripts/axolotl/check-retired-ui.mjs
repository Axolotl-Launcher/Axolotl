import { readdir, readFile } from 'node:fs/promises'
import path from 'node:path'

/**
 * Fails when a retired UI name comes back.
 *
 * Each of these names had a second implementation of one role -- two button
 * generations, two dropdowns, two names for the input, a colour alias with no
 * consumer. The duplicates were removed by migrating every call site, so a new
 * reference means a second implementation is being introduced again, and the
 * two will drift.
 *
 * Names are matched as written in source: an identifier (`ButtonStyled`), a
 * kebab-case tag (`<button-styled>`), or an import. Native `<input>` is not a
 * reference to the retired `Input` alias, so the tag patterns are case
 * sensitive and spell the component names out.
 */

const ROOTS = [
    'packages/ui/src',
    'apps/app-frontend/src',
    'apps/website/src',
    'packages/assets/styles',
]

const COMPONENT_NAMES = [
    'ButtonStyled',
    'LegacyButton',
    'NewButton',
    'DropdownSelect',
    'Input',
    'button-styled',
    'legacy-button',
    'new-button',
    'dropdown-select',
]

const TAG_PATTERN = new RegExp(`<\\/?(${COMPONENT_NAMES.join('|')})(?=[\\s/>])`, 'g')

// A multi-line import block is the house style, so the import check cannot be
// line based: `import {\n\tButtonStyled,\n} from '...'` never fits on one line.
const IMPORT_PATTERN = new RegExp(
    `\\bimport\\s+(?:type\\s+)?\\{[\\s\\S]{0,600}?\\b(${COMPONENT_NAMES.join('|')})\\b[\\s\\S]{0,600}?\\}\\s*from\\s*['"][^'"]+['"]`,
    'g',
)

const RETIRED = [
    { pattern: /\bButtonStyled\b/, reason: 'use Button, IconButton or ButtonLink' },
    { pattern: /\bLegacyButton\b/, reason: 'use Button, IconButton or ButtonLink' },
    { pattern: /\bNewButton\b/, reason: 'use Button, IconButton or ButtonLink' },
    { pattern: /\bDropdownSelect\b/, reason: 'use Combobox' },
    {
        pattern: /\bexport\s*\{[^}]*\bas\s+Input\b/,
        reason: 'the one-name alias was removed; export StyledInput instead',
    },
    { pattern: /\b(?:hover-)?color-fill\s*=/, reason: 'use the quiet-button interaction prop' },
    { pattern: /\bhighlighted-style\s*=/, reason: 'use semantic selected state and ARIA' },
    { pattern: /\bbtn-wrapper\b/, reason: 'the descendant-search wrapper was removed' },
    { pattern: /\bjoined-buttons\b/, reason: 'use ButtonGroup' },
    { pattern: /\banimated-dropdown\b/, reason: 'use Combobox, which exposes data-combobox' },
    {
        pattern: /\bmedal[-_]promo(?:tion)?\b/,
        reason: 'the colour was an exact alias of blue with no consumers',
    },
]

async function* walk(directory) {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
        if (/^(?:node_modules|dist|\.nuxt|\.output|__screenshots__)$/.test(entry.name)) continue
        const entryPath = path.join(directory, entry.name)
        if (entry.isDirectory()) yield* walk(entryPath)
        else if (/\.(?:vue|ts|js|mjs|scss|css)$/.test(entry.name)) yield entryPath
    }
}

function lineOf(contents, index) {
    return contents.slice(0, index).split(/\r?\n/).length
}

const violations = []
for (const root of ROOTS) {
    for await (const file of walk(root)) {
        const contents = await readFile(file, 'utf8')
        const lines = contents.split(/\r?\n/)

        // Imports are covered by the source check below, which also sees the
        // multi-line form, so the whole statement is skipped here rather than
        // reporting the same reference twice.
        let inImport = false
        for (const [index, line] of lines.entries()) {
            if (inImport) {
                if (/from\s*['"]/.test(line)) inImport = false
                continue
            }
            if (/^\s*import\b/.test(line)) {
                inImport = !/from\s*['"]/.test(line)
                continue
            }
            for (const retired of RETIRED) {
                if (retired.pattern.test(line)) {
                    violations.push(`${file}:${index + 1}: ${retired.reason}\n  ${line.trim()}`)
                }
            }
        }

        for (const [name, pattern] of [
            ['tag', TAG_PATTERN],
            ['import', IMPORT_PATTERN],
        ]) {
            pattern.lastIndex = 0
            for (const match of contents.matchAll(pattern)) {
                const reason =
                    name === 'tag'
                        ? `${match[1]} was retired; use the current component for this role`
                        : `${match[1]} was retired; import the current component for this role`
                violations.push(`${file}:${lineOf(contents, match.index)}: ${reason}`)
            }
        }
    }
}

if (violations.length > 0) {
    console.error(
        `Axolotl retired UI check failed: ${violations.length} retired UI reference(s).\n` +
            'Each retired name has a current replacement; see the reason on each line. If the mention is\n' +
            'documentation rather than code, move it to a doc outside the scanned roots.\n' +
            violations.join('\n'),
    )
    process.exit(1)
}

console.log('Axolotl retired UI check passed.')
