import { readFile, readdir } from 'node:fs/promises'
import path from 'node:path'

const roots = ['apps/app', 'apps/app-frontend', 'packages/app-lib']
const ignoredNames = new Set(['LICENSE', 'COPYING.md'])
const ignoredDirectories = new Set(['dist', 'build', 'node_modules', 'target', '.gradle', '.sqlx', 'bin'])
const forbiddenPatterns = [
    ['official product name', /Modrinth App/g],
    ['official API identity', /modrinth\/theseus/gi],
    ['official support identity', /support@modrinth\.com/gi],
    ['official deep link', /modrinth:\/\//gi],
    [
        'official telemetry',
        /phc_9Iqi6lFs9sr5BSqh9RRNRSJ0mATS9PSgirDiX3iOYJ|posthog\.modrinth\.com|ingest\.us\.sentry\.io/gi,
    ],
    ['advertising bridge', /plugin:ads|api::ads|Aditude/gi],
    ['official update feed', /launcher-files\.modrinth\.com\/updates\.json/gi],
    ['official signing service', /DIGICERT_ONE_SIGNER_CREDENTIALS/gi],
]

async function* files(directory) {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
        if (ignoredDirectories.has(entry.name)) continue
        const entryPath = path.join(directory, entry.name)
        if (entry.isDirectory()) yield* files(entryPath)
        else if (!ignoredNames.has(entry.name)) yield entryPath
    }
}

const failures = []
for (const root of roots) {
    for await (const file of files(root)) {
        let contents
        try {
            contents = await readFile(file, 'utf8')
        } catch {
            continue
        }

        for (const [label, pattern] of forbiddenPatterns) {
            pattern.lastIndex = 0
            if (pattern.test(contents)) failures.push(`${file}: ${label}`)
        }
    }
}

const tauriConfig = JSON.parse(await readFile('apps/app/tauri.conf.json', 'utf8'))
const frontendConfig = await readFile('apps/app-frontend/src/config.ts', 'utf8')
const requiredInvariants = [
    ['product name', tauriConfig.productName === 'Axolotl Launcher'],
    ['bundle identifier', tauriConfig.identifier === 'red.ghs.axolotl'],
    [
        'deep-link scheme',
        tauriConfig.plugins?.['deep-link']?.desktop?.schemes?.includes('axolotl') === true,
    ],
    [
        'User-Agent format',
        frontendConfig.includes('Axolotl-Launcher/${version} (${os})'),
    ],
    [
        'private Modrinth services disabled',
        frontendConfig.includes('privateModrinthServices: false'),
    ],
    ['GHS telemetry disabled', frontendConfig.includes('ghsTelemetry: false')],
]
for (const [label, valid] of requiredInvariants) {
    if (!valid) failures.push(`configuration: missing ${label}`)
}

if (failures.length > 0) {
    console.error(`Axolotl brand guard failed:\n${failures.join('\n')}`)
    process.exit(1)
}

console.log('Axolotl brand guard passed.')
