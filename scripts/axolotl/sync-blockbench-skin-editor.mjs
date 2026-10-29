import { access, mkdir, readdir, readFile, rm, writeFile } from 'node:fs/promises'
import { constants } from 'node:fs'
import { gzipSync } from 'node:zlib'
import { dirname, relative, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const scriptDirectory = dirname(fileURLToPath(import.meta.url))
const repositoryRoot = resolve(scriptDirectory, '../..')
const blockbenchRoot = resolve(repositoryRoot, 'third-party/blockbench')
const destination = resolve(repositoryRoot, 'apps/app/resources/blockbench-skin')
const bundle = resolve(blockbenchRoot, 'dist/skin.bundle.js')

try {
    await access(bundle, constants.R_OK)
} catch {
    throw new Error(
        `Missing ${bundle}. Run npm run build-skin in ${blockbenchRoot} before syncing.`,
    )
}

await mkdir(destination, { recursive: true })

const expectedFiles = new Set()
await Promise.all([
    syncTree(resolve(blockbenchRoot, 'assets')),
    syncTree(resolve(blockbenchRoot, 'css')),
    syncTree(resolve(blockbenchRoot, 'font')),
    syncFile(resolve(blockbenchRoot, 'index.html')),
])

const bundleContents = await readFile(bundle)
await writeIfChanged(resolve(destination, 'dist/skin.bundle.js.gz'), gzipSync(bundleContents, { level: 9 }))
expectedFiles.add('dist/skin.bundle.js.gz')
await removeStaleFiles(destination)

async function syncTree(sourceDirectory) {
    for (const entry of await readdir(sourceDirectory, { withFileTypes: true })) {
        const sourcePath = resolve(sourceDirectory, entry.name)
        if (entry.isDirectory()) await syncTree(sourcePath)
        else if (entry.isFile()) await syncFile(sourcePath)
    }
}

async function syncFile(sourcePath) {
    const relativePath = relative(blockbenchRoot, sourcePath).replaceAll('\\', '/')
    expectedFiles.add(relativePath)
    await writeIfChanged(resolve(destination, relativePath), await readFile(sourcePath))
}

async function writeIfChanged(destinationPath, contents) {
    try {
        const existing = await readFile(destinationPath)
        if (existing.equals(contents)) return
    } catch {}
    await mkdir(dirname(destinationPath), { recursive: true })
    await writeFile(destinationPath, contents)
}

async function removeStaleFiles(directory) {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
        const path = resolve(directory, entry.name)
        if (entry.isDirectory()) {
            await removeStaleFiles(path)
            if ((await readdir(path)).length === 0) await rm(path, { recursive: true })
            continue
        }
        if (entry.isFile() && !expectedFiles.has(relative(destination, path).replaceAll('\\', '/'))) {
            await rm(path)
        }
    }
}
