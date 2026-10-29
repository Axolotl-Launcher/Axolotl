import { readdir, readFile } from 'node:fs/promises'
import { parse, TYPE } from '@formatjs/icu-messageformat-parser'

const localePairs = [
    [
        'apps/app-frontend/src/locales/en-US/index.json',
        'apps/app-frontend/src/locales/zh-CN/index.json',
    ],
    ['packages/ui/src/locales/en-US/index.json', 'packages/ui/src/locales/zh-CN/index.json'],
]

// Directories scanned for the placeholder check below.
const localeRoots = ['apps/app-frontend/src/locales', 'packages/ui/src/locales']

const failures = []
const allowedUntranslatedMessages = new Set([
    'Chaos Cubed',
    'MINECON Earth 2017',
    'Modrinth',
    '.minecraft',
    'Striding Hero',
    'Axolotl Launcher',
    'Explore high-quality Minecraft content on Modrinth.',
    'example.modrinth.gg',
    '{title} - {count}',
    'Hooks',
    '/path/to/java',
    'https://example.com/api/yggdrasil',
    'Fabric',
    'Forge',
    'Paper',
    'Cleanroom',
    'LiteLoader',
    'LogShare.CN',
    'LogShare AI',
    'NeoForge',
    'OptiFine',
    'Quilt',
    'Mr Pack',
    'TNT',
    'CurseForge',
    'BBCode',
    'mclo.gs',
    'CMI',
    'CSV',
    'AI',
    'HTML',
    'Mojang G1GC',
    'PCL G1GC',
    'PCL',
    'Shenandoah',
    'ZGC',
    'MineDown',
    'MiniMessage',
    'TabooLib',
    'TrChat',
    'TNT',
    'Java {version}',
    'Studio',
    'https://api-free.deepl.com/v2/translate',
    'Java',
    'Minecraft EULA',
    'server.properties',
    '{type} · {version}',
    '{value} MB',
    '> {command}',
    'Beta',
    'Release',
    'DNS over HTTPS',
    'https://doh.pub/dns-query',
    '{name} · {duration}',
    '{name} {level}',
    '{name} {number} · {x}, {z}',
    '{biome} · {name}',
    '{degrees}°',
    'MCP',
])

function messageText(value) {
    return typeof value === 'string' ? value : (value?.message ?? value?.defaultMessage ?? '')
}

function hasExplicitTranslation(value) {
    return typeof value === 'string' || typeof value?.message === 'string'
}

function argumentNames(message) {
    const names = new Set()
    const argumentTypes = new Set([
        TYPE.argument,
        TYPE.number,
        TYPE.date,
        TYPE.time,
        TYPE.select,
        TYPE.plural,
    ])

    function visit(elements) {
        for (const element of elements) {
            if (argumentTypes.has(element.type)) names.add(element.value)
            if (element.options) {
                for (const option of Object.values(element.options)) visit(option.value)
            }
            if (element.children) visit(element.children)
        }
    }

    visit(parse(message))
    return [...names].sort()
}

for (const [sourcePath, translationPath] of localePairs) {
    const source = JSON.parse(await readFile(sourcePath, 'utf8'))
    const translation = JSON.parse(await readFile(translationPath, 'utf8'))

    for (const key of Object.keys(source)) {
        if (!(key in translation)) {
            failures.push(`${translationPath}: missing ${key}`)
            continue
        }

        try {
            const sourceMessage = messageText(source[key])
            const translationMessage = messageText(translation[key])
            const sourceArguments = argumentNames(sourceMessage)
            const translationArguments = argumentNames(translationMessage)
            if (sourceArguments.join('\0') !== translationArguments.join('\0')) {
                failures.push(
                    `${translationPath}: ICU arguments for ${key} are [${translationArguments.join(', ')}], expected [${sourceArguments.join(', ')}]`,
                )
            }

            if (
                sourceMessage === translationMessage &&
                hasExplicitTranslation(translation[key]) &&
                /[A-Za-z]{2}/.test(sourceMessage) &&
                !allowedUntranslatedMessages.has(sourceMessage)
            ) {
                failures.push(`${translationPath}: untranslated ${key}`)
            }
        } catch (error) {
            failures.push(`${translationPath}: invalid ICU message ${key}: ${error.message}`)
        }
    }
}

// Every supported locale must keep the placeholders of its English source: a
// translation that drops `{count}` renders the braces literally, and one that
// invents a name throws when the message is formatted. Completeness is not
// required — most locales are still partial — so only the keys a locale does
// provide are compared.
//
// Mismatches that already existed are listed in `i18n-placeholder-baseline.json`.
// They stay reported below but do not fail the check, so the backlog can be
// worked down without blocking every unrelated change; anything new fails.
const baselinePath = 'scripts/axolotl/i18n-placeholder-baseline.json'
let baseline = new Set()
try {
    const recorded = JSON.parse(await readFile(baselinePath, 'utf8'))
    baseline = new Set(
        (recorded.entries ?? []).map((entry) => `${entry.locale}\u0000${entry.key}`),
    )
} catch {
    // No baseline yet: every mismatch counts as new.
}
const knownMismatches = []

for (const root of localeRoots) {
    const source = JSON.parse(await readFile(`${root}/en-US/index.json`, 'utf8'))
    for (const locale of await readdir(root)) {
        if (locale === 'en-US') continue
        let translation
        try {
            translation = JSON.parse(await readFile(`${root}/${locale}/index.json`, 'utf8'))
        } catch {
            continue
        }
        for (const key of Object.keys(translation)) {
            if (!(key in source)) continue
            try {
                const expected = argumentNames(messageText(source[key]))
                const actual = argumentNames(messageText(translation[key]))
                if (expected.join() !== actual.join()) {
                    const detail =
                        `${root}/${locale}: placeholders differ in ${key}` +
                        ` (expected ${expected.join(',') || 'none'}, got ${actual.join(',') || 'none'})`
                    const id = `${root}/${locale}\u0000${key}`
                    if (baseline.has(id)) knownMismatches.push(detail)
                    else failures.push(detail)
                }
            } catch (error) {
                failures.push(`${root}/${locale}: invalid ICU message ${key}: ${error.message}`)
            }
        }
    }
}

if (knownMismatches.length > 0) {
    console.warn(
        `Known placeholder mismatches (baselined, not failing): ${knownMismatches.length}`,
    )
}

if (failures.length > 0) {
    console.error(`Simplified Chinese coverage check failed:\n${failures.join('\n')}`)
    process.exit(1)
}

console.log('Simplified Chinese key coverage and ICU argument checks passed.')
