import { effectScope, reactive, watch } from 'vue'

import { type PreviewState, usePreviewDomain } from '../../../lib/state'

/*
 * The instance's core components (`packages/app-lib/src/api/instance/core_components.rs`):
 * archives patched into or replacing Minecraft.jar, kept per instance for the life of
 * the window. An added archive is read in the browser the way the backend reads the
 * file: its ZIP directory is validated and its SHA-1 and SHA-256 are computed.
 */

export type CoreComponentKind = 'jar_mod' | 'replacement_jar' | 'agent'

export interface CoreComponentSource {
    provider: string
    projectId: string | null
    versionId: string | null
    fileId: string | null
    pageUrl: string | null
}

export interface CoreComponent {
    id: string
    kind: CoreComponentKind
    fileName: string
    relativePath: string
    enabled: boolean
    removed: boolean
    order: number
    sha1: string | null
    sha256: string | null
    source: CoreComponentSource | null
    targetGameVersion: string
    createdAt: string
    modifiedAt: string
    failureReason: string | null
}

export interface CoreJarPreview {
    outputPath: string
    componentCount: number
    replacementComponentId: string | null
    entries: number
    sha1: string
    sha256: string
}

/**
 * Entries of an installed vanilla client jar outside META-INF (example value of
 * the preview's installed versions; the assembled jar starts from it).
 */
const VANILLA_CLIENT_JAR_ENTRIES = 19_874

function uuid() {
    if (typeof crypto !== 'undefined' && 'randomUUID' in crypto) return crypto.randomUUID()
    return `${Date.now().toString(16)}-${Math.random().toString(16).slice(2, 10)}`
}

/** Reads the names of a ZIP archive's entries from its central directory. */
export function readZipEntryNames(buffer: ArrayBuffer): string[] {
    const view = new DataView(buffer)
    const bytes = new Uint8Array(buffer)
    const minimum = 22
    if (buffer.byteLength < minimum) throw new Error('invalid Zip archive: Could not find EOCD')
    let eocd = -1
    for (
        let offset = buffer.byteLength - minimum;
        offset >= Math.max(0, buffer.byteLength - minimum - 65_535);
        offset--
    ) {
        if (view.getUint32(offset, true) === 0x06054b50) {
            eocd = offset
            break
        }
    }
    if (eocd < 0) throw new Error('invalid Zip archive: Could not find EOCD')
    let count = view.getUint16(eocd + 10, true)
    let directoryOffset = view.getUint32(eocd + 16, true)
    // ZIP64: the locator sits right before the end of central directory record.
    if ((count === 0xffff || directoryOffset === 0xffffffff) && eocd >= 20) {
        const locator = eocd - 20
        if (view.getUint32(locator, true) === 0x07064b50) {
            const record = Number(view.getBigUint64(locator + 8, true))
            if (view.getUint32(record, true) === 0x06064b50) {
                count = Number(view.getBigUint64(record + 32, true))
                directoryOffset = Number(view.getBigUint64(record + 48, true))
            }
        }
    }
    const decoder = new TextDecoder()
    const names: string[] = []
    let offset = directoryOffset
    for (let index = 0; index < count; index++) {
        if (offset + 46 > buffer.byteLength || view.getUint32(offset, true) !== 0x02014b50) {
            throw new Error('invalid Zip archive: Invalid central directory header')
        }
        const nameLength = view.getUint16(offset + 28, true)
        const extraLength = view.getUint16(offset + 30, true)
        const commentLength = view.getUint16(offset + 32, true)
        names.push(decoder.decode(bytes.subarray(offset + 46, offset + 46 + nameLength)))
        offset += 46 + nameLength + extraLength + commentLength
    }
    return names
}

/** `validate_archive_entry`: the entry's path inside the jar, or null for a skipped entry. */
function archiveEntryPath(name: string): string | null {
    if (!name || name.includes('\0')) {
        throw new Error('Core component contains an invalid ZIP entry name')
    }
    const normalized = name.replaceAll('\\', '/')
    if (
        normalized.startsWith('/') ||
        /^[a-zA-Z]:/.test(normalized) ||
        normalized.split('/').includes('..')
    ) {
        throw new Error(`Core component ZIP entry escapes the archive: ${name}`)
    }
    if (normalized.endsWith('/')) return null
    if (normalized.toUpperCase().startsWith('META-INF/')) return null
    return normalized
}

async function digest(algorithm: 'SHA-1' | 'SHA-256', buffer: ArrayBuffer) {
    if (typeof crypto === 'undefined' || !crypto.subtle) return null
    const hash = await crypto.subtle.digest(algorithm, buffer)
    return [...new Uint8Array(hash)].map((byte) => byte.toString(16).padStart(2, '0')).join('')
}

interface Payload {
    entries: string[]
}

function createCoreComponents(state: PreviewState) {
    const manifests = reactive<Record<string, CoreComponent[]>>({})
    const payloads = new Map<string, Payload>()

    function manifest(instanceId: string) {
        if (!manifests[instanceId]) manifests[instanceId] = []
        return manifests[instanceId]
    }

    function list(instanceId: string): CoreComponent[] {
        return [...manifest(instanceId)]
            .sort((a, b) => a.order - b.order)
            .map((component) => ({ ...component }))
    }

    async function add(
        instanceId: string,
        file: File,
        kind: 'jar_mod' | 'replacement_jar',
        targetGameVersion: string,
    ): Promise<CoreComponent> {
        const instance = state.instances.get(instanceId)
        if (!instance) throw new Error('Unknown instance')
        if (!targetGameVersion) {
            throw new Error('Core component target Minecraft version is required')
        }
        if (instance.game_version !== targetGameVersion) {
            throw new Error(
                `Core component targets Minecraft ${targetGameVersion}, but this instance uses ${instance.game_version}`,
            )
        }
        const buffer = await file.arrayBuffer()
        let names: string[]
        try {
            names = readZipEntryNames(buffer)
        } catch (error) {
            throw new Error(
                `Core component is not a valid ZIP/JAR archive: ${error instanceof Error ? error.message : String(error)}`,
            )
        }
        const entries: string[] = []
        for (const name of names) {
            const path = archiveEntryPath(name)
            if (path) entries.push(path)
        }
        if (buffer.byteLength === 0) throw new Error('Core component archive is empty')
        const [sha1, sha256] = await Promise.all([
            digest('SHA-1', buffer),
            digest('SHA-256', buffer),
        ])
        const components = manifest(instanceId)
        if (kind === 'replacement_jar') {
            for (const existing of components) {
                if (existing.kind === 'replacement_jar' && !existing.removed) {
                    existing.enabled = false
                }
            }
        }
        const id = uuid()
        const now = new Date().toISOString()
        const component: CoreComponent = {
            id,
            kind,
            fileName: file.name.trim() ? file.name : 'component.jar',
            relativePath: `files/${id}`,
            enabled: true,
            removed: false,
            order: components.reduce((max, entry) => Math.max(max, entry.order), -1) + 1,
            sha1,
            sha256,
            source: null,
            targetGameVersion,
            createdAt: now,
            modifiedAt: now,
            failureReason: null,
        }
        payloads.set(id, { entries })
        components.push(component)
        return { ...component }
    }

    function move(instanceId: string, componentId: string, direction: -1 | 1) {
        const components = manifest(instanceId).sort((a, b) => a.order - b.order)
        const active = components.filter((component) => !component.removed)
        const current = active.findIndex((component) => component.id === componentId)
        if (current < 0) throw new Error('Unknown active core component')
        const target = current + direction
        if (target >= 0 && target < active.length) {
            const order = active[current].order
            active[current].order = active[target].order
            active[target].order = order
            const now = new Date().toISOString()
            active[current].modifiedAt = now
            active[target].modifiedAt = now
        }
        return list(instanceId)
    }

    function setEnabled(instanceId: string, componentId: string, enabled: boolean) {
        const components = manifest(instanceId)
        const component = components.find((entry) => entry.id === componentId && !entry.removed)
        if (!component) throw new Error('Unknown active core component')
        if (enabled && component.kind === 'replacement_jar') {
            for (const other of components) {
                if (other.id !== componentId && other.kind === 'replacement_jar') {
                    other.enabled = false
                }
            }
        }
        component.enabled = enabled
        component.modifiedAt = new Date().toISOString()
        component.failureReason = null
        return { ...component }
    }

    function remove(instanceId: string, componentId: string) {
        const component = manifest(instanceId).find(
            (entry) => entry.id === componentId && !entry.removed,
        )
        if (!component) throw new Error('Unknown active core component')
        component.relativePath = `trash/${component.id}`
        component.enabled = false
        component.removed = true
        component.modifiedAt = new Date().toISOString()
    }

    function restore(instanceId: string, componentId: string) {
        const component = manifest(instanceId).find(
            (entry) => entry.id === componentId && entry.removed,
        )
        if (!component) throw new Error('Unknown deleted core component')
        component.relativePath = `files/${component.id}`
        component.removed = false
        component.modifiedAt = new Date().toISOString()
        component.failureReason = null
        return { ...component }
    }

    /** `preview_core_jar`: assembles the enabled components onto the installed jar. */
    async function preview(instanceId: string): Promise<CoreJarPreview | null> {
        const instance = state.instances.get(instanceId)
        if (!instance) throw new Error('Unknown instance')
        if (instance.install_stage !== 'installed') {
            throw new Error(`Minecraft ${instance.game_version} is not installed yet`)
        }
        const components = manifest(instanceId)
        const enabled = components
            .filter((component) => component.enabled && !component.removed)
            .sort((a, b) => a.order - b.order)
        if (enabled.length === 0) return null
        const mismatch = enabled.find(
            (component) => component.targetGameVersion !== instance.game_version,
        )
        if (mismatch) {
            const reason = `Core component ${mismatch.fileName} targets Minecraft ${mismatch.targetGameVersion}, not ${instance.game_version}`
            for (const component of enabled) component.failureReason = reason
            throw new Error(reason)
        }
        for (const component of enabled) component.failureReason = null
        const replacement = enabled.find((component) => component.kind === 'replacement_jar')
        const written = new Set<string>()
        let entries = 0
        // Later inputs are written first and win, as `assemble_archive` iterates in reverse.
        const inputs = enabled.filter((component) => component.kind === 'jar_mod').reverse()
        for (const component of inputs) {
            for (const name of payloads.get(component.id)?.entries ?? []) {
                if (written.has(name)) continue
                written.add(name)
                entries++
            }
        }
        if (replacement) {
            for (const name of payloads.get(replacement.id)?.entries ?? []) {
                if (written.has(name)) continue
                written.add(name)
                entries++
            }
        } else {
            entries += VANILLA_CLIENT_JAR_ENTRIES
        }
        const seed = `${instanceId}:${enabled.map((component) => component.sha256 ?? component.id).join(',')}`
        const encoded = new TextEncoder().encode(seed)
        const [sha1, sha256] = await Promise.all([
            digest('SHA-1', encoded.buffer as ArrayBuffer),
            digest('SHA-256', encoded.buffer as ArrayBuffer),
        ])
        return {
            outputPath: `${instance.path}/core-components/assembled/minecraft.jar`,
            componentCount: enabled.length,
            replacementComponentId: replacement?.id ?? null,
            entries,
            sha1: sha1 ?? '',
            sha256: sha256 ?? '',
        }
    }

    const scope = effectScope(true)
    scope.run(() => {
        watch(
            () => state.instances.instances.value.map((instance) => instance.id),
            (ids) => {
                for (const id of Object.keys(manifests)) {
                    if (ids.includes(id)) continue
                    for (const component of manifests[id]) payloads.delete(component.id)
                    Reflect.deleteProperty(manifests, id)
                }
            },
        )
    })

    return {
        value: { list, add, move, setEnabled, remove, restore, preview },
        dispose: () => scope.stop(),
    }
}

export type InstanceCoreComponents = ReturnType<typeof createCoreComponents>['value']

export function useInstanceCoreComponents(): InstanceCoreComponents {
    return usePreviewDomain('instance-settings:core-components', createCoreComponents)
}

/** `legacy_modloader_game_version`: ModLoader patches exist through Minecraft 1.6.2. */
export function canInstallModLoaderFor(gameVersion: string) {
    const version = gameVersion.trim().replace(/^v/, '')
    if (/^(?:a|b|inf-)/.test(version)) return true
    const match = /^1\.(\d+)(?:\.(\d+))?$/.exec(version)
    if (!match) return false
    const minor = Number(match[1])
    const patch = Number(match[2] ?? 0)
    return minor <= 6 && (minor < 6 || patch <= 2)
}
