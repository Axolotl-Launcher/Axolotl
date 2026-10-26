import type { GameInstance, InstanceLoader } from '@/helpers/types'

import modpacks from '../data/modpacks.json'
import type { Handlers } from './types'

const MINUTE = 60 * 1000
const HOUR = 60 * MINUTE
const DAY = 24 * HOUR

/** Where app-lib keeps instances on macOS (packages/app-lib/src/state/dirs.rs). */
const PROFILES_DIR = '/Users/steve/Library/Application Support/red.ghs.axolotl/profiles'

const icons = import.meta.glob<string>('../assets/projects/*.webp', {
    eager: true,
    query: '?url',
    import: 'default',
})

function projectIcon(projectId: string) {
    return icons[`../assets/projects/${projectId}.webp`]
}

function ago(offsetMs: number) {
    return new Date(Date.now() - offsetMs)
}

function instance(
    id: string,
    name: string,
    loader: InstanceLoader,
    loaderVersion: string | null,
    gameVersion: string,
    icon: string | undefined,
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
        icon_path: icon,
        synced_options: {
            game_options: false,
            command_history: false,
            multiplayer_servers: false,
            creative_hotbars: false,
            screenshots: false,
            resource_packs: false,
            data_packs: false,
        },
        game_version: gameVersion,
        loader,
        loader_version: loaderVersion ?? undefined,
        loader_components: loaderVersion
            ? ([
                  { kind: loader, version: loaderVersion, role: 'primary' },
              ] as unknown as GameInstance['loader_components'])
            : [],
        groups: [],
        link: null,
        update_channel: 'release' as GameInstance['update_channel'],
        created: ago(createdAgo),
        modified: ago(playedAgo),
        last_played: ago(playedAgo),
        submitted_time_played: timePlayed / 1000,
        recent_time_played: 0,
        java_path: null,
        extra_launch_args: [],
        memory: null,
        hooks: {} as GameInstance['hooks'],
        ...extra,
    }
}

/** An instance installed from a recorded modpack version (data/modpacks.json). */
function modpackInstance(
    id: string,
    projectId: keyof typeof modpacks.packs,
    playedAgo: number,
    timePlayed: number,
    createdAgo: number,
): GameInstance {
    const pack = modpacks.packs[projectId]
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
        {
            install_stage: 'pack_installed',
            link: {
                type: 'modrinth_modpack',
                project_id: projectId,
                version_id: pack.version_id,
            } as unknown as GameInstance['link'],
        },
    )
}

const instances: GameInstance[] = [
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
        undefined,
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
        undefined,
        16 * DAY,
        41 * MINUTE,
        150 * DAY,
    ),
]

/** The last session of each played instance, for the home dashboard's playtime. */
export function instancePlaySessions() {
    return instances
        .filter((entry) => entry.last_played && entry.submitted_time_played > 0)
        .map((entry) => ({
            instance: entry,
            at: new Date(entry.last_played!),
            seconds: Math.min(entry.submitted_time_played, 2 * 3600),
        }))
}

function byId(id: unknown) {
    return instances.find((entry) => entry.id === id) ?? null
}

export const instanceHandlers: Handlers = {
    'plugin:instance|instance_list': () => structuredClone(instances),
    'plugin:instance|instance_get': ({ instanceId }) => structuredClone(byId(instanceId)),
    'plugin:instance|instance_get_many': ({ instanceIds }) =>
        structuredClone((instanceIds as string[]).map(byId).filter(Boolean)),
    'plugin:instance|instance_list_backup_operations': () => [],
}
