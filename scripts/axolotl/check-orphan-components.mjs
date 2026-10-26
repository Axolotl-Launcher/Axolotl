import { readdir, readFile, stat } from 'node:fs/promises'
import path from 'node:path'

/**
 * Fails when a component is neither exported from a barrel nor referenced
 * anywhere.
 *
 * Such a file is invisible: nothing imports it, no barrel lists it, and
 * nothing breaks when it rots. Six of them had accumulated in `packages/ui`
 * (a dead Breadcrumbs, LargeRadioButton, PageHeader, TagIcon,
 * ModalLoadingIndicator and an earlier batch), each one reading as supported
 * API while being unreachable.
 *
 * Components deliberately kept out of a barrel but imported by path are fine
 * — this only flags files nothing references at all.
 */

const COMPONENT_ROOT = 'packages/ui/src/components'
const SEARCH_ROOTS = ['packages/ui/src', 'apps/app-frontend/src', 'apps/website/src']

async function* walk(directory, extensions) {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
        if (/node_modules|\.vite$|dist|\.output|__screenshots__/.test(entry.name)) continue
        const entryPath = path.join(directory, entry.name)
        if (entry.isDirectory()) yield* walk(entryPath, extensions)
        else if (extensions.test(entry.name)) yield entryPath
    }
}

for (const root of [COMPONENT_ROOT, ...SEARCH_ROOTS]) {
    try {
        if (!(await stat(root)).isDirectory()) throw new Error('not a directory')
    } catch {
        console.error(`Axolotl component orphan check failed:\nscanned root is missing: ${root}`)
        process.exit(1)
    }
}

const sources = []
for (const root of SEARCH_ROOTS) {
    for await (const file of walk(root, /\.(vue|ts|js|mjs)$/)) {
        sources.push({ file, text: await readFile(file, 'utf8') })
    }
}

const orphans = []

/**
 * Reachability, not text coincidence: a component counts as referenced when it
 * is imported by name or path, or used as a tag / dynamic binding. A bare word
 * match would be satisfied by a comment, a string, or an unrelated identifier
 * of the same name, which would let a dead component evade the check.
 */
function isReferenced(name, text) {
    const importStatement = new RegExp(
        `import\\s[^;]*\\b${name}\\b[^;]*from\\s*['"][^'"]+['"]`,
    )
    const namedImport = new RegExp(`\\b${name}\\b\\s*[,}]`)
    const elementTag = new RegExp(`<${name}[\\s/>]`)
    const dynamicBinding = new RegExp(`\\b(?:is|component)\\s*[:=]\\s*['"]?${name}\\b`)
    return (
        importStatement.test(text) ||
        (namedImport.test(text) && /import\s/.test(text)) ||
        elementTag.test(text) ||
        dynamicBinding.test(text)
    )
}

for await (const component of walk(COMPONENT_ROOT, /\.vue$/)) {
    const name = path.basename(component, '.vue')
    const componentDirectory = path.dirname(component)

    // Each component is accountable to its own barrel; one that is not exported
    // there is only suspicious, not dead, unless nothing else reaches it.
    const barrelPath = path.join(componentDirectory, 'index.ts')
    let exported = false
    try {
        exported = (await readFile(barrelPath, 'utf8')).includes(`./${name}.vue`)
    } catch {
        exported = false
    }
    if (exported) continue

    const referenced = sources.some(({ file, text }) => {
        if (file === component) return false
        return isReferenced(name, text)
    })

    if (!referenced) orphans.push(component)
}

if (orphans.length > 0) {
    console.error(
        `Axolotl component orphan check failed: ${orphans.length} component(s) are neither exported nor referenced.\n` +
            'Delete them, or export them from their barrel if they are meant to be public.\n' +
            orphans.join('\n'),
    )
    process.exit(1)
}

console.log('Axolotl component orphan check passed.')
