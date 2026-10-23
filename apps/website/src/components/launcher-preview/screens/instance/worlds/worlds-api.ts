import { revealInFolder } from '../../../lib/platform/opener'
import { currentPreview, previewDomain } from '../../../lib/state'
import {
    getWorldIdentifier,
    type HomeWorld,
    normalizeServerAddress,
    type ServerWorld as HomeServerWorld,
    type SingleplayerWorld as HomeSingleplayerWorld,
} from '../../home/home-utils'
import { createHomeState, type HomeState } from '../../home/state'

/*
 * The app's `helpers/worlds.ts` for the instance's Worlds tab, over the world
 * list the Home screen keeps for every instance (`screens/home/state.ts`), so
 * a server added or a world deleted here shows on Home too.
 */

export type World = HomeWorld
export type ServerWorld = HomeServerWorld
export type SingleplayerWorld = HomeSingleplayerWorld
export type DisplayStatus = HomeWorld['display_status']
export type ServerPackStatus = HomeServerWorld['pack_status']

export { getWorldIdentifier, normalizeServerAddress }
export { hasServerQuickPlaySupport, hasWorldQuickPlaySupport } from '../../home/home-utils'

function home(): HomeState {
    return previewDomain(currentPreview(), 'home', createHomeState)
}

function reindexServers(worlds: HomeWorld[], instanceId: string): HomeWorld[] {
    let index = 0
    return worlds.map((world) =>
        world.instance_id === instanceId && world.type === 'server'
            ? { ...world, index: index, path: `servers.dat#${index++}` }
            : world,
    )
}

/** `add_server_to_instance`: appends the entry to the instance's `servers.dat`. */
export async function add_server_to_instance(
    instanceId: string,
    name: string,
    address: string,
    packStatus: ServerPackStatus,
): Promise<number> {
    const state = home()
    await state.delay(120)
    const index = state.worlds.value.filter(
        (world) => world.instance_id === instanceId && world.type === 'server',
    ).length
    state.worlds.value = [
        ...state.worlds.value,
        {
            type: 'server',
            instance_id: instanceId,
            name,
            address,
            index,
            path: `servers.dat#${index}`,
            pack_status: packStatus,
            icon: null,
            display_status: 'normal',
        },
    ]
    return index
}

/** `edit_server_in_instance`: rewrites the entry at `index`. */
export async function edit_server_in_instance(
    instanceId: string,
    index: number,
    name: string,
    address: string,
    packStatus: ServerPackStatus,
): Promise<void> {
    const state = home()
    await state.delay(120)
    state.worlds.value = state.worlds.value.map((world) =>
        world.instance_id === instanceId && world.type === 'server' && world.index === index
            ? { ...world, name, address, pack_status: packStatus }
            : world,
    )
}

/** `remove_server_from_instance`: drops the entry; later entries move up. */
export async function remove_server_from_instance(instanceId: string, index: number) {
    const state = home()
    await state.delay(120)
    state.worlds.value = reindexServers(
        state.worlds.value.filter(
            (world) =>
                !(
                    world.instance_id === instanceId &&
                    world.type === 'server' &&
                    world.index === index
                ),
        ),
        instanceId,
    )
}

/** `delete_world`: removes the save folder. */
export async function delete_world(instanceId: string, path: string) {
    const state = home()
    await state.delay(180)
    state.worlds.value = state.worlds.value.filter(
        (world) =>
            !(
                world.instance_id === instanceId &&
                world.type === 'singleplayer' &&
                world.path === path
            ),
    )
}

/** `refreshWorlds`: the instance's saves and `servers.dat`, newest played first. */
export async function refreshWorlds(instanceId: string): Promise<World[]> {
    const worlds = [...(await home().getInstanceWorlds(instanceId, true))]
    sortWorlds(worlds)
    return worlds
}

export function sortWorlds(worlds: World[]) {
    worlds.sort((a, b) => {
        if (!a.last_played) return 1
        if (!b.last_played) return -1
        return Date.parse(b.last_played) - Date.parse(a.last_played)
    })
}

/** `showWorldInFolder`: Finder belongs to the desktop app. */
export function showWorldInFolder(_instanceId: string, worldPath: string) {
    revealInFolder(`saves/${worldPath}`)
}

function isIPv4Host(host: string): boolean {
    const segments = host.split('.')
    if (segments.length !== 4) return false
    return segments.every((segment) => {
        if (!/^\d+$/.test(segment)) return false
        const value = Number.parseInt(segment, 10)
        return value >= 0 && value <= 255
    })
}

/**
 * Domain key used for deduping server entries by removing a single leading subdomain.
 * Example: test.cobblemon.gg and cobblemon.gg map to cobblemon.gg
 */
export function getServerDomainKey(address: string): string {
    const normalizedAddress = normalizeServerAddress(address)
    if (!normalizedAddress) return ''

    const separator = normalizedAddress.lastIndexOf(':')
    if (separator <= 0 || separator === normalizedAddress.length - 1) return normalizedAddress

    const host = normalizedAddress.slice(0, separator).replace(/\.+$/, '')
    if (!host) return normalizedAddress
    if (host.includes(':') || isIPv4Host(host)) return normalizedAddress

    const segments = host.split('.').filter(Boolean)
    if (segments.length <= 2) return host

    return segments.slice(1).join('.')
}

export function resolveManagedServerWorld(
    worlds: World[],
    managedName: string | null | undefined,
    managedAddress: string | null | undefined,
): ServerWorld | null {
    if (!managedName || !managedAddress) return null

    const normalizedManagedAddress = normalizeServerAddress(managedAddress)
    if (!normalizedManagedAddress) return null

    const servers = worlds
        .filter((world): world is ServerWorld => world.type === 'server')
        .slice()
        .sort((a, b) => a.index - b.index)

    const exactMatch = servers.find(
        (server) =>
            server.name === managedName &&
            normalizeServerAddress(server.address) === normalizedManagedAddress,
    )
    if (exactMatch) return exactMatch

    return (
        servers.find(
            (server) => normalizeServerAddress(server.address) === normalizedManagedAddress,
        ) ?? null
    )
}

/* ------------------------------------------------------------ level.dat */

export type SingleplayerGameMode = SingleplayerWorld['game_mode']
export type WorldDifficulty = 'peaceful' | 'easy' | 'normal' | 'hard'

export type GameRuleEntry = {
    key: string
    value: string
}

export type WorldLevelData = {
    name: string
    icon?: string
    game_mode: SingleplayerGameMode
    difficulty?: WorldDifficulty
    difficulty_locked: boolean
    hardcore: boolean
    allow_commands?: boolean
    /** The world seed as a base-10 string, preserving the full i64 range. */
    seed?: string
    game_rules: GameRuleEntry[]
    version_name?: string
    last_played?: string
    modded: boolean
    locked: boolean
}

export type WorldSettingsPatch = {
    name?: string
    game_mode?: SingleplayerGameMode
    difficulty?: WorldDifficulty
    allow_commands?: boolean
    seed?: string
    game_rules?: GameRuleEntry[]
}

type StoredLevel = Pick<WorldLevelData, 'difficulty' | 'allow_commands' | 'seed' | 'game_rules'>

/** `level.dat` fields the world list does not carry, per `<instance>:<save folder>`. */
function levelStore() {
    return previewDomain(currentPreview(), 'instance:world-levels', () => ({
        value: new Map<string, StoredLevel>(),
    }))
}

/** Game versions from 1.21.11 on store rules under snake_case `minecraft:` keys. */
function usesNamespacedRules(gameVersion: string) {
    const [major, minor = 0, patch = 0] = gameVersion.split(/[.-]/).map((part) => Number(part) || 0)
    if (major > 1) return true
    return minor > 21 || (minor === 21 && patch >= 11)
}

/** A new world's rules: every vanilla rule of its version at its default value. */
async function defaultGameRules(gameVersion: string): Promise<GameRuleEntry[]> {
    const { gameRuleDefinitions, getGameRuleMetadata } = await import('./gameRuleRegistry')
    const namespaced = usesNamespacedRules(gameVersion)
    const rules: GameRuleEntry[] = []
    for (const definition of gameRuleDefinitions) {
        const key = namespaced
            ? definition.keys.find((candidate) => candidate.includes('_'))
            : definition.keys.find((candidate) => !candidate.includes('_'))
        const defaultValue = key ? getGameRuleMetadata(key)?.defaultValue : undefined
        if (!key || defaultValue === undefined) continue
        rules.push({ key: namespaced ? `minecraft:${key}` : key, value: defaultValue })
    }
    return rules
}

/** A stable 64-bit seed for a save the preview did not record one for. */
function worldSeed(instanceId: string, path: string): string {
    let hash = 0xcbf29ce484222325n
    for (const char of `${instanceId}/${path}`) {
        hash ^= BigInt(char.codePointAt(0) ?? 0)
        hash = (hash * 0x100000001b3n) & 0xffffffffffffffffn
    }
    return BigInt.asIntN(64, hash).toString()
}

function findSave(instanceId: string, path: string): SingleplayerWorld | undefined {
    return home().worlds.value.find(
        (world): world is SingleplayerWorld =>
            world.instance_id === instanceId &&
            world.type === 'singleplayer' &&
            world.path === path,
    )
}

/** `get_world_level_data`: reads the save's `level.dat` and game rules. */
export async function get_world_level_data(
    instanceId: string,
    path: string,
): Promise<WorldLevelData> {
    const state = home()
    await state.delay(140)
    const world = findSave(instanceId, path)
    const instance = currentPreview().instances.get(instanceId)
    if (!world || !instance) throw new Error(`No world named "${path}" in this instance`)
    const store = levelStore()
    const key = `${instanceId}:${path}`
    let level = store.get(key)
    if (!level) {
        level = {
            difficulty: world.hardcore ? 'hard' : 'normal',
            allow_commands: world.game_mode === 'creative',
            seed: worldSeed(instanceId, path),
            game_rules: await defaultGameRules(instance.game_version),
        }
        store.set(key, level)
    }
    return {
        name: world.name,
        icon: world.icon ?? undefined,
        game_mode: world.game_mode,
        difficulty: level.difficulty,
        difficulty_locked: false,
        hardcore: world.hardcore,
        allow_commands: level.allow_commands,
        seed: level.seed,
        game_rules: level.game_rules.map((rule) => ({ ...rule })),
        version_name: instance.game_version,
        last_played: world.last_played,
        modded: instance.loader !== 'vanilla',
        locked: world.locked,
    }
}

/** `update_world_settings`: writes the patch into `level.dat` and the game rules. */
export async function update_world_settings(
    instanceId: string,
    path: string,
    patch: WorldSettingsPatch,
) {
    const state = home()
    await state.delay(220)
    const world = findSave(instanceId, path)
    if (!world) throw new Error(`No world named "${path}" in this instance`)
    if (world.locked) throw new Error('The world is open in Minecraft')
    const store = levelStore()
    const key = `${instanceId}:${path}`
    const level = store.get(key)
    if (level) {
        if (patch.difficulty) level.difficulty = patch.difficulty
        if (patch.allow_commands !== undefined) level.allow_commands = patch.allow_commands
        if (patch.seed !== undefined) level.seed = patch.seed
        for (const rule of patch.game_rules ?? []) {
            const existing = level.game_rules.find((entry) => entry.key === rule.key)
            if (existing) existing.value = rule.value
            else level.game_rules.push({ ...rule })
        }
    }
    state.worlds.value = state.worlds.value.map((entry) =>
        entry === world
            ? {
                  ...world,
                  name: patch.name ?? world.name,
                  game_mode: patch.game_mode ?? world.game_mode,
              }
            : entry,
    )
}

/** `reset_world_icon`: deletes the save's `icon.png`. */
export async function reset_world_icon(instanceId: string, path: string) {
    const state = home()
    await state.delay(120)
    state.worlds.value = state.worlds.value.map((world) =>
        world.instance_id === instanceId && world.type === 'singleplayer' && world.path === path
            ? { ...world, icon: null }
            : world,
    )
}
