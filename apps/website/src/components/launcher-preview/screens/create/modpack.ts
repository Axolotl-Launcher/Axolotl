/**
 * Reads what `install_create_modpack_instance` takes from a local pack file: the
 * pack's name and version, its Minecraft version and loader, and the files it
 * will download. Supports the formats the launcher detects (`pack/detect.rs`):
 * Modrinth `.mrpack`, CurseForge `manifest.json`, MCBBS, MultiMC exports and HMCL.
 */
import { openZip, readZipJson, type ZipArchive } from './zip'

export interface LocalModpackInfo {
    name: string
    versionNumber: string | null
    gameVersion: string | null
    loader: string
    loaderVersion: string | null
    /** Files the install downloads, and their total size when the format records it. */
    downloads: { files: number; bytes: number }
    /** Bytes of the pack's own overrides, extracted into the instance. */
    overridesBytes: number
}

interface MrpackIndex {
    name?: string
    versionId?: string
    files?: { path: string; fileSize?: number }[]
    dependencies?: Record<string, string>
}

interface CurseForgeManifest {
    name?: string
    version?: string
    minecraft?: { version?: string; modLoaders?: { id: string; primary?: boolean }[] }
    files?: unknown[]
    overrides?: string
    addons?: { id: string; version: string }[]
}

interface McbbsManifest {
    name?: string
    version?: string
    addons?: { id: string; version: string }[]
    files?: unknown[]
}

interface MmcPack {
    components?: { uid: string; version?: string }[]
}

interface HmclManifest {
    name?: string
    version?: string
    gameVersion?: string
}

const mrpackLoaderKeys: [string, string][] = [
    ['fabric-loader', 'fabric'],
    ['quilt-loader', 'quilt'],
    ['neoforge', 'neoforge'],
    ['forge', 'forge'],
]

const mmcLoaderUids: Record<string, string> = {
    'net.fabricmc.fabric-loader': 'fabric',
    'org.quiltmc.quilt-loader': 'quilt',
    'net.minecraftforge': 'forge',
    'net.neoforged': 'neoforge',
}

function stem(fileName: string) {
    return fileName.replace(/\.(zip|mrpack)$/i, '')
}

function overridesSize(archive: ZipArchive, prefix: string) {
    return archive.entries
        .filter((entry) => !entry.directory && entry.name.startsWith(prefix))
        .reduce((total, entry) => total + entry.uncompressedSize, 0)
}

/** Parses a CurseForge `modLoaders[].id` such as `forge-47.2.0`. */
function parseLoaderId(id: string): { loader: string; version: string | null } {
    const [name, ...rest] = id.split('-')
    const loader = name === 'neoforge' ? 'neoforge' : name
    return { loader, version: rest.join('-') || null }
}

export async function readLocalModpack(source: Blob, fileName: string): Promise<LocalModpackInfo> {
    const archive = await openZip(source)
    const names = new Set(archive.entries.map((entry) => entry.name))
    const fallback: LocalModpackInfo = {
        name: stem(fileName),
        versionNumber: null,
        gameVersion: null,
        loader: 'vanilla',
        loaderVersion: null,
        downloads: { files: 0, bytes: 0 },
        overridesBytes: overridesSize(archive, ''),
    }

    if (names.has('mcbbs.packmeta')) {
        const manifest = await readZipJson<McbbsManifest>(archive, 'mcbbs.packmeta').catch(
            () => null,
        )
        const game = manifest?.addons?.find((addon) => addon.id === 'game')
        const loader = manifest?.addons?.find((addon) => addon.id !== 'game')
        return {
            ...fallback,
            name: manifest?.name || fallback.name,
            versionNumber: manifest?.version ?? null,
            gameVersion: game?.version ?? null,
            loader: loader ? parseLoaderId(loader.id).loader : 'vanilla',
            loaderVersion: loader?.version ?? null,
            downloads: { files: manifest?.files?.length ?? 0, bytes: 0 },
            overridesBytes: overridesSize(archive, 'overrides/'),
        }
    }

    if (names.has('mmc-pack.json')) {
        const pack = await readZipJson<MmcPack>(archive, 'mmc-pack.json').catch(() => null)
        const minecraft = pack?.components?.find((component) => component.uid === 'net.minecraft')
        const loader = pack?.components?.find((component) => mmcLoaderUids[component.uid])
        return {
            ...fallback,
            gameVersion: minecraft?.version ?? null,
            loader: loader ? mmcLoaderUids[loader.uid] : 'vanilla',
            loaderVersion: loader?.version ?? null,
        }
    }

    if (names.has('modrinth.index.json')) {
        const index = await readZipJson<MrpackIndex>(archive, 'modrinth.index.json').catch(
            () => null,
        )
        const dependencies = index?.dependencies ?? {}
        const loaderKey = mrpackLoaderKeys.find(([key]) => dependencies[key])
        const files = index?.files ?? []
        return {
            ...fallback,
            name: index?.name || fallback.name,
            versionNumber: index?.versionId ?? null,
            gameVersion: dependencies.minecraft ?? null,
            loader: loaderKey?.[1] ?? 'vanilla',
            loaderVersion: loaderKey ? dependencies[loaderKey[0]] : null,
            downloads: {
                files: files.length,
                bytes: files.reduce((total, entry) => total + (entry.fileSize ?? 0), 0),
            },
            overridesBytes:
                overridesSize(archive, 'overrides/') + overridesSize(archive, 'client-overrides/'),
        }
    }

    if (names.has('manifest.json')) {
        const manifest = await readZipJson<CurseForgeManifest>(archive, 'manifest.json').catch(
            () => null,
        )
        const primary =
            manifest?.minecraft?.modLoaders?.find((entry) => entry.primary) ??
            manifest?.minecraft?.modLoaders?.[0]
        const parsed = primary ? parseLoaderId(primary.id) : null
        return {
            ...fallback,
            name: manifest?.name || fallback.name,
            versionNumber: manifest?.version ?? null,
            gameVersion: manifest?.minecraft?.version ?? null,
            loader: parsed?.loader ?? 'vanilla',
            loaderVersion: parsed?.version ?? null,
            downloads: { files: manifest?.files?.length ?? 0, bytes: 0 },
            overridesBytes: overridesSize(archive, `${manifest?.overrides ?? 'overrides'}/`),
        }
    }

    if (names.has('modpack.json')) {
        const manifest = await readZipJson<HmclManifest>(archive, 'modpack.json').catch(() => null)
        return {
            ...fallback,
            name: manifest?.name || fallback.name,
            versionNumber: manifest?.version ?? null,
            gameVersion: manifest?.gameVersion ?? null,
        }
    }

    return fallback
}
