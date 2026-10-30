import { createServer } from 'node:http'

const host = '127.0.0.1'
const port = Number(process.env.CURSEFORGE_MOCK_PORT ?? 18081)
const projectId = 100001
const fileId = 200001
const fileBody = Buffer.from('Axolotl CurseForge local fixture\n')
const now = '2026-07-14T00:00:00Z'

const file = {
    id: fileId,
    gameId: 432,
    modId: projectId,
    isAvailable: true,
    displayName: 'Local Fixture 1.0.0',
    fileName: 'axolotl-local-fixture-1.0.0.jar',
    releaseType: 1,
    fileStatus: 4,
    hashes: [],
    fileDate: now,
    fileLength: fileBody.length,
    downloadCount: 1,
    fileSizeOnDisk: fileBody.length,
    downloadUrl: `http://${host}:${port}/files/axolotl-local-fixture-1.0.0.jar`,
    gameVersions: ['1.21.1', 'Fabric'],
    sortableGameVersions: [],
    dependencies: [],
    exposeAsAlternative: false,
    parentProjectFileId: null,
    alternateFileId: null,
    isServerPack: false,
    serverPackFileId: null,
    isEarlyAccessContent: false,
    earlyAccessEndDate: null,
    fileFingerprint: 123456789,
    modules: [],
}

const project = {
    id: projectId,
    gameId: 432,
    name: 'Axolotl Local Fixture',
    slug: 'axolotl-local-fixture',
    links: {
        websiteUrl: 'https://www.curseforge.com/minecraft/mc-mods',
        wikiUrl: null,
        issuesUrl: null,
        sourceUrl: null,
    },
    summary: 'A local CurseForge API fixture used to exercise search, details and installation.',
    status: 4,
    downloadCount: 42,
    isFeatured: false,
    primaryCategoryId: 6,
    categories: [
        {
            id: 6,
            gameId: 432,
            name: 'Fabric',
            slug: 'fabric',
            url: 'https://www.curseforge.com/minecraft/mc-mods/fabric',
            iconUrl: null,
            dateModified: now,
            isClass: false,
            classId: 6,
            parentCategoryId: null,
            displayIndex: 0,
        },
    ],
    classId: 6,
    authors: [{ id: 1, name: 'Local Fixture', url: 'https://example.invalid/local-fixture' }],
    logo: null,
    screenshots: [],
    mainFileId: fileId,
    latestFiles: [file],
    latestFilesIndexes: [
        {
            gameVersion: '1.21.1',
            fileId,
            filename: file.fileName,
            releaseType: 1,
            gameVersionTypeId: null,
            modLoader: 4,
        },
    ],
    dateCreated: now,
    dateModified: now,
    dateReleased: now,
    allowModDistribution: true,
    gamePopularityRank: 1,
    isAvailable: true,
    thumbsUpCount: 1,
}

function json(response, status, value) {
    response.writeHead(status, { 'content-type': 'application/json; charset=utf-8' })
    response.end(JSON.stringify(value))
}

function api(data, pagination) {
    return pagination ? { data, pagination } : { data }
}

const server = createServer((request, response) => {
    const url = new URL(request.url ?? '/', `http://${host}:${port}`)
    const path = url.pathname
    process.stdout.write(`${request.method} ${url.pathname}${url.search}\n`)

    if (path === '/files/axolotl-local-fixture-1.0.0.jar') {
        response.writeHead(200, {
            'content-type': 'application/java-archive',
            'content-length': fileBody.length,
        })
        response.end(fileBody)
        return
    }

    if (path === '/v1/games') return json(response, 200, api([]))
    if (path === '/v1/categories') return json(response, 200, api(project.categories))
    if (path === '/v1/mods/search') {
        return json(
            response,
            200,
            api([project], { index: 0, pageSize: 20, resultCount: 1, totalCount: 1 }),
        )
    }
    if (path === '/v1/mods' && request.method === 'POST') {
        return json(response, 200, api([project]))
    }
    if (path === `/v1/mods/${projectId}`) return json(response, 200, api(project))
    if (path === `/v1/mods/${projectId}/description`) {
        return json(
            response,
            200,
            api('<h2>Local fixture</h2><p>This response comes from tools/curseforge-mock.mjs.</p>'),
        )
    }
    if (path === `/v1/mods/${projectId}/files`) {
        return json(
            response,
            200,
            api([file], { index: 0, pageSize: 50, resultCount: 1, totalCount: 1 }),
        )
    }
    if (path === `/v1/mods/${projectId}/files/${fileId}`) {
        return json(response, 200, api(file))
    }
    if (path === `/v1/mods/${projectId}/files/${fileId}/download-url`) {
        return json(response, 200, api(file.downloadUrl))
    }
    if (path === `/v1/mods/${projectId}/files/${fileId}/changelog`) {
        return json(response, 200, api('<p>Initial local fixture release.</p>'))
    }
    if (path === '/v1/mods/files' && request.method === 'POST') {
        return json(response, 200, api([file]))
    }
    if (path === '/v1/fingerprints/432' && request.method === 'POST') {
        return json(
            response,
            200,
            api({
                isCacheBuilt: true,
                exactMatches: [],
                exactFingerprints: [],
                partialMatches: [],
                partialMatchFingerprints: {},
                installedFingerprints: [],
                unmatchedFingerprints: [],
            }),
        )
    }

    json(response, 404, { message: `No local fixture route for ${request.method} ${path}` })
})

server.listen(port, host, () => {
    process.stdout.write(`CurseForge fixture listening on http://${host}:${port}\n`)
})
