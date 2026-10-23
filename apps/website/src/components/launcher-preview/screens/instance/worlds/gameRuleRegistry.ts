import { defineMessages, type MessageDescriptor } from '@modrinth/ui/src/composables/i18n.ts'

/* Port of `apps/app-frontend/src/components/ui/world/gameRuleRegistry.ts`. */

export type GameRuleType = 'boolean' | 'integer'

export type GameRuleCategory = 'player' | 'mobs' | 'drops' | 'world' | 'chat' | 'commands' | 'other'

export type ResolvedGameRule = {
    name: MessageDescriptor
    /**
     * Reserved for a future localized explanation of the rule, rendered as a
     * tooltip next to the rule name once provided.
     */
    description?: MessageDescriptor
    category: Exclude<GameRuleCategory, 'other'>
    type: GameRuleType
    defaultValue?: string
}

/**
 * One conceptual game rule. `keys` lists every storage key the rule has had
 * across game versions (e.g. camelCase before 1.21.11, snake_case after), so
 * a plain rename shares one definition and one set of translations.
 * `overridesByKey` covers renames that changed semantics alongside the key,
 * such as a different default value under the new name.
 */
type GameRuleDefinition = ResolvedGameRule & {
    keys: readonly string[]
    overridesByKey?: Readonly<
        Record<string, Partial<Pick<ResolvedGameRule, 'name' | 'description' | 'defaultValue'>>>
    >
}

export const gameRuleCategoryMessages = defineMessages({
    player: {
        id: 'app.world-editor.gamerule-category.player',
        defaultMessage: 'Player',
    },
    mobs: {
        id: 'app.world-editor.gamerule-category.mobs',
        defaultMessage: 'Mobs',
    },
    drops: {
        id: 'app.world-editor.gamerule-category.drops',
        defaultMessage: 'Drops',
    },
    world: {
        id: 'app.world-editor.gamerule-category.world',
        defaultMessage: 'World updates',
    },
    chat: {
        id: 'app.world-editor.gamerule-category.chat',
        defaultMessage: 'Chat',
    },
    commands: {
        id: 'app.world-editor.gamerule-category.commands',
        defaultMessage: 'Commands',
    },
    other: {
        id: 'app.world-editor.gamerule-category.other',
        defaultMessage: 'Other',
    },
})

const ruleNames = defineMessages({
    announceAdvancements: {
        id: 'app.world-editor.gamerule.announceAdvancements',
        defaultMessage: 'Announce advancements',
    },
    allowEnteringNetherUsingPortals: {
        id: 'app.world-editor.gamerule.allow_entering_nether_using_portals',
        defaultMessage: 'Allow entering the Nether using portals',
    },
    allowFireTicksAwayFromPlayer: {
        id: 'app.world-editor.gamerule.allowFireTicksAwayFromPlayer',
        defaultMessage: 'Update fire and lava away from players',
    },
    blockExplosionDropDecay: {
        id: 'app.world-editor.gamerule.blockExplosionDropDecay',
        defaultMessage: 'Bed and respawn anchor explosions destroy some drops',
    },
    commandBlockOutput: {
        id: 'app.world-editor.gamerule.commandBlockOutput',
        defaultMessage: 'Broadcast command block output',
    },
    commandBlocksWork: {
        id: 'app.world-editor.gamerule.command_blocks_work',
        defaultMessage: 'Command blocks work',
    },
    commandModificationBlockLimit: {
        id: 'app.world-editor.gamerule.commandModificationBlockLimit',
        defaultMessage: 'Command modification block limit',
    },
    disableElytraMovementCheck: {
        id: 'app.world-editor.gamerule.disableElytraMovementCheck',
        defaultMessage: 'Disable elytra movement check',
    },
    disablePlayerMovementCheck: {
        id: 'app.world-editor.gamerule.disablePlayerMovementCheck',
        defaultMessage: 'Disable player movement check',
    },
    disableRaids: {
        id: 'app.world-editor.gamerule.disableRaids',
        defaultMessage: 'Disable raids',
    },
    doDaylightCycle: {
        id: 'app.world-editor.gamerule.doDaylightCycle',
        defaultMessage: 'Advance time of day',
    },
    doEntityDrops: {
        id: 'app.world-editor.gamerule.doEntityDrops',
        defaultMessage: 'Drops from non-mob entities',
    },
    doFireTick: {
        id: 'app.world-editor.gamerule.doFireTick',
        defaultMessage: 'Update fire',
    },
    doImmediateRespawn: {
        id: 'app.world-editor.gamerule.doImmediateRespawn',
        defaultMessage: 'Respawn immediately',
    },
    doInsomnia: {
        id: 'app.world-editor.gamerule.doInsomnia',
        defaultMessage: 'Spawn phantoms',
    },
    doLimitedCrafting: {
        id: 'app.world-editor.gamerule.doLimitedCrafting',
        defaultMessage: 'Require recipe for crafting',
    },
    doMobLoot: {
        id: 'app.world-editor.gamerule.doMobLoot',
        defaultMessage: 'Drop mob loot',
    },
    doMobSpawning: {
        id: 'app.world-editor.gamerule.doMobSpawning',
        defaultMessage: 'Spawn mobs',
    },
    doPatrolSpawning: {
        id: 'app.world-editor.gamerule.doPatrolSpawning',
        defaultMessage: 'Spawn pillager patrols',
    },
    doTileDrops: {
        id: 'app.world-editor.gamerule.doTileDrops',
        defaultMessage: 'Drop blocks',
    },
    doTraderSpawning: {
        id: 'app.world-editor.gamerule.doTraderSpawning',
        defaultMessage: 'Spawn wandering traders',
    },
    doVinesSpread: {
        id: 'app.world-editor.gamerule.doVinesSpread',
        defaultMessage: 'Vines spread',
    },
    doWardenSpawning: {
        id: 'app.world-editor.gamerule.doWardenSpawning',
        defaultMessage: 'Spawn wardens',
    },
    doWeatherCycle: {
        id: 'app.world-editor.gamerule.doWeatherCycle',
        defaultMessage: 'Update weather',
    },
    drowningDamage: {
        id: 'app.world-editor.gamerule.drowningDamage',
        defaultMessage: 'Deal drowning damage',
    },
    elytraMovementCheck: {
        id: 'app.world-editor.gamerule.elytra_movement_check',
        defaultMessage: 'Elytra movement check',
    },
    enderPearlsVanishOnDeath: {
        id: 'app.world-editor.gamerule.enderPearlsVanishOnDeath',
        defaultMessage: 'Thrown ender pearls vanish on death',
    },
    fallDamage: {
        id: 'app.world-editor.gamerule.fallDamage',
        defaultMessage: 'Deal fall damage',
    },
    fireDamage: {
        id: 'app.world-editor.gamerule.fireDamage',
        defaultMessage: 'Deal fire damage',
    },
    fireSpreadRadiusAroundPlayer: {
        id: 'app.world-editor.gamerule.fire_spread_radius_around_player',
        defaultMessage: 'Fire spread radius around players',
    },
    forgiveDeadPlayers: {
        id: 'app.world-editor.gamerule.forgiveDeadPlayers',
        defaultMessage: 'Forgive dead players',
    },
    freezeDamage: {
        id: 'app.world-editor.gamerule.freezeDamage',
        defaultMessage: 'Deal freeze damage',
    },
    globalSoundEvents: {
        id: 'app.world-editor.gamerule.globalSoundEvents',
        defaultMessage: 'Global sound events',
    },
    keepInventory: {
        id: 'app.world-editor.gamerule.keepInventory',
        defaultMessage: 'Keep inventory after death',
    },
    lavaSourceConversion: {
        id: 'app.world-editor.gamerule.lavaSourceConversion',
        defaultMessage: 'Allow lava to form new sources',
    },
    locatorBar: {
        id: 'app.world-editor.gamerule.locatorBar',
        defaultMessage: 'Show locator bar',
    },
    logAdminCommands: {
        id: 'app.world-editor.gamerule.logAdminCommands',
        defaultMessage: 'Broadcast admin commands',
    },
    maxCommandChainLength: {
        id: 'app.world-editor.gamerule.maxCommandChainLength',
        defaultMessage: 'Command chain size limit',
    },
    maxCommandForkCount: {
        id: 'app.world-editor.gamerule.maxCommandForkCount',
        defaultMessage: 'Command context limit',
    },
    maxEntityCramming: {
        id: 'app.world-editor.gamerule.maxEntityCramming',
        defaultMessage: 'Entity cramming threshold',
    },
    minecartMaxSpeed: {
        id: 'app.world-editor.gamerule.minecartMaxSpeed',
        defaultMessage: 'Minecart max speed',
    },
    mobExplosionDropDecay: {
        id: 'app.world-editor.gamerule.mobExplosionDropDecay',
        defaultMessage: 'Mob explosions destroy some drops',
    },
    mobGriefing: {
        id: 'app.world-editor.gamerule.mobGriefing',
        defaultMessage: 'Allow destructive mob actions',
    },
    naturalRegeneration: {
        id: 'app.world-editor.gamerule.naturalRegeneration',
        defaultMessage: 'Regenerate health naturally',
    },
    playerMovementCheck: {
        id: 'app.world-editor.gamerule.player_movement_check',
        defaultMessage: 'Player movement check',
    },
    playersNetherPortalCreativeDelay: {
        id: 'app.world-editor.gamerule.playersNetherPortalCreativeDelay',
        defaultMessage: 'Nether portal delay in creative mode',
    },
    playersNetherPortalDefaultDelay: {
        id: 'app.world-editor.gamerule.playersNetherPortalDefaultDelay',
        defaultMessage: 'Nether portal delay in non-creative mode',
    },
    playersSleepingPercentage: {
        id: 'app.world-editor.gamerule.playersSleepingPercentage',
        defaultMessage: 'Sleep percentage',
    },
    projectilesCanBreakBlocks: {
        id: 'app.world-editor.gamerule.projectilesCanBreakBlocks',
        defaultMessage: 'Projectiles can break blocks',
    },
    pvp: {
        id: 'app.world-editor.gamerule.pvp',
        defaultMessage: 'Player vs. player damage (PvP)',
    },
    raids: {
        id: 'app.world-editor.gamerule.raids',
        defaultMessage: 'Enable raids',
    },
    randomTickSpeed: {
        id: 'app.world-editor.gamerule.randomTickSpeed',
        defaultMessage: 'Random tick speed rate',
    },
    reducedDebugInfo: {
        id: 'app.world-editor.gamerule.reducedDebugInfo',
        defaultMessage: 'Reduce debug info',
    },
    sendCommandFeedback: {
        id: 'app.world-editor.gamerule.sendCommandFeedback',
        defaultMessage: 'Send command feedback',
    },
    showDeathMessages: {
        id: 'app.world-editor.gamerule.showDeathMessages',
        defaultMessage: 'Show death messages',
    },
    snowAccumulationHeight: {
        id: 'app.world-editor.gamerule.snowAccumulationHeight',
        defaultMessage: 'Snow accumulation height',
    },
    spawnChunkRadius: {
        id: 'app.world-editor.gamerule.spawnChunkRadius',
        defaultMessage: 'Spawn chunk radius',
    },
    spawnMonsters: {
        id: 'app.world-editor.gamerule.spawn_monsters',
        defaultMessage: 'Spawn monsters',
    },
    spawnRadius: {
        id: 'app.world-editor.gamerule.spawnRadius',
        defaultMessage: 'Respawn location radius',
    },
    spawnerBlocksWork: {
        id: 'app.world-editor.gamerule.spawner_blocks_work',
        defaultMessage: 'Spawner blocks work',
    },
    spectatorsGenerateChunks: {
        id: 'app.world-editor.gamerule.spectatorsGenerateChunks',
        defaultMessage: 'Allow spectators to generate terrain',
    },
    tntExplodes: {
        id: 'app.world-editor.gamerule.tntExplodes',
        defaultMessage: 'Allow TNT to explode',
    },
    tntExplosionDropDecay: {
        id: 'app.world-editor.gamerule.tntExplosionDropDecay',
        defaultMessage: 'TNT explosions destroy some drops',
    },
    universalAnger: {
        id: 'app.world-editor.gamerule.universalAnger',
        defaultMessage: 'Universal anger',
    },
    waterSourceConversion: {
        id: 'app.world-editor.gamerule.waterSourceConversion',
        defaultMessage: 'Allow water to form new sources',
    },
})

/**
 * Display metadata for known game rules, one definition per rule concept.
 * The set of rules shown in the editor always comes from the world's own
 * storage: rules present in the world but missing here fall back to their
 * raw key under the "Other" category, and rules listed here but absent from
 * the world are not rendered.
 */
const definitions: readonly GameRuleDefinition[] = [
    {
        keys: ['disableElytraMovementCheck'],
        name: ruleNames.disableElytraMovementCheck,
        category: 'player',
        type: 'boolean',
        defaultValue: 'false',
    },
    {
        keys: ['elytra_movement_check'],
        name: ruleNames.elytraMovementCheck,
        category: 'player',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['disablePlayerMovementCheck'],
        name: ruleNames.disablePlayerMovementCheck,
        category: 'player',
        type: 'boolean',
        defaultValue: 'false',
    },
    {
        keys: ['player_movement_check'],
        name: ruleNames.playerMovementCheck,
        category: 'player',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['doImmediateRespawn', 'immediate_respawn'],
        name: ruleNames.doImmediateRespawn,
        category: 'player',
        type: 'boolean',
        defaultValue: 'false',
    },
    {
        keys: ['doLimitedCrafting', 'limited_crafting'],
        name: ruleNames.doLimitedCrafting,
        category: 'player',
        type: 'boolean',
        defaultValue: 'false',
    },
    {
        keys: ['drowningDamage', 'drowning_damage'],
        name: ruleNames.drowningDamage,
        category: 'player',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['enderPearlsVanishOnDeath', 'ender_pearls_vanish_on_death'],
        name: ruleNames.enderPearlsVanishOnDeath,
        category: 'player',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['fallDamage', 'fall_damage'],
        name: ruleNames.fallDamage,
        category: 'player',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['fireDamage', 'fire_damage'],
        name: ruleNames.fireDamage,
        category: 'player',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['freezeDamage', 'freeze_damage'],
        name: ruleNames.freezeDamage,
        category: 'player',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['keepInventory', 'keep_inventory'],
        name: ruleNames.keepInventory,
        category: 'player',
        type: 'boolean',
        defaultValue: 'false',
    },
    {
        keys: ['naturalRegeneration', 'natural_health_regeneration'],
        name: ruleNames.naturalRegeneration,
        category: 'player',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['playersNetherPortalCreativeDelay', 'players_nether_portal_creative_delay'],
        name: ruleNames.playersNetherPortalCreativeDelay,
        category: 'player',
        type: 'integer',
        defaultValue: '1',
        overridesByKey: {
            players_nether_portal_creative_delay: { defaultValue: '0' },
        },
    },
    {
        keys: ['playersNetherPortalDefaultDelay', 'players_nether_portal_default_delay'],
        name: ruleNames.playersNetherPortalDefaultDelay,
        category: 'player',
        type: 'integer',
        defaultValue: '80',
    },
    {
        keys: ['playersSleepingPercentage', 'players_sleeping_percentage'],
        name: ruleNames.playersSleepingPercentage,
        category: 'player',
        type: 'integer',
        defaultValue: '100',
    },
    {
        keys: ['pvp'],
        name: ruleNames.pvp,
        category: 'player',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['spawnRadius', 'respawn_radius'],
        name: ruleNames.spawnRadius,
        category: 'player',
        type: 'integer',
        defaultValue: '10',
    },
    {
        keys: ['spectatorsGenerateChunks', 'spectators_generate_chunks'],
        name: ruleNames.spectatorsGenerateChunks,
        category: 'player',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['disableRaids'],
        name: ruleNames.disableRaids,
        category: 'mobs',
        type: 'boolean',
        defaultValue: 'false',
    },
    {
        keys: ['raids'],
        name: ruleNames.raids,
        category: 'mobs',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['doInsomnia', 'spawn_phantoms'],
        name: ruleNames.doInsomnia,
        category: 'mobs',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['doMobSpawning', 'spawn_mobs'],
        name: ruleNames.doMobSpawning,
        category: 'mobs',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['spawn_monsters'],
        name: ruleNames.spawnMonsters,
        category: 'mobs',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['spawner_blocks_work'],
        name: ruleNames.spawnerBlocksWork,
        category: 'mobs',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['doPatrolSpawning', 'spawn_patrols'],
        name: ruleNames.doPatrolSpawning,
        category: 'mobs',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['doTraderSpawning', 'spawn_wandering_traders'],
        name: ruleNames.doTraderSpawning,
        category: 'mobs',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['doWardenSpawning', 'spawn_wardens'],
        name: ruleNames.doWardenSpawning,
        category: 'mobs',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['forgiveDeadPlayers', 'forgive_dead_players'],
        name: ruleNames.forgiveDeadPlayers,
        category: 'mobs',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['maxEntityCramming', 'max_entity_cramming'],
        name: ruleNames.maxEntityCramming,
        category: 'mobs',
        type: 'integer',
        defaultValue: '24',
    },
    {
        keys: ['mobGriefing', 'mob_griefing'],
        name: ruleNames.mobGriefing,
        category: 'mobs',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['universalAnger', 'universal_anger'],
        name: ruleNames.universalAnger,
        category: 'mobs',
        type: 'boolean',
        defaultValue: 'false',
    },
    {
        keys: ['blockExplosionDropDecay', 'block_explosion_drop_decay'],
        name: ruleNames.blockExplosionDropDecay,
        category: 'drops',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['doEntityDrops', 'entity_drops'],
        name: ruleNames.doEntityDrops,
        category: 'drops',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['doMobLoot', 'mob_drops'],
        name: ruleNames.doMobLoot,
        category: 'drops',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['doTileDrops', 'block_drops'],
        name: ruleNames.doTileDrops,
        category: 'drops',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['mobExplosionDropDecay', 'mob_explosion_drop_decay'],
        name: ruleNames.mobExplosionDropDecay,
        category: 'drops',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['projectilesCanBreakBlocks', 'projectiles_can_break_blocks'],
        name: ruleNames.projectilesCanBreakBlocks,
        category: 'drops',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['tntExplosionDropDecay', 'tnt_explosion_drop_decay'],
        name: ruleNames.tntExplosionDropDecay,
        category: 'drops',
        type: 'boolean',
        defaultValue: 'false',
    },
    {
        keys: ['allowFireTicksAwayFromPlayer'],
        name: ruleNames.allowFireTicksAwayFromPlayer,
        category: 'world',
        type: 'boolean',
        defaultValue: 'false',
    },
    {
        keys: ['allow_entering_nether_using_portals'],
        name: ruleNames.allowEnteringNetherUsingPortals,
        category: 'world',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['doDaylightCycle', 'advance_time'],
        name: ruleNames.doDaylightCycle,
        category: 'world',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['doFireTick'],
        name: ruleNames.doFireTick,
        category: 'world',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['fire_spread_radius_around_player'],
        name: ruleNames.fireSpreadRadiusAroundPlayer,
        category: 'world',
        type: 'integer',
        defaultValue: '128',
    },
    {
        keys: ['doVinesSpread', 'spread_vines'],
        name: ruleNames.doVinesSpread,
        category: 'world',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['doWeatherCycle', 'advance_weather'],
        name: ruleNames.doWeatherCycle,
        category: 'world',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['lavaSourceConversion', 'lava_source_conversion'],
        name: ruleNames.lavaSourceConversion,
        category: 'world',
        type: 'boolean',
        defaultValue: 'false',
    },
    {
        keys: ['minecartMaxSpeed', 'max_minecart_speed'],
        name: ruleNames.minecartMaxSpeed,
        category: 'world',
        type: 'integer',
        defaultValue: '8',
    },
    {
        keys: ['randomTickSpeed', 'random_tick_speed'],
        name: ruleNames.randomTickSpeed,
        category: 'world',
        type: 'integer',
        defaultValue: '3',
    },
    {
        keys: ['snowAccumulationHeight', 'max_snow_accumulation_height'],
        name: ruleNames.snowAccumulationHeight,
        category: 'world',
        type: 'integer',
        defaultValue: '1',
    },
    {
        keys: ['spawnChunkRadius'],
        name: ruleNames.spawnChunkRadius,
        category: 'world',
        type: 'integer',
        defaultValue: '2',
    },
    {
        keys: ['tntExplodes', 'tnt_explodes'],
        name: ruleNames.tntExplodes,
        category: 'world',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['waterSourceConversion', 'water_source_conversion'],
        name: ruleNames.waterSourceConversion,
        category: 'world',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['announceAdvancements', 'show_advancement_messages'],
        name: ruleNames.announceAdvancements,
        category: 'chat',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['commandBlockOutput', 'command_block_output'],
        name: ruleNames.commandBlockOutput,
        category: 'chat',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['logAdminCommands', 'log_admin_commands'],
        name: ruleNames.logAdminCommands,
        category: 'chat',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['sendCommandFeedback', 'send_command_feedback'],
        name: ruleNames.sendCommandFeedback,
        category: 'chat',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['showDeathMessages', 'show_death_messages'],
        name: ruleNames.showDeathMessages,
        category: 'chat',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['command_blocks_work'],
        name: ruleNames.commandBlocksWork,
        category: 'commands',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['commandModificationBlockLimit', 'max_block_modifications'],
        name: ruleNames.commandModificationBlockLimit,
        category: 'commands',
        type: 'integer',
        defaultValue: '32768',
    },
    {
        keys: ['globalSoundEvents', 'global_sound_events'],
        name: ruleNames.globalSoundEvents,
        category: 'commands',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['locatorBar', 'locator_bar'],
        name: ruleNames.locatorBar,
        category: 'commands',
        type: 'boolean',
        defaultValue: 'true',
    },
    {
        keys: ['maxCommandChainLength', 'max_command_sequence_length'],
        name: ruleNames.maxCommandChainLength,
        category: 'commands',
        type: 'integer',
        defaultValue: '65536',
    },
    {
        keys: ['maxCommandForkCount', 'max_command_forks'],
        name: ruleNames.maxCommandForkCount,
        category: 'commands',
        type: 'integer',
        defaultValue: '65536',
    },
    {
        keys: ['reducedDebugInfo', 'reduced_debug_info'],
        name: ruleNames.reducedDebugInfo,
        category: 'commands',
        type: 'boolean',
        defaultValue: 'false',
    },
]

const lookup = new Map<string, ResolvedGameRule>()
for (const { keys, overridesByKey, ...base } of definitions) {
    for (const key of keys) {
        lookup.set(key, { ...base, ...overridesByKey?.[key] })
    }
}

const MINECRAFT_NAMESPACE = 'minecraft:'

/**
 * Strips the `minecraft:` namespace that split game_rules.dat storage adds
 * to vanilla rule keys. Other namespaces (modded rules) are kept as-is so
 * they fall back to their raw key.
 */
export function normalizeGameRuleKey(key: string): string {
    return key.startsWith(MINECRAFT_NAMESPACE) ? key.slice(MINECRAFT_NAMESPACE.length) : key
}

export function getGameRuleMetadata(key: string): ResolvedGameRule | undefined {
    return lookup.get(normalizeGameRuleKey(key))
}

export function getGameRuleCategory(key: string): GameRuleCategory {
    return getGameRuleMetadata(key)?.category ?? 'other'
}

const BOOLEAN_VALUES = new Set(['true', 'false'])

/**
 * Resolves the editor widget for a rule from its current stored value, so a
 * corrupted value degrades to a plain text input instead of a lying toggle,
 * and unknown modded rules still get a fitting widget.
 */
export function resolveGameRuleType(value: string): GameRuleType | 'text' {
    if (BOOLEAN_VALUES.has(value)) {
        return 'boolean'
    }
    if (/^[+-]?\d+$/.test(value.trim())) {
        return 'integer'
    }
    return 'text'
}

/**
 * The rule definitions, which the preview reads to write a world's game rules
 * as the game stores them (the app reads them from `level.dat` instead).
 */
export const gameRuleDefinitions: readonly { keys: readonly string[] }[] = definitions
