import { readFile, readdir } from 'node:fs/promises'
import path from 'node:path'

/**
 * Scoped styles reach further than they look.
 *
 * A `<style scoped>` rule whose subject is a bare element -- `button`,
 * `input`, `label` -- compiles to `button[data-v-<scopeId>]`. Vue also puts
 * that attribute on the root element of a child component, so the rule styles
 * the migrated component rather than this file's own markup. That is how
 * `Button { width: 100% }` in `instance/Index.vue` stretched a page header
 * until the settings button was pushed off-screen, and how three more call
 * sites broke the same way without anyone noticing.
 *
 * A class or an id in the subject keeps the rule off components that merely
 * happen to be rooted on that tag, and `:deep(…)` is matched unscoped on
 * purpose, so both are left alone. Everything else has to be listed below with
 * the site that justifies it -- a deliberate decision at that site rather than
 * a blanket exemption.
 */

const roots = ['packages/ui/src', 'apps/app-frontend/src', 'apps/website/src']
const ignoredDirectories = new Set([
    'node_modules',
    '.vite',
    'dist',
    '.output',
    '__screenshots__',
    'public',
])

// The tags the interactive components are rooted on: `Button`/`IconButton`'s
// slot -> `button`, `FileInput`-style wrappers -> `label`, and the form
// controls. These are the ones where a stray scoped rule overrides the
// component's own layout and behaviour -- which is the failure this guards.
//
// Media and layout tags (`svg`, `img`, `progress`, `canvas`, `table`) are
// deliberately not listed: styling a child icon is ubiquitous in this
// codebase, the blast radius is a size rather than a broken layout, and
// reporting them buries the rules that matter.
const elementTags = new Set(['button', 'a', 'input', 'select', 'textarea', 'label'])

// Every entry names the file and the resolved selector, and says which element
// in that file's own markup the rule is for. A rule that reaches into a child
// component belongs in the source as `:deep(…)` instead of in this list.
const allowlist = new Map()
function allow(group, entries) {
    for (const [file, selector, reason] of entries) {
        allowlist.set(`${file}|${selector}`, { group, reason })
    }
}

// The element this rule styles is written in that same file's `<template>`, so
// it carries the file's own scope id and the rule is deliberate. Each entry was
// checked against the template, not the selector.
allow('styles-its-own-element', [
    [
        'packages/ui/src/components/skin/SkinButton.vue',
        '.skin-button--disabled button',
        'the button is written inside its own .skin-button (SkinButton.vue:65)',
    ],
    [
        'apps/app-frontend/src/components/lab/mod-translation/ModTranslationTaskTimeline.vue',
        '.toggle:focus-visible,.debug button:focus-visible',
        'both are raw buttons in this template (ModTranslationTaskTimeline.vue:44,67,74)',
    ],
    [
        'apps/app-frontend/src/components/lab/mod-translation/ModTranslationTaskTimeline.vue',
        '.debug button',
        'raw buttons under its own .debug div (ModTranslationTaskTimeline.vue:66,67,74)',
    ],
    [
        'apps/app-frontend/src/components/lab/recipe-generator/TagPalette.vue',
        '.recipe-tag-tabs button',
        'two raw tabs inside its own .recipe-tag-tabs (TagPalette.vue:192,195,204)',
    ],
    [
        'apps/app-frontend/src/pages/LabGradientText.vue',
        '.lab-color-swatch input',
        'raw colour input inside its own label (LabGradientText.vue:741,742)',
    ],
    [
        'apps/app-frontend/src/pages/LabRecipeGenerator.vue',
        '.recipe-tag-tabs button',
        'two raw tabs inside its own .recipe-tag-tabs (LabRecipeGenerator.vue:1626)',
    ],
    [
        'apps/app-frontend/src/pages/LabSeedMap.vue',
        '.ore-range-controls > label',
        'raw labels are direct children of its own .ore-range-controls (LabSeedMap.vue:2719,2728,2737)',
    ],
    [
        'apps/app-frontend/src/pages/LabSeedMap.vue',
        '.advanced-panel > label',
        'raw labels are direct children of its own .advanced-panel (LabSeedMap.vue:2982,2995,2999)',
    ],
    [
        'apps/website/src/components/ui/AxolotlFooter.vue',
        '.footer-legal a',
        'raw anchors inside its own .footer-legal (AxolotlFooter.vue:35,42,46)',
    ],
    [
        'apps/website/src/components/ui/SiteSettingsModal.vue',
        '.settings-sidebar button',
        'three raw buttons inside its own .settings-sidebar (SiteSettingsModal.vue:313,315,322,329), repeated in the narrow-viewport block at :700',
    ],
    [
        'apps/website/src/components/ui/SiteSettingsModal.vue',
        '.settings-sidebar button:hover',
        'same own buttons (SiteSettingsModal.vue:533)',
    ],
    [
        'apps/website/src/components/ui/SiteSettingsModal.vue',
        '.setting-row label',
        'raw labels inside its own .setting-row blocks (SiteSettingsModal.vue:366,373,380)',
    ],
    [
        'apps/website/src/pages/index.vue',
        '.footer .download-channel-picker label',
        'raw label inside its own .download-channel-picker (index.vue:1114,1115)',
    ],
    [
        'apps/website/src/pages/index.vue',
        '.footer .download-error-banner .download-error-links a',
        'one raw anchor inside its own .download-error-links (index.vue:1223,1224)',
    ],
])

// These style an element written in the file itself AND land on a `NuxtLink`,
// whose single root is an `<a>` that inherits the file's scope id. Reaching
// that root is wanted here (the links sit in the same list), so the entry
// records the reach rather than forbidding it.
allow('also-matches-a-child-root', [
    [
        'apps/website/src/components/ui/AxolotlFooter.vue',
        '.footer-links a',
        'raw anchor at AxolotlFooter.vue:61, plus NuxtLink roots in the same list',
    ],
    [
        'apps/website/src/components/ui/AxolotlFooter.vue',
        '.footer-links a:hover',
        'same list (AxolotlFooter.vue:141)',
    ],
    [
        'apps/website/src/components/ui/AxolotlHeader.vue',
        '.mobile-navigation a',
        'raw anchors at AxolotlHeader.vue:157,165, plus NuxtLink roots beside them',
    ],
    [
        'apps/website/src/components/ui/AxolotlHeader.vue',
        '.mobile-navigation a:hover',
        'same list (AxolotlHeader.vue:236)',
    ],
    [
        'apps/website/src/pages/index.vue',
        '.footer .download-section .download-card .description a',
        'raw anchors at index.vue:1150-1184, plus a NuxtLink root in the same block',
    ],
    [
        'apps/website/src/pages/index.vue',
        '.footer .download-section .download-card .description a:hover',
        'same block (index.vue:1966)',
    ],
    [
        'apps/website/src/pages/index.vue',
        '.footer .terms a',
        'raw anchor in the terms slot at index.vue:1236, plus its NuxtLink root',
    ],
])

const styleBlockPattern = /<style([^>]*)>([\s\S]*?)<\/style>/g

function isScoped(attributes) {
    return /(^|\s)scoped(\s|$|=)/.test(attributes)
}

function stripGroups(selector) {
    let result = ''
    let depth = 0
    for (const character of selector) {
        if (character === '[' || character === '(') depth++
        else if (character === ']' || character === ')') depth = Math.max(0, depth - 1)
        else if (depth === 0) result += character
    }
    return result
}

/** The compound a selector ends on, which is the element it actually styles. */
function subjectOf(selector) {
    const parts = selector
        .replace(/[>+~]/g, ' ')
        .split(/\s+/)
        .filter((part) => part.length > 0)
    return parts[parts.length - 1] ?? ''
}

function leadingTag(subject) {
    const match = /^([a-z][a-z0-9-]*)/i.exec(subject)
    return match ? match[1].toLowerCase() : null
}

/**
 * Walks a style block and yields each rule's full selector, resolving nesting
 * so `button { &:hover { … } }` reports the outer rule once. Comments are
 * skipped in place (rather than stripped up front) so every rule keeps its
 * exact offset in the block and the reported line is the one an editor shows.
 */
function* rulesIn(css) {
    const stack = []
    let buffer = ''
    for (let index = 0; index < css.length; index++) {
        if (css.startsWith('/*', index)) {
            const end = css.indexOf('*/', index + 2)
            index = end === -1 ? css.length : end + 1
            continue
        }
        if (css.startsWith('//', index) && css[index - 1] !== ':') {
            const end = css.indexOf('\n', index)
            index = end === -1 ? css.length : end
            continue
        }
        const character = css[index]
        if (character === '{') {
            const selector = buffer.trim()
            buffer = ''
            if (selector.startsWith('@')) {
                stack.push(null)
                continue
            }
            const parent = [...stack].reverse().find((entry) => entry !== null) ?? null
            const resolved = selector
                .split(',')
                .map((part) => {
                    const trimmed = part.trim().replace(/\s+/g, ' ')
                    if (!parent) return trimmed
                    // SCSS nesting: a relative selector hangs off the parent, and
                    // `&` is replaced by it.
                    return trimmed.includes('&') ? trimmed.replace(/&/g, parent) : `${parent} ${trimmed}`
                })
                .join(',')
            stack.push(resolved)
            yield { selector: resolved, own: selector, index }
        } else if (character === '}') {
            stack.pop()
            buffer = ''
        } else if (character === ';') {
            // Ends a declaration at any depth, so it must not be read as part of
            // the next rule's selector.
            buffer = ''
        } else {
            buffer += character
        }
    }
}

async function* walk(directory) {
    let entries
    try {
        entries = await readdir(directory, { withFileTypes: true })
    } catch {
        return
    }
    for (const entry of entries) {
        if (entry.isDirectory()) {
            if (ignoredDirectories.has(entry.name)) continue
            yield* walk(path.join(directory, entry.name))
        } else if (entry.name.endsWith('.vue')) {
            yield path.join(directory, entry.name)
        }
    }
}

const failures = []
const seen = new Set()

for (const root of roots) {
    for await (const file of walk(root)) {
        const source = await readFile(file, 'utf8')
        const key = file.split(path.sep).join('/')
        for (const block of source.matchAll(styleBlockPattern)) {
            if (!isScoped(block[1])) continue
            const innerStart = source.indexOf(block[0]) + block[0].indexOf(block[2])
            for (const rule of rulesIn(block[2])) {
                // `:deep(…)` (and its siblings) make everything after it unscoped
                // on purpose, so a selector carrying one has already opted in.
                if (/:(deep|slotted|global)\(/.test(rule.selector)) continue
                const subject = subjectOf(rule.selector)
                if (subject.startsWith('&')) continue
                const bare = stripGroups(subject)
                if (bare.includes('.') || bare.includes('#')) continue
                const tag = leadingTag(bare)
                if (!tag || !elementTags.has(tag)) continue

                const line = source.slice(0, innerStart + rule.index).split('\n').length

                seen.add(`${key}|${rule.selector}`)
                if (allowlist.has(`${key}|${rule.selector}`)) continue
                failures.push(`${key}:${line}  ${rule.selector}`)
            }
        }
    }
}

const stale = [...allowlist.keys()].filter((key) => !seen.has(key))
if (stale.length > 0) {
    console.warn(
        `Axolotl scoped style allowlist has ${stale.length} unused entr${stale.length === 1 ? 'y' : 'ies'}; remove ${stale.join(', ')}.`,
    )
}

if (failures.length > 0) {
    console.error(
        `Axolotl scoped style check failed: ${failures.length} rule(s) style a bare element inside a scoped block.\n` +
            'Such a selector also matches the root element of a child component, which is how a page header ended up\n' +
            'stretched. Give the subject a class, write it as `:deep(…)` if it really is meant to reach a component,\n' +
            'or add a justified entry to the allowlist in scripts/axolotl/check-scoped-styles.mjs.\n' +
            failures.join('\n'),
    )
    process.exit(1)
}

console.log(`Axolotl scoped style check passed (${allowlist.size} allowlisted).`)
