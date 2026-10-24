import { computed, type Ref, ref } from 'vue'

import { projectIcon } from '../../data'
import modpackSnapshot from '../../data/modpacks.json'
import type { GameInstance, InstanceLoader } from '../types'

const MINUTE = 60 * 1000
const HOUR = 60 * MINUTE
const DAY = 24 * HOUR

/**
 * The app keeps instances in `<data dir>/red.ghs.axolotl/profiles/<name>`
 * (packages/app-lib/src/state/dirs.rs); this is that folder on macOS.
 */
const PROFILES_DIR = '/Users/steve/Library/Application Support/red.ghs.axolotl/profiles'

function ago(offsetMs: number) {
    return new Date(Date.now() - offsetMs).toISOString()
}

function instance(
    id: string,
    name: string,
    loader: InstanceLoader,
    loaderVersion: string | null,
    gameVersion: string,
    icon: string | null,
    playedAgo: number,
    timePlayed: number,
    createdAgo: number,
    extra: Partial<GameInstance> = {},
): GameInstance {
    return {
        id,
        path: `${PROFILES_DIR}/${name.replace(/[/:]/g, '')}`,
        install_stage: 'installed',
        launcher_feature_version: '1.10.0',
        name,
        icon_path: icon ?? undefined,
        game_version: gameVersion,
        loader,
        loader_version: loaderVersion ?? undefined,
        loader_components: loaderVersion
            ? [{ instanceId: id, kind: loader, version: loaderVersion, role: 'primary' }]
            : [],
        groups: [],
        link: null,
        update_channel: 'release',
        created: ago(createdAgo),
        modified: ago(playedAgo),
        last_played: ago(playedAgo),
        submitted_time_played: timePlayed,
        recent_time_played: timePlayed,
        java_path: null,
        extra_launch_args: [],
        memory: { maximum: 4096, automatic: true, optimize_before_launch: false },
        hooks: {},
        ...extra,
    }
}

/** An instance installed from a modpack version in `data/modpacks.json`. */
function modpackInstance(
    id: string,
    projectId: string,
    playedAgo: number,
    timePlayed: number,
    createdAgo: number,
): GameInstance {
    const pack = modpackSnapshot.packs.find((entry) => entry.project_id === projectId)
    if (!pack) throw new Error(`Missing modpack snapshot ${projectId}`)
    return instance(
        id,
        pack.pack_name,
        pack.loader as InstanceLoader,
        pack.loader_version,
        pack.game_version,
        projectIcon(projectId),
        playedAgo,
        timePlayed,
        createdAgo,
        { link: { type: 'modrinth_modpack', project_id: projectId, version_id: pack.version_id } },
    )
}

/**
 * Instances of the preview user. The modpack instances are the packs' real
 * published versions, with the game and loader versions their
 * `modrinth.index.json` pins; the Fabric instance uses the newest stable
 * Fabric Loader for 26.3 from the loader manifest snapshot.
 */
function defaultInstances(): GameInstance[] {
    return [
        modpackInstance(
            'fabulously-optimized',
            '1KVo5zza',
            5 * MINUTE,
            3 * HOUR + 42 * MINUTE,
            63 * DAY,
        ),
        modpackInstance('cobblemon', '5FFgwNNP', 26 * HOUR, 27 * HOUR, 120 * DAY),
        instance(
            'fabric-26-3',
            'Fabric 26.3',
            'fabric',
            '0.19.5',
            '26.3',
            null,
            2 * DAY,
            5 * HOUR + 10 * MINUTE,
            9 * DAY,
        ),
        modpackInstance('better-mc', 'shFhR8Vx', 3 * DAY, 9 * HOUR + 15 * MINUTE, 210 * DAY),
        modpackInstance('zombie-invade', 'l9m9tuPN', 9 * DAY, 2 * HOUR + 48 * MINUTE, 88 * DAY),
        instance(
            'vanilla',
            'Vanilla',
            'vanilla',
            null,
            '26.3',
            null,
            16 * DAY,
            41 * MINUTE,
            150 * DAY,
        ),
        // Identity and recommended client version: data/project/s8kML22l.json.
        instance(
            'hoplite',
            'Hoplite',
            'vanilla',
            null,
            '1.21.11',
            projectIcon('s8kML22l'),
            20 * DAY,
            0,
            21 * DAY,
            { link: { type: 'server_project', project_id: 's8kML22l' }, last_played: undefined },
        ),
    ]
}

export type SortDirection = 'asc' | 'desc'
export type LibrarySort = 'name' | 'last_played' | 'created' | 'time_played'
export type GroupMode = 'none' | 'custom' | 'loader' | 'game_version'

export function createInstancesState(_clock: unknown) {
    const instances = ref<GameInstance[]>(defaultInstances())
    const groups = ref<string[]>(['冒险', '性能优化'])
    const pinnedIds = ref<string[]>(['fabulously-optimized', 'better-mc'])
    instances.value[0].groups = ['性能优化']
    instances.value[1].groups = ['冒险']
    instances.value[2].groups = ['冒险']

    function get(id: string) {
        return instances.value.find((entry) => entry.id === id)
    }

    function sort(list: GameInstance[], sort: LibrarySort, direction: SortDirection) {
        const factor = direction === 'asc' ? 1 : -1
        return [...list].sort((a, b) => {
            switch (sort) {
                case 'name':
                    return a.name.localeCompare(b.name) * factor
                case 'created':
                    return (Date.parse(a.created) - Date.parse(b.created)) * factor
                case 'time_played':
                    return (a.submitted_time_played - b.submitted_time_played) * factor
                case 'last_played':
                default:
                    return (
                        (Date.parse(a.last_played ?? a.created) -
                            Date.parse(b.last_played ?? b.created)) *
                        factor
                    )
            }
        })
    }

    const pinned = computed(() => {
        const order = pinnedIds.value
        return order.map((id) => get(id)).filter((entry): entry is GameInstance => Boolean(entry))
    })

    function create(input: {
        name: string
        gameVersion: string
        loader: InstanceLoader
        loaderVersion: string | null
        iconPath?: string | null
        link?: GameInstance['link']
    }) {
        const id = `${
            input.name
                .toLowerCase()
                .replace(/[^a-z0-9]+/g, '-')
                .replace(/^-|-$/g, '') || 'instance'
        }-${Math.random().toString(36).slice(2, 6)}`
        const entry = instance(
            id,
            input.name,
            input.loader,
            input.loaderVersion,
            input.gameVersion,
            input.iconPath ?? null,
            0,
            0,
            0,
            // A new instance has never been played.
            { link: input.link ?? null, last_played: undefined },
        )
        instances.value = [entry, ...instances.value]
        return entry
    }

    function remove(id: string) {
        instances.value = instances.value.filter((entry) => entry.id !== id)
        pinnedIds.value = pinnedIds.value.filter((entry) => entry !== id)
    }

    function rename(id: string, name: string) {
        const entry = get(id)
        if (entry) entry.name = name
    }

    function setIcon(id: string, iconPath: string) {
        const entry = get(id)
        if (entry) entry.icon_path = iconPath
    }

    function setGroups(id: string, next: string[]) {
        const entry = get(id)
        if (entry) entry.groups = next
    }

    function addToGroup(id: string, group: string) {
        const entry = get(id)
        if (entry && !entry.groups.includes(group)) entry.groups = [...entry.groups, group]
    }

    function removeFromGroup(id: string, group: string) {
        const entry = get(id)
        if (entry) entry.groups = entry.groups.filter((entry) => entry !== group)
    }

    function createGroup(name: string) {
        if (!groups.value.includes(name)) groups.value = [...groups.value, name]
    }

    function renameGroup(from: string, to: string) {
        groups.value = groups.value.map((entry) => (entry === from ? to : entry))
        for (const entry of instances.value) {
            if (entry.groups.includes(from))
                entry.groups = entry.groups.map((g) => (g === from ? to : g))
        }
    }

    function deleteGroup(name: string) {
        groups.value = groups.value.filter((entry) => entry !== name)
        for (const entry of instances.value) {
            entry.groups = entry.groups.filter((group) => group !== name)
        }
    }

    function pin(id: string) {
        if (!pinnedIds.value.includes(id)) pinnedIds.value = [...pinnedIds.value, id]
    }

    function unpin(id: string) {
        pinnedIds.value = pinnedIds.value.filter((entry) => entry !== id)
    }

    function touch(id: string, playedMs: number) {
        const entry = get(id)
        if (!entry) return
        entry.last_played = new Date().toISOString()
        entry.recent_time_played += playedMs
        entry.submitted_time_played += playedMs
        entry.modified = entry.last_played
    }

    function recents(limit = 5): Ref<GameInstance[]> {
        return computed(() =>
            [...instances.value]
                .sort(
                    (a, b) =>
                        Date.parse(b.last_played ?? b.created) -
                        Date.parse(a.last_played ?? a.created),
                )
                .slice(0, limit),
        )
    }

    return {
        instances,
        groups,
        pinnedIds,
        pinned,
        recents,
        get,
        sort,
        create,
        remove,
        rename,
        setIcon,
        setGroups,
        addToGroup,
        removeFromGroup,
        createGroup,
        renameGroup,
        deleteGroup,
        pin,
        unpin,
        touch,
    }
}

export type InstancesState = ReturnType<typeof createInstancesState>

export const loaderNames: Record<InstanceLoader, string> = {
    vanilla: 'Vanilla',
    forge: 'Forge',
    fabric: 'Fabric',
    quilt: 'Quilt',
    neoforge: 'NeoForge',
    optifine: 'OptiFine',
    lite_loader: 'LiteLoader',
    cleanroom: 'Cleanroom',
    legacy_fabric: 'Legacy Fabric',
    babric: 'Babric',
}
