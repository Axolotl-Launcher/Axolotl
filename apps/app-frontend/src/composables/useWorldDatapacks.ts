import { type ContentItem, defineMessage, useDebugLogger, useVIntl } from '@modrinth/ui'
import { computed, ref } from 'vue'

import {
    deleteDatapack,
    listDatapacks,
    setDatapackEnabled,
    type WorldWithDatapacks,
} from '@/helpers/datapacks'
import { formatDatapackVersion } from '@/helpers/pack-formats'

function datapackDescription(description: unknown): string | undefined {
    if (typeof description === 'string') return description
    if (Array.isArray(description)) {
        return description.map(datapackDescription).filter(Boolean).join(' ')
    }
    if (description && typeof description === 'object') {
        const record = description as Record<string, unknown>
        if (typeof record.text === 'string') {
            const extra = Array.isArray(record.extra)
                ? record.extra.map(datapackDescription).filter(Boolean).join('')
                : ''
            return record.text + extra
        }
        if (typeof record.translate === 'string') return record.translate
    }
    return undefined
}

export function useWorldDatapacks(getInstanceId: () => string) {
    const debugState = useDebugLogger('Mods:world-datapacks')
    const { formatMessage } = useVIntl()
    const worldDatapacks = ref<WorldWithDatapacks[]>([])
    const worldDatapackErrors = ref<string[]>([])
    let loadedInstanceId: string | undefined
    let worldDatapackRequest = 0

    async function loadWorldDatapacks() {
        const instanceId = getInstanceId()
        const request = ++worldDatapackRequest
        if (loadedInstanceId !== instanceId) {
            worldDatapacks.value = []
            worldDatapackErrors.value = []
            loadedInstanceId = instanceId
        }
        const isCurrent = () => request === worldDatapackRequest && instanceId === getInstanceId()
        try {
            const data = await listDatapacks(instanceId)
            if (!isCurrent()) return
            const previous = new Map(worldDatapacks.value.map((world) => [world.path, world]))
            worldDatapackErrors.value = data.flatMap((world) =>
                [world.read_error, world.state_error]
                    .filter(Boolean)
                    .map((error) => `${world.name}: ${error}`),
            )
            worldDatapacks.value = data.map((world) => {
                const old = previous.get(world.path)
                return world.read_error && old
                    ? { ...old, read_error: world.read_error, state_error: world.state_error }
                    : world
            })
            debugState('world datapacks loaded', {
                worlds: worldDatapacks.value.length,
                datapacks: worldDatapacks.value.reduce(
                    (total, world) => total + world.datapacks.length,
                    0,
                ),
            })
        } catch (error) {
            if (!isCurrent()) return
            worldDatapackErrors.value = [error instanceof Error ? error.message : String(error)]
            console.warn('Could not load world datapacks:', error)
        }
    }

    /**
     * World datapacks (files inside `saves/<world>/datapacks/`) are surfaced as
     * plain content items so the content tab treats them exactly like mods,
     * resource packs, shaders and schematics: the type filter pill and its count
     * come from real items, rows render in the content table grouped per save,
     * and delete goes through the world-datapack command.
     */
    const worldDatapackItems = computed<ContentItem[]>(() =>
        worldDatapacks.value.flatMap((entry) =>
            entry.datapacks.map((datapack) => {
                const id = `local:world-datapack:${entry.path}:${datapack.file_name}`
                const versionNumber = formatDatapackVersion(datapack)
                return {
                    id,
                    file_name: datapack.file_name,
                    file_path: `saves/${entry.path}/datapacks/${datapack.file_name}`,
                    project_type: 'datapack',
                    update: null,
                    origin_provider: null,
                    enabled: entry.state_error ? undefined : datapack.enabled,
                    external: true,
                    source_kind: 'world_datapack',
                    groupMeta: {
                        icon_url: entry.icon,
                        title: entry.name,
                        last_played: entry.last_played,
                        game_mode: entry.state_error ? undefined : entry.game_mode,
                        hardcore: entry.state_error ? undefined : entry.hardcore,
                    },
                    instanceOwnershipKind: 'local_discovered',
                    instanceMaterializationState: 'present',
                    instanceCapabilities: {
                        canToggle:
                            datapack.enabled !== undefined &&
                            !entry.read_error &&
                            !entry.state_error,
                        canDelete: true,
                        canUpdate: false,
                        canChangeVersion: false,
                        canRestorePackDefault: false,
                    },
                    project: {
                        id,
                        slug: datapack.display_name,
                        title: datapack.display_name,
                        icon_url: datapack.icon,
                        description: datapackDescription(datapack.description),
                    },
                    version: {
                        id: datapack.file_name,
                        version_number: versionNumber,
                        file_name: datapack.file_name,
                    },
                    provider_refs: [],
                } satisfies ContentItem
            }),
        ),
    )

    function isWorldDatapackItem(item: ContentItem) {
        return item.project_type === 'datapack' && item.source_kind === 'world_datapack'
    }

    async function deleteWorldDatapackItem(item: ContentItem) {
        const segments = (item.file_path ?? '').split('/')
        const worldPath = segments[1]
        const fileName = segments[3]
        if (!worldPath || !fileName) {
            throw new Error('Invalid world datapack path')
        }
        await deleteDatapack(getInstanceId(), worldPath, fileName)
        await loadWorldDatapacks()
    }

    async function toggleWorldDatapackItem(item: ContentItem, enabled: boolean) {
        const segments = (item.file_path ?? '').split('/')
        const worldPath = segments[1]
        const fileName = segments[3]
        if (!worldPath || !fileName) {
            throw new Error('Invalid world datapack path')
        }

        const world = worldDatapacks.value.find((entry) => entry.path === worldPath)
        const datapack = world?.datapacks.find((entry) => entry.file_name === fileName)
        if (
            !datapack ||
            datapack.enabled === undefined ||
            world?.read_error ||
            world?.state_error
        ) {
            throw new Error(
                formatMessage(
                    defineMessage({
                        id: 'app.instance.content.datapack-state-unavailable',
                        defaultMessage: 'Datapack state is unavailable. Refresh and retry.',
                    }),
                ),
            )
        }
        const previousEnabled = datapack.enabled
        if (datapack) {
            datapack.enabled = enabled
        }

        try {
            await setDatapackEnabled(getInstanceId(), worldPath, fileName, enabled)
        } catch (error) {
            if (datapack) {
                datapack.enabled = previousEnabled
            }
            throw error
        }
    }

    return {
        worldDatapacks,
        worldDatapackErrors,
        loadWorldDatapacks,
        worldDatapackItems,
        isWorldDatapackItem,
        deleteWorldDatapackItem,
        toggleWorldDatapackItem,
    }
}
