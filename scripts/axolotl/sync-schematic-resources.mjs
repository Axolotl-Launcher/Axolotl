import fs from 'node:fs/promises'

const SUMMARY_REF = 'd96c75fec200c4580dd75e033a76341521165461'
const ATLAS_REF = '41de69bb755237ac62cfc4a076fe17aafd41fb07'
const EXPECTED_VERSION = '26.3'
const OUTPUT_ROOT = new URL(
    '../../apps/app-frontend/src/lab/schematic-preview/assets/vanilla/',
    import.meta.url,
)

function summaryUrl(path) {
    return `https://raw.githubusercontent.com/misode/mcmeta/${SUMMARY_REF}/${path}`
}

function atlasUrl(path) {
    return `https://raw.githubusercontent.com/misode/mcmeta/${ATLAS_REF}/${path}`
}

async function download(url) {
    let failure
    for (let attempt = 1; attempt <= 3; attempt++) {
        try {
            const response = await fetch(url, { signal: AbortSignal.timeout(120_000) })
            if (!response.ok) throw new Error(`HTTP ${response.status}`)
            return new Uint8Array(await response.arrayBuffer())
        } catch (error) {
            failure = error
            if (attempt < 3) await new Promise((resolve) => setTimeout(resolve, attempt * 500))
        }
    }
    throw new Error(`Unable to download ${url}`, { cause: failure })
}

function parseJson(bytes, source) {
    try {
        return JSON.parse(new TextDecoder().decode(bytes))
    } catch (error) {
        throw new Error(`Invalid JSON from ${source}`, { cause: error })
    }
}

function requireObject(value, source) {
    if (!value || typeof value !== 'object' || Array.isArray(value)) {
        throw new Error(`${source} must contain a JSON object`)
    }
    return value
}

function pngDimensions(bytes) {
    const signature = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]
    if (bytes.length < 24 || signature.some((value, index) => bytes[index] !== value)) {
        throw new Error('The downloaded texture atlas is not a PNG image')
    }
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
    return [view.getUint32(16), view.getUint32(20)]
}

const sources = {
    definitions: summaryUrl('assets/block_definition/data.min.json'),
    models: summaryUrl('assets/model/data.min.json'),
    blocks: summaryUrl('blocks/data.min.json'),
    summaryVersion: summaryUrl('version.json'),
    atlasLayout: atlasUrl('all/data.min.json'),
    atlasImage: atlasUrl('all/atlas.png'),
    atlasVersion: atlasUrl('version.json'),
}

const entries = await Promise.all(
    Object.entries(sources).map(async ([name, url]) => [name, await download(url)]),
)
const downloaded = Object.fromEntries(entries)
const summaryVersion = requireObject(
    parseJson(downloaded.summaryVersion, sources.summaryVersion),
    'summary version',
)
const atlasVersion = requireObject(
    parseJson(downloaded.atlasVersion, sources.atlasVersion),
    'atlas version',
)
if (summaryVersion.id !== EXPECTED_VERSION || atlasVersion.id !== EXPECTED_VERSION) {
    throw new Error(
        `Expected ${EXPECTED_VERSION}, received summary ${summaryVersion.id} and atlas ${atlasVersion.id}`,
    )
}

const definitions = requireObject(
    parseJson(downloaded.definitions, sources.definitions),
    'block definitions',
)
const models = requireObject(parseJson(downloaded.models, sources.models), 'block models')
const blocks = requireObject(parseJson(downloaded.blocks, sources.blocks), 'block summary')
const atlasLayout = requireObject(
    parseJson(downloaded.atlasLayout, sources.atlasLayout),
    'atlas layout',
)
const defaultProperties = Object.fromEntries(
    Object.entries(blocks).map(([blockId, summary]) => {
        if (
            !Array.isArray(summary) ||
            !summary[1] ||
            typeof summary[1] !== 'object' ||
            Array.isArray(summary[1])
        ) {
            throw new Error(`Block summary ${blockId} has no default property object`)
        }
        return [blockId, summary[1]]
    }),
)

const [atlasWidth, atlasHeight] = pngDimensions(downloaded.atlasImage)
for (const [textureId, region] of Object.entries(atlasLayout)) {
    if (
        !Array.isArray(region) ||
        region.length !== 4 ||
        region.some((value) => !Number.isInteger(value) || value < 0) ||
        region[0] + region[2] > atlasWidth ||
        region[1] + region[3] > atlasHeight
    ) {
        throw new Error(`Atlas region ${textureId} is outside the ${atlasWidth}x${atlasHeight} image`)
    }
}

await Promise.all([
    fs.writeFile(new URL('block-state-index.json', OUTPUT_ROOT), JSON.stringify(definitions, null, 4)),
    fs.writeFile(new URL('block-model-index.json', OUTPUT_ROOT), JSON.stringify(models, null, 4)),
    fs.writeFile(
        new URL('block-property-defaults.json', OUTPUT_ROOT),
        JSON.stringify(defaultProperties, null, 4),
    ),
    fs.writeFile(new URL('texture-layout.json', OUTPUT_ROOT), JSON.stringify(atlasLayout, null, 4)),
    fs.writeFile(new URL('texture-atlas.png', OUTPUT_ROOT), downloaded.atlasImage),
])

console.log(
    `Updated schematic resources to ${EXPECTED_VERSION}: ${Object.keys(definitions).length} block definitions, ${Object.keys(models).length} models, and ${Object.keys(atlasLayout).length} atlas regions (${atlasWidth}x${atlasHeight}).`,
)
