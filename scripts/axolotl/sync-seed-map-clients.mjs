import { createHash } from 'node:crypto'
import fs from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { spawnSync } from 'node:child_process'

const directory = process.argv[2]
if (!directory)
    throw new Error(
        'Usage: node scripts/axolotl/sync-seed-map-clients.mjs <jar-directory> [--import]',
    )
const output = path.resolve(directory)
await fs.mkdir(output, { recursive: true })
const root = fileURLToPath(new URL('../..', import.meta.url))
const rust = await fs.readFile(path.join(root, 'apps/app/src/seed_map/mod.rs'), 'utf8')
const section = rust.split('const JAVA_VERSIONS:')[1].split('];')[0]
const targets = [...section.matchAll(/\("([^"]+)",\s*(-?\d+)\)/g)].map((match) => match[1])
const canonical = targets.filter(
    (version) =>
        version.startsWith('b') ||
        version.split('.').length === 3 ||
        Number(version.split('.')[0]) >= 26,
)
const clientId = (version) => ({ '1.0.0': '1.0', '1.1.0': '1.1' })[version] ?? version
const versions = [...new Set([...canonical.map(clientId), '26.1.2', 'b1.7.3', 'b1.8.1', '1.21.7'])]

async function fetchBytes(url) {
    let failure
    for (let attempt = 0; attempt < 4; attempt++) {
        try {
            const response = await fetch(url, { signal: AbortSignal.timeout(240_000) })
            if (!response.ok) throw new Error(`HTTP ${response.status}: ${url}`)
            return Buffer.from(await response.arrayBuffer())
        } catch (error) {
            failure = error
            if (attempt < 3)
                await new Promise((resolve) => setTimeout(resolve, (attempt + 1) * 1000))
        }
    }
    throw failure
}
const json = async (url) => JSON.parse((await fetchBytes(url)).toString('utf8'))
const manifestUrl = 'https://piston-meta.mojang.com/mc/game/version_manifest_v2.json'
const manifest = await json(manifestUrl)
const results = []
const queue = [...versions]
async function download(version) {
    const entry = manifest.versions.find((entry) => entry.id === version)
    if (!entry) throw new Error(`Official client metadata is unavailable: ${version}`)
    const metadataBytes = await fetchBytes(entry.url)
    if (entry.sha1 && createHash('sha1').update(metadataBytes).digest('hex') !== entry.sha1)
        throw new Error(`Version metadata checksum mismatch: ${version}`)
    const metadata = JSON.parse(metadataBytes.toString('utf8'))
    const client = metadata.downloads?.client
    if (!client?.url || !client.sha1) throw new Error(`Client download missing: ${version}`)
    const destination = path.join(output, `${version}.jar`)
    let bytes
    try {
        bytes = await fs.readFile(destination)
    } catch (error) {
        if (error.code !== 'ENOENT') throw error
    }
    const valid = (data) =>
        data &&
        data.length === client.size &&
        createHash('sha1').update(data).digest('hex') === client.sha1
    const reused = valid(bytes)
    if (!reused) {
        bytes = await fetchBytes(client.url)
        if (!valid(bytes)) throw new Error(`Client checksum mismatch: ${version}`)
        await fs.writeFile(`${destination}.part`, bytes)
        await fs.rename(`${destination}.part`, destination)
    }
    await fs.writeFile(path.join(output, `${version}.json`), metadataBytes)
    results.push({
        version,
        file: `${version}.jar`,
        size: bytes.length,
        sha1: client.sha1,
        sha256: createHash('sha256').update(bytes).digest('hex'),
        url: client.url,
        metadataUrl: entry.url,
        engineVersions: targets.filter((target) => clientId(target) === version),
    })
    console.log(
        `${reused ? 'Verified' : 'Downloaded'} ${version}: ${(bytes.length / 1048576).toFixed(1)} MiB (${results.length}/${versions.length})`,
    )
}
await Promise.all(
    Array.from({ length: 3 }, async () => {
        while (queue.length) await download(queue.shift())
    }),
)
await fs.writeFile(
    path.join(output, 'manifest.json'),
    JSON.stringify(
        {
            source: manifestUrl,
            clients: results.sort((a, b) =>
                a.version.localeCompare(b.version, 'en', { numeric: true }),
            ),
        },
        null,
        2,
    ) + '\n',
)
console.log(`Verified ${results.length} official clients in ${output}`)

if (process.argv.includes('--import')) {
    for (const client of results.sort(
        (a, b) => Number(b.version === '26.3') - Number(a.version === '26.3'),
    )) {
        const result = spawnSync(
            process.execPath,
            [
                path.join(root, 'scripts/axolotl/sync-seed-map-templates.mjs'),
                path.join(output, client.file),
                client.version,
            ],
            { stdio: 'inherit' },
        )
        if (result.status !== 0) throw new Error(`Resource extraction failed: ${client.version}`)
    }
    const catalogPath = path.join(root, 'apps/app-frontend/src/lab/seed-map/assets/catalog.json')
    const catalog = JSON.parse(await fs.readFile(catalogPath, 'utf8'))
    const bridge = await fs.readFile(
        path.join(root, 'apps/app/src/seed_map/cubiomes_bridge.c'),
        'utf8',
    )
    const codes = new Map(
        [...section.matchAll(/\("([^"]+)",\s*(-?\d+)\)/g)].map((match) => [match[1], match[2]]),
    )
    const mappings = new Map(
        [...bridge.matchAll(/case (-?\d+): return MC_([A-Z0-9_]+);/g)].map((match) => [
            match[1],
            match[2],
        ]),
    )
    const resourceVersion = (version) => {
        if (catalog.versions[version] && !catalog.versions[version].resourceVersion) return version
        const name = mappings.get(codes.get(version))
        if (!name) throw new Error(`Engine mapping missing: ${version}`)
        return clientId(name.replace(/^B/, 'b').replaceAll('_', '.'))
    }
    for (const version of targets) {
        const source = resourceVersion(version)
        if (!catalog.versions[source]) throw new Error(`Resource pack missing: ${source}`)
        if (source !== version) catalog.versions[version] = { resourceVersion: source }
    }
    await fs.writeFile(catalogPath, JSON.stringify(catalog, null, 4) + '\n')
    const nativeRoot = path.join(root, 'apps/app/src/seed_map')
    const lines = [
        "pub(super) fn ancient_city_resources(version: &str) -> Option<(&'static [u8], &'static [u8])> {",
        '    match version {',
    ]
    for (const version of Object.keys(catalog.versions)) {
        const source = catalog.versions[version].resourceVersion ?? version
        if (!catalog.versions[source].structures.includes('ancient-city')) continue
        lines.push(
            `        "${version}" => Some((include_bytes!("assets/versions/${source}/ancient-city-layout.json.gz"), include_bytes!("assets/versions/${source}/ancient-city-blocks.json.gz"))),`,
        )
    }
    lines.push('        _ => None,', '    }', '}', '')
    await fs.writeFile(path.join(nativeRoot, 'template_resources.rs'), lines.join('\n'))
    console.log(
        `Imported ${results.length} client resource packs and registered ${targets.length} engine version names.`,
    )
}
