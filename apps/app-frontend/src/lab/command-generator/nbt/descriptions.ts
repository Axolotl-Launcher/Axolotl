/**
 * 物品组件的说明文案，键是目录里的字段路径（子字段用点连接）。
 *
 * 目录数据里的 `desc` 是 wiki 英文原文（搜索也会用到），但界面要给两种语言，
 * 所以显示走这里的消息描述符。id 必须写成字面量，formatjs 才能静态抽取。
 * 文案里的尖括号与花括号是字面量，按 ICU 规则用单引号转义。
 */

import { defineMessages } from '@modrinth/ui'

export const ITEM_DESCRIPTIONS = defineMessages({
    additional_trade_cost: {
        id: 'app.lab.command-generator.nbt.desc.item.additional_trade_cost',
        defaultMessage:
            'Used on the gives item of a villager trade to add to the count of the wants item.',
    },
    attack_animation: {
        id: 'app.lab.command-generator.nbt.desc.item.attack_animation',
        defaultMessage: 'The swing animation to play when the item is used to attack something',
    },
    attack_range: {
        id: 'app.lab.command-generator.nbt.desc.item.attack_range',
        defaultMessage: 'Determines the attack range and hitbox margin of a weapon.',
    },
    attribute_modifiers: {
        id: 'app.lab.command-generator.nbt.desc.item.attribute_modifiers',
        defaultMessage:
            "A list of attribute modifiers which, if present on an item, may be applied to a player or mob that has equipped the item. If the item is not in the correct equipment slot, it has no effect. The modifiers are removed from the entity once the item is moved out of that slot or is deleted. Any attribute modifiers specified are listed in the item's tooltip (unless hidden).",
    },
    banner_patterns: {
        id: 'app.lab.command-generator.nbt.desc.item.banner_patterns',
        defaultMessage:
            'List of all patterns applied to the banner or the shield. If used on a shield and the minecraft:base_color component is not specified, then white is used as the base banner color.',
    },
    'banner_patterns.color': {
        id: 'app.lab.command-generator.nbt.desc.item.banner_patterns.color',
        defaultMessage: 'Dye color of the section.',
    },
    'banner_patterns.pattern': {
        id: 'app.lab.command-generator.nbt.desc.item.banner_patterns.pattern',
        defaultMessage: 'One banner pattern (an ID, or a new banner pattern definition).',
    },
    'banner_patterns.pattern.asset_id': {
        id: 'app.lab.command-generator.nbt.desc.item.banner_patterns.pattern.asset_id',
        defaultMessage: 'The resource location for the texture asset.',
    },
    'banner_patterns.pattern.translation_key': {
        id: 'app.lab.command-generator.nbt.desc.item.banner_patterns.pattern.translation_key',
        defaultMessage: 'The translation key for displaying the banner tooltip.',
    },
    base_color: {
        id: 'app.lab.command-generator.nbt.desc.item.base_color',
        defaultMessage:
            'The base dye color of the banner applied on a shield. If neither this nor minecraft:banner_patterns is specified, then the shield model will show its normal wooden texture. If the minecraft:banner_patterns component is specified but this component is not, then white is used as the base banner color. If present on a shield item, the item name is overridden as "\'<color>\' Shield" (e.g. if set to green, the name will be "Green Shield").',
    },
    bees: {
        id: 'app.lab.command-generator.nbt.desc.item.bees',
        defaultMessage:
            'A list of the entities currently in the beehive or bee nest. If present on an item, text is added to the item\'s tooltip that says how many entries are in the list (e.g. if there are two entries, the tooltip will say "Bees: 2 / 3") and each item is considered a full stack for the minecraft:bundle_contents components. Despite the tooltip suggesting there is a maximum of 3 entries, there is no limit to the size of this component.',
    },
    'bees.entity_data': {
        id: 'app.lab.command-generator.nbt.desc.item.bees.entity_data',
        defaultMessage:
            'Must include id tag. If the id is not minecraft:bee then the entity will never leave the hive. The Passengers, Pos, Rotation, and UUID tags are ignored when the bee spawns (the bee will always spawn in front of the hive with a random UUID, facing south, and without any passengers). May also be written as a plain string holding the entity ID.',
    },
    'bees.entity_data.Air': {
        id: 'app.lab.command-generator.nbt.desc.item.bees.entity_data.Air',
        defaultMessage:
            'How much air the entity has, in game ticks. Decreases when unable to breathe (except suffocating in a block). Increases when it can breathe. Being at or below -20 game ticks (while still unable to breathe) on a given game tick causes the entity to immediately lose 1 health to drowning damage. This resets to 0 game ticks. Most mobs can have a maximum of 300 game ticks (15 seconds) of air, while dolphins can reach up to 4800 game ticks (240 seconds), and axolotls have 6000 game ticks (300 seconds).',
    },
    'bees.entity_data.CustomName': {
        id: 'app.lab.command-generator.nbt.desc.item.bees.entity_data.CustomName',
        defaultMessage:
            "The custom name text component of this entity. Appears in player death messages and villager trading interfaces, as well as above the entity when the player's cursor is over it. May be empty or not exist. Represents the minecraft:custom_name component.",
    },
    'bees.entity_data.CustomNameVisible': {
        id: 'app.lab.command-generator.nbt.desc.item.bees.entity_data.CustomNameVisible',
        defaultMessage:
            '1 or 0 (true/false) - if true, and this entity has a custom name, the name always appears above the entity, regardless of where the cursor points. If the entity does not have a custom name, a default name is shown. May not exist.',
    },
    'bees.entity_data.data': {
        id: 'app.lab.command-generator.nbt.desc.item.bees.entity_data.data',
        defaultMessage:
            'Optional arbitrary NBT data. Is removed if empty. Represents the minecraft:custom_data component.',
    },
    'bees.entity_data.fall_distance': {
        id: 'app.lab.command-generator.nbt.desc.item.bees.entity_data.fall_distance',
        defaultMessage:
            'Distance the entity has fallen. Larger values cause more damage when the entity lands.',
    },
    'bees.entity_data.Fire': {
        id: 'app.lab.command-generator.nbt.desc.item.bees.entity_data.Fire',
        defaultMessage:
            'Number of game ticks until the fire is put out. Negative values reflect how long the entity can stand in fire before burning. Default to -20 when not on fire.',
    },
    'bees.entity_data.Glowing': {
        id: 'app.lab.command-generator.nbt.desc.item.bees.entity_data.Glowing',
        defaultMessage: '1 or 0 (true/false) - if true, the entity has a glowing outline.',
    },
    'bees.entity_data.HasVisualFire': {
        id: 'app.lab.command-generator.nbt.desc.item.bees.entity_data.HasVisualFire',
        defaultMessage:
            '1 or 0 (true/false) - if true, the entity visually appears on fire, even if it is not actually on fire.',
    },
    'bees.entity_data.id': {
        id: 'app.lab.command-generator.nbt.desc.item.bees.entity_data.id',
        defaultMessage:
            "Namespaced ID of the entity's entity type. Does not exist for entities in the world, only stored entities (such as in data components).",
    },
    'bees.entity_data.Invulnerable': {
        id: 'app.lab.command-generator.nbt.desc.item.bees.entity_data.Invulnerable',
        defaultMessage:
            '1 or 0 (true/false) - if true, the entity should not take damage. This applies to living and nonliving entities alike: mobs should not take damage from any source (including potion effects), and cannot be moved by fishing rods, attacks, explosions, or projectiles, and objects such as vehicles and item frames cannot be destroyed unless their supports are removed. Invulnerable player entities are also ignored by any hostile mobs. Note that these entities can be damaged by players in Creative mode.',
    },
    'bees.entity_data.invulnerable_time': {
        id: 'app.lab.command-generator.nbt.desc.item.bees.entity_data.invulnerable_time',
        defaultMessage:
            'Optional, removed if zero or less. When greater than zero, the entity is invulnerable for that many ticks. This behaves the same as setting to true, and the two fields can be enabled or disabled independently. The entity is invulnerable when either is true, is positive, or both. Decreases by one every tick. When a cushion is struck by lightning, the dropped item has this set to 20 game ticks (1 second). When a skeleton trap is triggered, the three newly spawned skeletons and skeleton horses have this set to 60 game ticks (3 seconds).',
    },
    'bees.entity_data.Motion': {
        id: 'app.lab.command-generator.nbt.desc.item.bees.entity_data.Motion',
        defaultMessage:
            'List of 3 doubles describing the current dX, dY, and dZ velocity of the entity in meters per game tick. Only allows between 10.0 and -10.0 (inclusive), else resets to 0.',
    },
    'bees.entity_data.NoGravity': {
        id: 'app.lab.command-generator.nbt.desc.item.bees.entity_data.NoGravity',
        defaultMessage:
            '1 or 0 (true/false) - if true, the entity does not fall down naturally. Does not exist if set to false. Set to true by striders in lava.',
    },
    'bees.entity_data.OnGround': {
        id: 'app.lab.command-generator.nbt.desc.item.bees.entity_data.OnGround',
        defaultMessage: '1 or 0 (true/false) - if true, the entity is touching the ground.',
    },
    'bees.entity_data.Passengers': {
        id: 'app.lab.command-generator.nbt.desc.item.bees.entity_data.Passengers',
        defaultMessage: 'The data of the entities that are riding this entity.',
    },
    'bees.entity_data.PortalCooldown': {
        id: 'app.lab.command-generator.nbt.desc.item.bees.entity_data.PortalCooldown',
        defaultMessage:
            'The number of game ticks before which the entity may be teleported back through a nether portal. Initially starts at 300 game ticks (15 seconds) after teleportation and counts down to 0.',
    },
    'bees.entity_data.Pos': {
        id: 'app.lab.command-generator.nbt.desc.item.bees.entity_data.Pos',
        defaultMessage:
            'List of 3 doubles describing the current X, Y, and Z position (coordinates) of the entity.',
    },
    'bees.entity_data.Rotation': {
        id: 'app.lab.command-generator.nbt.desc.item.bees.entity_data.Rotation',
        defaultMessage:
            "List of 2 floats representing the entity's server-side rotation (facing direction). This does not necessarily match with the direction that the mob is looking in on the client's side. The yaw and pitch are stored inverted for most projectile entities.",
    },
    'bees.entity_data.Silent': {
        id: 'app.lab.command-generator.nbt.desc.item.bees.entity_data.Silent',
        defaultMessage: '1 or 0 (true/false) - if true, this entity is silenced. May not exist.',
    },
    'bees.entity_data.Tags': {
        id: 'app.lab.command-generator.nbt.desc.item.bees.entity_data.Tags',
        defaultMessage:
            'List of scoreboard tags of this entity. It is not preserved if it is removed.',
    },
    'bees.entity_data.TicksFrozen': {
        id: 'app.lab.command-generator.nbt.desc.item.bees.entity_data.TicksFrozen',
        defaultMessage:
            'Optional. How many game ticks the entity has been freezing (by powder snow or effect). Increases while in powder snow or with freezing effect, even partially, up to a maximum of 140 game ticks (7 seconds), and decreases at double speed while not in powder snow. Although this tag is defined for all entities, sulfur cube that has absorbed a block, entities equipped with an item in the tag #freeze_immune_wearables, skeletons, entities in the tag #freeze_immune_entity_types/#immune_to_freezing, and non-living entities are not affected by freezing.',
    },
    'bees.entity_data.UUID': {
        id: 'app.lab.command-generator.nbt.desc.item.bees.entity_data.UUID',
        defaultMessage:
            "This entity's Universally Unique Identifier. The 128-bit UUID is stored as four 32-bit integers, ordered from most to least significant.",
    },
    'bees.min_ticks_in_hive': {
        id: 'app.lab.command-generator.nbt.desc.item.bees.min_ticks_in_hive',
        defaultMessage: 'The minimum amount of time in ticks for this entity to stay in the hive.',
    },
    'bees.ticks_in_hive': {
        id: 'app.lab.command-generator.nbt.desc.item.bees.ticks_in_hive',
        defaultMessage: 'The amount of ticks the entity has stayed in the hive.',
    },
    block_entity_data: {
        id: 'app.lab.command-generator.nbt.desc.item.block_entity_data',
        defaultMessage:
            "Block entity NBT applied when this block is placed. Depending on the block type that this item places, and the block entity ID specified in this component, this component may add a red message to the item's tooltip (for operator players only) warning the player that placing it may result in command execution.",
    },
    block_state: {
        id: 'app.lab.command-generator.nbt.desc.item.block_state',
        defaultMessage:
            'The block state properties to apply when placing this block. When present on an item, regardless of the item type, if the honey_level property is specified as a number from 0 to 5 then a gray "Honey: \'<honey_level>\' / 5" text is added to the tooltip.',
    },
    block_transformer: {
        id: 'app.lab.command-generator.nbt.desc.item.block_transformer',
        defaultMessage:
            "One block transformer (an [String] ID). Define the item's usage within a block. Some blocks have usage that depends on the block, having priority (see interactable blocks), and some items have hardcoded usage.",
    },
    blocks_attacks: {
        id: 'app.lab.command-generator.nbt.desc.item.blocks_attacks',
        defaultMessage:
            'When present, this item can be used like a shield to block attacks to the holding player.',
    },
    break_sound: {
        id: 'app.lab.command-generator.nbt.desc.item.break_sound',
        defaultMessage:
            'One server-side sound event (an [String] ID, or a new [NBT Compound / JSON Object] server-side sound event definition) to play when the item runs out of durability and breaks.',
    },
    brewing_fuel: {
        id: 'app.lab.command-generator.nbt.desc.item.brewing_fuel',
        defaultMessage: 'If present, this item can be consumed for fuel by brewing stands.',
    },
    bucket_entity_data: {
        id: 'app.lab.command-generator.nbt.desc.item.bucket_entity_data',
        defaultMessage:
            'NBT applied to an entity when placed from this bucket. Only tags below are applied.',
    },
    bundle_contents: {
        id: 'app.lab.command-generator.nbt.desc.item.bundle_contents',
        defaultMessage:
            'The items stored inside this bundle. Adding this component to any item changes that item to occupy 1/16 of the space of a bundle plus any items specified in the component, regardless of how of that item normally fit in a bundle. Shulker boxes with this component are still not able to be placed inside a bundle. If this component is removed from a bundle, it cannot be used by the player and the usual instruction and fullness bar does not appear in the tooltip.',
    },
    'bundle_contents.id': {
        id: 'app.lab.command-generator.nbt.desc.item.bundle_contents.id',
        defaultMessage: 'The identifier of the item. Must not be minecraft:air.',
    },
    'bundle_contents.count': {
        id: 'app.lab.command-generator.nbt.desc.item.bundle_contents.count',
        defaultMessage:
            'Number of items stacked in this inventory slot. Defaults to 1 if not specified.',
    },
    'bundle_contents.components': {
        id: 'app.lab.command-generator.nbt.desc.item.bundle_contents.components',
        defaultMessage: 'Optional map of additional (non-default) data components.',
    },
    can_break: {
        id: 'app.lab.command-generator.nbt.desc.item.can_break',
        defaultMessage:
            'When present, the player holding the item can break the specified blocks in Adventure mode.',
    },
    'can_break.blocks': {
        id: 'app.lab.command-generator.nbt.desc.item.can_break.blocks',
        defaultMessage:
            'Any number of block types (an ID, or a tag with #, or a list of IDs). Test fails if the block is not a matching block type.',
    },
    'can_break.state': {
        id: 'app.lab.command-generator.nbt.desc.item.can_break.state',
        defaultMessage:
            "Tests the block state. Test fails if the block doesn't have matching block state properties.",
    },
    'can_break.nbt': {
        id: 'app.lab.command-generator.nbt.desc.item.can_break.nbt',
        defaultMessage:
            'An NBT compound, represented either as an object or a string containing SNBT. Test fails if the block does not have matching NBT data.',
    },
    'can_break.components': {
        id: 'app.lab.command-generator.nbt.desc.item.can_break.components',
        defaultMessage:
            "Data components to match exactly. Each key in this object is a component ID, and its value is a full component of that type. Test fails if the block doesn't have exactly the same value for every provided component.",
    },
    'can_break.predicates': {
        id: 'app.lab.command-generator.nbt.desc.item.can_break.predicates',
        defaultMessage:
            "Data component predicates to match components loosely. Each key in this object is a component predicate ID, and its value is a component predicate of that type. Test fails if the block's components do not match all predicates.",
    },
    can_place_on: {
        id: 'app.lab.command-generator.nbt.desc.item.can_place_on',
        defaultMessage:
            'Determines which blocks that blocks with this component can be placed against in Adventure mode. If defined as a compound, corresponds to [NBT Compound / JSON Object] A single block predicate. Items are listed in the tooltip in the order specified (when viewed in inventory).',
    },
    'can_place_on.blocks': {
        id: 'app.lab.command-generator.nbt.desc.item.can_place_on.blocks',
        defaultMessage:
            'Any number of block types (an ID, or a tag with #, or a list of IDs). Test fails if the block is not a matching block type.',
    },
    'can_place_on.state': {
        id: 'app.lab.command-generator.nbt.desc.item.can_place_on.state',
        defaultMessage:
            "Tests the block state. Test fails if the block doesn't have matching block state properties.",
    },
    'can_place_on.nbt': {
        id: 'app.lab.command-generator.nbt.desc.item.can_place_on.nbt',
        defaultMessage:
            'An NBT compound, represented either as an object or a string containing SNBT. Test fails if the block does not have matching NBT data.',
    },
    'can_place_on.components': {
        id: 'app.lab.command-generator.nbt.desc.item.can_place_on.components',
        defaultMessage:
            "Data components to match exactly. Each key in this object is a component ID, and its value is a full component of that type. Test fails if the block doesn't have exactly the same value for every provided component.",
    },
    'can_place_on.predicates': {
        id: 'app.lab.command-generator.nbt.desc.item.can_place_on.predicates',
        defaultMessage:
            "Data component predicates to match components loosely. Each key in this object is a component predicate ID, and its value is a component predicate of that type. Test fails if the block's components do not match all predicates.",
    },
    charged_projectiles: {
        id: 'app.lab.command-generator.nbt.desc.item.charged_projectiles',
        defaultMessage:
            'The items loaded as projectiles into this crossbow. If there are item stacks in this component, the item name for item stack is listed in the tooltip of the item as "Projectile: [\'<name>\']", and any duplicate item stack entries are grouped together as "Projectile: N x [\'<name>\']". If not present, this crossbow is not charged.',
    },
    'charged_projectiles.id': {
        id: 'app.lab.command-generator.nbt.desc.item.charged_projectiles.id',
        defaultMessage: 'The identifier of the item. Must not be minecraft:air.',
    },
    'charged_projectiles.count': {
        id: 'app.lab.command-generator.nbt.desc.item.charged_projectiles.count',
        defaultMessage:
            'Number of items stacked in this inventory slot. Defaults to 1 if not specified.',
    },
    'charged_projectiles.components': {
        id: 'app.lab.command-generator.nbt.desc.item.charged_projectiles.components',
        defaultMessage: 'Optional map of additional (non-default) data components.',
    },
    compostable: {
        id: 'app.lab.command-generator.nbt.desc.item.compostable',
        defaultMessage:
            'If present, the item can be used on composters, regardless of the value returned by layers:',
    },
    consumable: {
        id: 'app.lab.command-generator.nbt.desc.item.consumable',
        defaultMessage: 'If present, the item can be consumed. Its options can also be modified.',
    },
    'consumable.consume_seconds': {
        id: 'app.lab.command-generator.nbt.desc.item.consumable.consume_seconds',
        defaultMessage:
            'The amount of seconds it takes for a player to consume the item. Defaults to 1.6.',
    },
    'consumable.animation': {
        id: 'app.lab.command-generator.nbt.desc.item.consumable.animation',
        defaultMessage:
            'The animation used during consumption of the item. Must be one of none, eat, drink, block, bow, spear, crossbow, spyglass, toot_horn, brush, bundle, or trident. Defaults to eat.',
    },
    'consumable.sound': {
        id: 'app.lab.command-generator.nbt.desc.item.consumable.sound',
        defaultMessage:
            "Optional. One server-side sound event (an ID, or a new server-side sound event definition) used during and on completion of the item's consumption. Defaults to entity.generic.eat.",
    },
    'consumable.has_consume_particles': {
        id: 'app.lab.command-generator.nbt.desc.item.consumable.has_consume_particles',
        defaultMessage:
            'Whether consumption particles are emitted while consuming this item. Defaults to true.',
    },
    'consumable.on_consume_effects': {
        id: 'app.lab.command-generator.nbt.desc.item.consumable.on_consume_effects',
        defaultMessage:
            'A list of effects which take place as a result of consuming this item. Optional.',
    },
    container: {
        id: 'app.lab.command-generator.nbt.desc.item.container',
        defaultMessage:
            "The items contained in this container's slots. If present on an item, the first 5 item slots in the container are listed in the tooltip, and the number of any extra slots are listed as \"and '<N>' more...\" underneath (only seen on shulker boxes in normal gameplay or ctrl-middle-clicked containers in creative mode). This component supports up to 256 item slots but, when present on some block entities, the number of slots may be artificially limited based on the number of slots the block needs (e.g. 1 for decorated pots, and 27 for chests).",
    },
    'container.item': {
        id: 'app.lab.command-generator.nbt.desc.item.container.item',
        defaultMessage: 'The item stack in this slot.',
    },
    'container.item.id': {
        id: 'app.lab.command-generator.nbt.desc.item.container.item.id',
        defaultMessage: 'The identifier of the item. Must not be minecraft:air.',
    },
    'container.item.count': {
        id: 'app.lab.command-generator.nbt.desc.item.container.item.count',
        defaultMessage:
            'Number of items stacked in this inventory slot. Defaults to 1 if not specified.',
    },
    'container.item.components': {
        id: 'app.lab.command-generator.nbt.desc.item.container.item.components',
        defaultMessage: 'Optional map of additional (non-default) data components.',
    },
    'container.slot': {
        id: 'app.lab.command-generator.nbt.desc.item.container.slot',
        defaultMessage: 'A slot in this container. Can be from 0 to 255 (inclusive).',
    },
    container_loot: {
        id: 'app.lab.command-generator.nbt.desc.item.container_loot',
        defaultMessage: 'The unresolved loot table and seed of this container item.',
    },
    cooking_fuel: {
        id: 'app.lab.command-generator.nbt.desc.item.cooking_fuel',
        defaultMessage:
            'Controls whether an item can be used as smelting fuel and how effective a fuel it is.',
    },
    creative_slot_lock: {
        id: 'app.lab.command-generator.nbt.desc.item.creative_slot_lock',
        defaultMessage:
            'Only used internally to lock the informational paper items in the creative inventory. If set, this item cannot be taken out of its slot.',
    },
    custom_data: {
        id: 'app.lab.command-generator.nbt.desc.item.custom_data',
        defaultMessage:
            'Contains key-value pairs of any custom data not used by the game, either as an object or a SNBT string.',
    },
    custom_model_data: {
        id: 'app.lab.command-generator.nbt.desc.item.custom_model_data',
        defaultMessage:
            'A list of values used by items model definitions for model selection and coloring.',
    },
    'custom_model_data.floats': {
        id: 'app.lab.command-generator.nbt.desc.item.custom_model_data.floats',
        defaultMessage:
            'A list of floats for the range_dispatch model type. Missing values return the fallback model.',
    },
    'custom_model_data.flags': {
        id: 'app.lab.command-generator.nbt.desc.item.custom_model_data.flags',
        defaultMessage:
            'A list of booleans for the condition model type. Missing value returns the is_false model.',
    },
    'custom_model_data.strings': {
        id: 'app.lab.command-generator.nbt.desc.item.custom_model_data.strings',
        defaultMessage:
            'A list of strings for the select model type. Missing value returns the fallback model.',
    },
    'custom_model_data.colors': {
        id: 'app.lab.command-generator.nbt.desc.item.custom_model_data.colors',
        defaultMessage:
            "A list of RGB values for the model model type's tints. Missing values return the default value provided by the item model definition. Any provided lists of three floats automatically convert to an integer, so the component is saved as an int array.",
    },
    custom_name: {
        id: 'app.lab.command-generator.nbt.desc.item.custom_name',
        defaultMessage: "Used to specify an item, block, or entity's custom name.",
    },
    damage: {
        id: 'app.lab.command-generator.nbt.desc.item.damage',
        defaultMessage:
            "The number of uses consumed (not remaining) of the item's durability. Must be a non-negative integer, defaults to 0. If not present, the item cannot take damage. If this component is present on an item, its value is non-zero, the minecraft:max_damage is also present on the item, and the player hovering over the item has advanced tooltips on, then white text is added to the tooltip showing \"Durability: '<durability>' / '<max_damage>'\".",
    },
    damage_resistant: {
        id: 'app.lab.command-generator.nbt.desc.item.damage_resistant',
        defaultMessage:
            'If specified, this item is invulnerable to the specified damage types when in entity form or equipped.',
    },
    'damage_resistant.types': {
        id: 'app.lab.command-generator.nbt.desc.item.damage_resistant.types',
        defaultMessage: 'A damage type tag prefixed with #.',
    },
    damage_type: {
        id: 'app.lab.command-generator.nbt.desc.item.damage_type',
        defaultMessage: 'Specifies the damage type this item deals',
    },
    death_protection: {
        id: 'app.lab.command-generator.nbt.desc.item.death_protection',
        defaultMessage:
            'If present, this item protects the holder from dying by restoring a single health point.',
    },
    debug_stick_state: {
        id: 'app.lab.command-generator.nbt.desc.item.debug_stick_state',
        defaultMessage: 'The selected block state properties used by this debug stick.',
    },
    dye: {
        id: 'app.lab.command-generator.nbt.desc.item.dye',
        defaultMessage:
            'When present on an item, stores the color of dye that this item can be used as for the purpose of crafting recipes and mob or block interactions. Can be one of white, orange, magenta, light_blue, yellow, lime, pink, gray, light_gray, cyan, purple, blue, brown, green, red, or black.',
    },
    dyed_color: {
        id: 'app.lab.command-generator.nbt.desc.item.dyed_color',
        defaultMessage:
            "The RGB color of the tint applied to a dyed item, typically as a result of combining the item with dyes in a minecraft:crafting_dyed type crafting recipe. More generally, this component is used as the minecraft:dye color provider in item model definitions (so only item models that explicitly use this color provider will be tinted). If present on an item, gray text is added to the tooltip showing either an italic \"Dyed\" when advanced tooltips is off, or a non-italic \"Color: #'<hex color code>'\" instead when advanced tooltips is on. The color is stored as an integer which packs the color's red, green and blue channels using this formula:( Red '<<' 16) + ( Green '<<' 8) + Blue",
    },
    enchantable: {
        id: 'app.lab.command-generator.nbt.desc.item.enchantable',
        defaultMessage:
            'If this and minecraft:enchantments are present on an item, and applicable enchantments are available, the item can be enchanted in an enchanting table.',
    },
    'enchantable.value': {
        id: 'app.lab.command-generator.nbt.desc.item.enchantable.value',
        defaultMessage:
            "Positive integer representing the item's enchantability. A higher value allows enchantments with a higher cost to be picked.",
    },
    enchantment_glint_override: {
        id: 'app.lab.command-generator.nbt.desc.item.enchantment_glint_override',
        defaultMessage:
            'Overrides the enchantment glint effect on this item. When true, this item displays a glint, even without enchantments. When false, this item does not display a glint, even with enchantments.',
    },
    enchantments: {
        id: 'app.lab.command-generator.nbt.desc.item.enchantments',
        defaultMessage:
            "Contains a map of each of this item's enchantments to its enchantment level. This and minecraft:enchantable must exist on an item for it to be able to be enchanted in an enchanting table. If present on an item, the color of the item name is aqua if its rarity is common or uncommon, and light purple if its rarity is rare or epic. If present on an item, the enchantments are listed in the item's tooltip. The order of the enchantments in the tooltip is defined by the #minecraft:tooltip_order enchantment tag.",
    },
    entity_data: {
        id: 'app.lab.command-generator.nbt.desc.item.entity_data',
        defaultMessage:
            'NBT applied to an entity when created from an item. If this component is present on an item, the game\'s difficulty is set to peaceful and the [String] id in this component is that of a hostile mob, red text is added to the tooltip that says "Disabled in Peaceful". If the item is a spawn egg and this [String] id is falling_block, command_block_minecart, or spawner_minecart, this component adds a red message to the item\'s tooltip (visible for operator players only) warning the player that using it may result in command execution. May also be written as a plain string holding the entity ID.',
    },
    'entity_data.Air': {
        id: 'app.lab.command-generator.nbt.desc.item.entity_data.Air',
        defaultMessage:
            'How much air the entity has, in game ticks. Decreases when unable to breathe (except suffocating in a block). Increases when it can breathe. Being at or below -20 game ticks (while still unable to breathe) on a given game tick causes the entity to immediately lose 1 health to drowning damage. This resets to 0 game ticks. Most mobs can have a maximum of 300 game ticks (15 seconds) of air, while dolphins can reach up to 4800 game ticks (240 seconds), and axolotls have 6000 game ticks (300 seconds).',
    },
    'entity_data.CustomName': {
        id: 'app.lab.command-generator.nbt.desc.item.entity_data.CustomName',
        defaultMessage:
            "The custom name text component of this entity. Appears in player death messages and villager trading interfaces, as well as above the entity when the player's cursor is over it. May be empty or not exist. Represents the minecraft:custom_name component.",
    },
    'entity_data.CustomNameVisible': {
        id: 'app.lab.command-generator.nbt.desc.item.entity_data.CustomNameVisible',
        defaultMessage:
            '1 or 0 (true/false) - if true, and this entity has a custom name, the name always appears above the entity, regardless of where the cursor points. If the entity does not have a custom name, a default name is shown. May not exist.',
    },
    'entity_data.data': {
        id: 'app.lab.command-generator.nbt.desc.item.entity_data.data',
        defaultMessage:
            'Optional arbitrary NBT data. Is removed if empty. Represents the minecraft:custom_data component.',
    },
    'entity_data.fall_distance': {
        id: 'app.lab.command-generator.nbt.desc.item.entity_data.fall_distance',
        defaultMessage:
            'Distance the entity has fallen. Larger values cause more damage when the entity lands.',
    },
    'entity_data.Fire': {
        id: 'app.lab.command-generator.nbt.desc.item.entity_data.Fire',
        defaultMessage:
            'Number of game ticks until the fire is put out. Negative values reflect how long the entity can stand in fire before burning. Default to -20 when not on fire.',
    },
    'entity_data.Glowing': {
        id: 'app.lab.command-generator.nbt.desc.item.entity_data.Glowing',
        defaultMessage: '1 or 0 (true/false) - if true, the entity has a glowing outline.',
    },
    'entity_data.HasVisualFire': {
        id: 'app.lab.command-generator.nbt.desc.item.entity_data.HasVisualFire',
        defaultMessage:
            '1 or 0 (true/false) - if true, the entity visually appears on fire, even if it is not actually on fire.',
    },
    'entity_data.id': {
        id: 'app.lab.command-generator.nbt.desc.item.entity_data.id',
        defaultMessage:
            "Namespaced ID of the entity's entity type. Does not exist for entities in the world, only stored entities (such as in data components).",
    },
    'entity_data.Invulnerable': {
        id: 'app.lab.command-generator.nbt.desc.item.entity_data.Invulnerable',
        defaultMessage:
            '1 or 0 (true/false) - if true, the entity should not take damage. This applies to living and nonliving entities alike: mobs should not take damage from any source (including potion effects), and cannot be moved by fishing rods, attacks, explosions, or projectiles, and objects such as vehicles and item frames cannot be destroyed unless their supports are removed. Invulnerable player entities are also ignored by any hostile mobs. Note that these entities can be damaged by players in Creative mode.',
    },
    'entity_data.invulnerable_time': {
        id: 'app.lab.command-generator.nbt.desc.item.entity_data.invulnerable_time',
        defaultMessage:
            'Optional, removed if zero or less. When greater than zero, the entity is invulnerable for that many ticks. This behaves the same as setting to true, and the two fields can be enabled or disabled independently. The entity is invulnerable when either is true, is positive, or both. Decreases by one every tick. When a cushion is struck by lightning, the dropped item has this set to 20 game ticks (1 second). When a skeleton trap is triggered, the three newly spawned skeletons and skeleton horses have this set to 60 game ticks (3 seconds).',
    },
    'entity_data.Motion': {
        id: 'app.lab.command-generator.nbt.desc.item.entity_data.Motion',
        defaultMessage:
            'List of 3 doubles describing the current dX, dY, and dZ velocity of the entity in meters per game tick. Only allows between 10.0 and -10.0 (inclusive), else resets to 0.',
    },
    'entity_data.NoGravity': {
        id: 'app.lab.command-generator.nbt.desc.item.entity_data.NoGravity',
        defaultMessage:
            '1 or 0 (true/false) - if true, the entity does not fall down naturally. Does not exist if set to false. Set to true by striders in lava.',
    },
    'entity_data.OnGround': {
        id: 'app.lab.command-generator.nbt.desc.item.entity_data.OnGround',
        defaultMessage: '1 or 0 (true/false) - if true, the entity is touching the ground.',
    },
    'entity_data.Passengers': {
        id: 'app.lab.command-generator.nbt.desc.item.entity_data.Passengers',
        defaultMessage: 'The data of the entities that are riding this entity.',
    },
    'entity_data.PortalCooldown': {
        id: 'app.lab.command-generator.nbt.desc.item.entity_data.PortalCooldown',
        defaultMessage:
            'The number of game ticks before which the entity may be teleported back through a nether portal. Initially starts at 300 game ticks (15 seconds) after teleportation and counts down to 0.',
    },
    'entity_data.Pos': {
        id: 'app.lab.command-generator.nbt.desc.item.entity_data.Pos',
        defaultMessage:
            'List of 3 doubles describing the current X, Y, and Z position (coordinates) of the entity.',
    },
    'entity_data.Rotation': {
        id: 'app.lab.command-generator.nbt.desc.item.entity_data.Rotation',
        defaultMessage:
            "List of 2 floats representing the entity's server-side rotation (facing direction). This does not necessarily match with the direction that the mob is looking in on the client's side. The yaw and pitch are stored inverted for most projectile entities.",
    },
    'entity_data.Silent': {
        id: 'app.lab.command-generator.nbt.desc.item.entity_data.Silent',
        defaultMessage: '1 or 0 (true/false) - if true, this entity is silenced. May not exist.',
    },
    'entity_data.Tags': {
        id: 'app.lab.command-generator.nbt.desc.item.entity_data.Tags',
        defaultMessage:
            'List of scoreboard tags of this entity. It is not preserved if it is removed.',
    },
    'entity_data.TicksFrozen': {
        id: 'app.lab.command-generator.nbt.desc.item.entity_data.TicksFrozen',
        defaultMessage:
            'Optional. How many game ticks the entity has been freezing (by powder snow or effect). Increases while in powder snow or with freezing effect, even partially, up to a maximum of 140 game ticks (7 seconds), and decreases at double speed while not in powder snow. Although this tag is defined for all entities, sulfur cube that has absorbed a block, entities equipped with an item in the tag #freeze_immune_wearables, skeletons, entities in the tag #freeze_immune_entity_types/#immune_to_freezing, and non-living entities are not affected by freezing.',
    },
    'entity_data.UUID': {
        id: 'app.lab.command-generator.nbt.desc.item.entity_data.UUID',
        defaultMessage:
            "This entity's Universally Unique Identifier. The 128-bit UUID is stored as four 32-bit integers, ordered from most to least significant.",
    },
    equippable: {
        id: 'app.lab.command-generator.nbt.desc.item.equippable',
        defaultMessage: 'If present, this item can be equipped in the specified slot.',
    },
    'equippable.slot': {
        id: 'app.lab.command-generator.nbt.desc.item.equippable.slot',
        defaultMessage:
            'The slot to put the item on. Can be one of head, chest, legs, feet, body, mainhand, offhand, or saddle.',
    },
    'equippable.equip_sound': {
        id: 'app.lab.command-generator.nbt.desc.item.equippable.equip_sound',
        defaultMessage:
            'Optional. One server-side sound event (an ID, or a new server-side sound event definition) to play when the item is equipped. Defaults to item.armor.equip_generic.',
    },
    'equippable.asset_id': {
        id: 'app.lab.command-generator.nbt.desc.item.equippable.asset_id',
        defaultMessage:
            "The identifier of the equipment model to use when equipped. The directory this refers to is assets/'<namespace>'/equipment/'<id>'.json. If not specified, falls back to rendering as the item itself when in the head slot (if not applicable, the item does not render).",
    },
    'equippable.allowed_entities': {
        id: 'app.lab.command-generator.nbt.desc.item.equippable.allowed_entities',
        defaultMessage:
            'Entity ID, Entity Tag, or list of Entity IDs to limit which entities can equip this item. Defaults to all entities.',
    },
    'equippable.dispensable': {
        id: 'app.lab.command-generator.nbt.desc.item.equippable.dispensable',
        defaultMessage: 'Whether the item can be dispensed by using a dispenser. Defaults to true.',
    },
    'equippable.swappable': {
        id: 'app.lab.command-generator.nbt.desc.item.equippable.swappable',
        defaultMessage:
            'Whether the item can be equipped into the relevant slot by right-clicking. Defaults to true.',
    },
    'equippable.damage_on_hurt': {
        id: 'app.lab.command-generator.nbt.desc.item.equippable.damage_on_hurt',
        defaultMessage:
            'Whether this item is damaged when the wearing entity is damaged. Defaults to true.',
    },
    'equippable.equip_on_interact': {
        id: 'app.lab.command-generator.nbt.desc.item.equippable.equip_on_interact',
        defaultMessage:
            'Whether this item can be equipped onto a target mob by pressing use on it (as long as this item can be equipped on the target at all). Defaults to false.',
    },
    'equippable.camera_overlay': {
        id: 'app.lab.command-generator.nbt.desc.item.equippable.camera_overlay',
        defaultMessage:
            "The identifier of the overlay texture to use when equipped. The directory this refers to is assets/'<namespace>'/textures/'<id>'. Assets which do not exist will use the missing texture, rather than falling back to a default such as the pumpkin overlay.",
    },
    'equippable.can_be_sheared': {
        id: 'app.lab.command-generator.nbt.desc.item.equippable.can_be_sheared',
        defaultMessage:
            'Whether this item can be unequipped from a target mob by right-clicking with Shears. Defaults to false.',
    },
    'equippable.shearing_sound': {
        id: 'app.lab.command-generator.nbt.desc.item.equippable.shearing_sound',
        defaultMessage:
            'Optional. One server-side sound event (an ID, or a new server-side sound event definition) to play when the item is sheared. Defaults to item.shears.snip.',
    },
    firework_explosion: {
        id: 'app.lab.command-generator.nbt.desc.item.firework_explosion',
        defaultMessage: 'The explosion effect stored by this firework star.',
    },
    fireworks: {
        id: 'app.lab.command-generator.nbt.desc.item.fireworks',
        defaultMessage:
            'The flight duration of this firework rocket and the explosion effects it carries.',
    },
    food: {
        id: 'app.lab.command-generator.nbt.desc.item.food',
        defaultMessage: 'The food stats applied to the mob or player upon consuming this item.',
    },
    'food.nutrition': {
        id: 'app.lab.command-generator.nbt.desc.item.food.nutrition',
        defaultMessage:
            'The number of food points restored by this item when the player eats. The number of health points restored by this item when used to feed cats. Wolfs, nautilus and zombie nautilus restored the twice the value. Must be a non-negative integer.',
    },
    'food.saturation': {
        id: 'app.lab.command-generator.nbt.desc.item.food.saturation',
        defaultMessage: 'The amount of saturation restored by this item when eaten.',
    },
    'food.can_always_eat': {
        id: 'app.lab.command-generator.nbt.desc.item.food.can_always_eat',
        defaultMessage:
            'If true, this item can be eaten even if the player is not hungry. Defaults to false.',
    },
    glider: {
        id: 'app.lab.command-generator.nbt.desc.item.glider',
        defaultMessage:
            "If present, this item allows living entities to glide (as with elytra) when equipped. If the item is damageable, it will only allow entities to glide if it has minecraft:damage '<' minecraft:max_damage − 1. Every second while an entity is gliding, the game will select a random equipped glider item that isn't too damaged to function and try to apply one point of durability damage.",
    },
    instrument: {
        id: 'app.lab.command-generator.nbt.desc.item.instrument',
        defaultMessage:
            "One instrument (an [String] ID, or a new [NBT Compound / JSON Object] instrument definition). If present on an item, the description of the instrument is shown in the item's tooltip as gray text.",
    },
    intangible_projectile: {
        id: 'app.lab.command-generator.nbt.desc.item.intangible_projectile',
        defaultMessage:
            'If present on an item that can be used as a projectile without breaking upon impact (such as arrows and tridents), the projectile cannot be picked up by a player once fired, unless they are in creative mode. When present on any item, a line of gray text that says "Intangible" is added to the item\'s tooltip.',
    },
    interact_animation: {
        id: 'app.lab.command-generator.nbt.desc.item.interact_animation',
        defaultMessage:
            'The swing animation to play when the item is used to interact with something.',
    },
    item_model: {
        id: 'app.lab.command-generator.nbt.desc.item.item_model',
        defaultMessage:
            'The identifier of an item model definition. Referencing nonexistent models will cause the missing model to be used.',
    },
    item_name: {
        id: 'app.lab.command-generator.nbt.desc.item.item_name',
        defaultMessage:
            'The default name of this item, as a text component, present on all items by default. Unlike the minecraft:custom_name component, this name cannot be erased using an anvil, is not italicized, and does not appear in some labels, such as banner markers and item frames.',
    },
    jukebox_playable: {
        id: 'app.lab.command-generator.nbt.desc.item.jukebox_playable',
        defaultMessage:
            "One jukebox song (an [String] ID) to play when inserted into a jukebox. If present on an item, the item can be inserted into a jukebox to play the specified song. If present on an item, the artist and the title of the song are added to the item's tooltip as gray text.",
    },
    kinetic_weapon: {
        id: 'app.lab.command-generator.nbt.desc.item.kinetic_weapon',
        defaultMessage:
            'Enables a charge-type attack when using the item where, while being used, the damage is dealt along a ray every tick based on the relative speed of the entities',
    },
    lock: {
        id: 'app.lab.command-generator.nbt.desc.item.lock',
        defaultMessage:
            'An item predicate representing the "key" to open this container. If present on a block, players must have an item that passes this predicate in their main hand in order to open the container. If they do not, interacting with the container plays a sound and displays a message in the actionbar.',
    },
    lodestone_tracker: {
        id: 'app.lab.command-generator.nbt.desc.item.lodestone_tracker',
        defaultMessage:
            'Stores information about the lodestone this compass should point toward. If present on a compass item, the base item name will be overridden as "Lodestone Compass". While tracked in a player\'s inventory, a compass with this component will do active polling to see if the position of its lodestone is loaded, and if it is and the lodestone has been destroyed then this component is removed.',
    },
    lore: {
        id: 'app.lab.command-generator.nbt.desc.item.lore',
        defaultMessage:
            "List of additional lines to display in this item's tooltip. Has a maximum of 256 lines.",
    },
    map_color: {
        id: 'app.lab.command-generator.nbt.desc.item.map_color',
        defaultMessage:
            'The color code for the minecraft:map_color color provider in item model definitions. Normally used by the minecraft:filled_map item model.',
    },
    map_decorations: {
        id: 'app.lab.command-generator.nbt.desc.item.map_decorations',
        defaultMessage: 'Contains key-value pairs of the icons to display on this filled map.',
    },
    map_id: {
        id: 'app.lab.command-generator.nbt.desc.item.map_id',
        defaultMessage:
            'The map data number for this item, representing the shared state holding map contents banner and item frames decoration. When present, add information to the tooltip and change the rendering of the item in hand or in the item frame for map rendering. Only maps (filled_map) and explorer maps can update map content.',
    },
    map_post_processing: {
        id: 'app.lab.command-generator.nbt.desc.item.map_post_processing',
        defaultMessage:
            'Only used internally when this filled map has been duplicated or locked in a crafting table or a cartography table. Can be 0 (lock) or 1 (scale), adding the "Locked" line or "Scale" line in this item\'s tooltip, respectively.',
    },
    max_damage: {
        id: 'app.lab.command-generator.nbt.desc.item.max_damage',
        defaultMessage:
            'The maximum amount of damage that this item can take. If not set, this item cannot take damage. Must be a non-zero positive integer. Cannot be combined with minecraft:max_stack_size if it has a value greater than 1 (if the item can be stacked). For the durability bar to appear, the damage component must have a value. Example damage=0.',
    },
    max_stack_size: {
        id: 'app.lab.command-generator.nbt.desc.item.max_stack_size',
        defaultMessage:
            'The maximum number of items that can fit in a stack. Must be a positive integer between 1 and 99 (inclusive). If it has a value greater than 1 (if the item can be stacked), cannot be combined with minecraft:max_damage. If this component is removed, it will behave as though it was set to 1.',
    },
    minimum_attack_charge: {
        id: 'app.lab.command-generator.nbt.desc.item.minimum_attack_charge',
        defaultMessage:
            'Sets the minimum attack charge on the attack indicator required to attack with this item. Must be a non-negative float between 0.0 and 1.0',
    },
    mob_visibility: {
        id: 'app.lab.command-generator.nbt.desc.item.mob_visibility',
        defaultMessage:
            'Represents the visibility percentage provided by an item with the appropriate minecraft:equippable component, modifying the range at which mobs are able to detect an entity.',
    },
    note_block_sound: {
        id: 'app.lab.command-generator.nbt.desc.item.note_block_sound',
        defaultMessage:
            'The ID of the sound event played by a note block when this player head is placed above.',
    },
    ominous_bottle_amplifier: {
        id: 'app.lab.command-generator.nbt.desc.item.ominous_bottle_amplifier',
        defaultMessage:
            'The amplifier of the Bad Omen effect given to the player or mob upon consuming this item (normally an ominous bottle). Must be a non-negative integer from 0 to 4 (inclusive). The duration of the effect is always 120000 ticks (1 hour and 40 minutes at the normal tick rate) and minecraft:potion_duration_scale is ignored. If present on an item, information about the mob effect is added to the tooltip, in the same format as the minecraft:potion_contents component.',
    },
    piercing_weapon: {
        id: 'app.lab.command-generator.nbt.desc.item.piercing_weapon',
        defaultMessage:
            'Melee attacks using this item damage multiple entities along a ray, instead of only a single entity. Also prevents this item from being used to mine blocks.',
    },
    pot_decorations: {
        id: 'app.lab.command-generator.nbt.desc.item.pot_decorations',
        defaultMessage:
            'A map for face to the item stacks (typically pottery sherds or bricks). The faces can be empty. When used on a decorated pot, it defines the items that will be dropped from its faces when broken. If an item has the minecraft:provides_pottery_pattern component, it defines the texture of the corresponding face.',
    },
    'pot_decorations.left': {
        id: 'app.lab.command-generator.nbt.desc.item.pot_decorations.left',
        defaultMessage: 'The item stack in the left side.',
    },
    'pot_decorations.left.id': {
        id: 'app.lab.command-generator.nbt.desc.item.pot_decorations.left.id',
        defaultMessage: 'The identifier of the item. Must not be minecraft:air.',
    },
    'pot_decorations.left.count': {
        id: 'app.lab.command-generator.nbt.desc.item.pot_decorations.left.count',
        defaultMessage:
            'Number of items stacked in this inventory slot. Defaults to 1 if not specified.',
    },
    'pot_decorations.left.components': {
        id: 'app.lab.command-generator.nbt.desc.item.pot_decorations.left.components',
        defaultMessage: 'Optional map of additional (non-default) data components.',
    },
    'pot_decorations.back': {
        id: 'app.lab.command-generator.nbt.desc.item.pot_decorations.back',
        defaultMessage: 'The item stack in the back side.',
    },
    'pot_decorations.back.id': {
        id: 'app.lab.command-generator.nbt.desc.item.pot_decorations.back.id',
        defaultMessage: 'The identifier of the item. Must not be minecraft:air.',
    },
    'pot_decorations.back.count': {
        id: 'app.lab.command-generator.nbt.desc.item.pot_decorations.back.count',
        defaultMessage:
            'Number of items stacked in this inventory slot. Defaults to 1 if not specified.',
    },
    'pot_decorations.back.components': {
        id: 'app.lab.command-generator.nbt.desc.item.pot_decorations.back.components',
        defaultMessage: 'Optional map of additional (non-default) data components.',
    },
    'pot_decorations.right': {
        id: 'app.lab.command-generator.nbt.desc.item.pot_decorations.right',
        defaultMessage: 'The item stack in the right side.',
    },
    'pot_decorations.right.id': {
        id: 'app.lab.command-generator.nbt.desc.item.pot_decorations.right.id',
        defaultMessage: 'The identifier of the item. Must not be minecraft:air.',
    },
    'pot_decorations.right.count': {
        id: 'app.lab.command-generator.nbt.desc.item.pot_decorations.right.count',
        defaultMessage:
            'Number of items stacked in this inventory slot. Defaults to 1 if not specified.',
    },
    'pot_decorations.right.components': {
        id: 'app.lab.command-generator.nbt.desc.item.pot_decorations.right.components',
        defaultMessage: 'Optional map of additional (non-default) data components.',
    },
    'pot_decorations.front': {
        id: 'app.lab.command-generator.nbt.desc.item.pot_decorations.front',
        defaultMessage: 'The item stack in the front side.',
    },
    'pot_decorations.front.id': {
        id: 'app.lab.command-generator.nbt.desc.item.pot_decorations.front.id',
        defaultMessage: 'The identifier of the item. Must not be minecraft:air.',
    },
    'pot_decorations.front.count': {
        id: 'app.lab.command-generator.nbt.desc.item.pot_decorations.front.count',
        defaultMessage:
            'Number of items stacked in this inventory slot. Defaults to 1 if not specified.',
    },
    'pot_decorations.front.components': {
        id: 'app.lab.command-generator.nbt.desc.item.pot_decorations.front.components',
        defaultMessage: 'Optional map of additional (non-default) data components.',
    },
    potion_contents: {
        id: 'app.lab.command-generator.nbt.desc.item.potion_contents',
        defaultMessage:
            "The base potion, custom list of mob effects, and custom color contained in this potion, splash potion, lingering potion, tipped arrow, or area effect cloud. When present on an item, the mob effects are listed in the item's tooltip. If this and a minecraft:consumable component are present on an item, consuming the item will apply all of the effects from this component to the player or mob that consumed it. If defined as a string, corresponds to [String] potion.",
    },
    potion_duration_scale: {
        id: 'app.lab.command-generator.nbt.desc.item.potion_duration_scale',
        defaultMessage:
            'When present on an item or area effect cloud that has the minecraft:potion_contents component, the duration of the applied effects is scaled by this factor.',
    },
    profile: {
        id: 'app.lab.command-generator.nbt.desc.item.profile',
        defaultMessage:
            'Provides the textures needed for rendering player models (player head blocks, player head item models, and mannequins) and player face sprites. Each texture can be provided either as the namespaced path to a texture in a resource pack, or from a player profile. A player profile can be specified either as player information whose profile properties must be requested on the fly, or a list of already-resolved profile properties. May also be written as a plain string holding a player name or UUID.',
    },
    'profile.name': {
        id: 'app.lab.command-generator.nbt.desc.item.profile.name',
        defaultMessage:
            "Optional. A username, consisting of 0-16 non-whitespace printable ASCII characters (this is more permissive than actual player usernames, which must be 3-16 characters and may only contain letters, numbers, and underscores). If no other profile fields are specified, this is used to dynamically request the profile of a player with that username from Mojang's servers.",
    },
    'profile.id': {
        id: 'app.lab.command-generator.nbt.desc.item.profile.id',
        defaultMessage:
            "Optional. A UUID. If no other profile fields are specified, this is used to dynamically request the profile of a player with that UUID from Mojang's servers.",
    },
    'profile.properties': {
        id: 'app.lab.command-generator.nbt.desc.item.profile.properties',
        defaultMessage:
            'Optional. A non-empty list of user profile properties. If this is specified, the dynamic profile fields are ignored, and a custom profile with these properties is used instead. If set to an empty list, the field is removed rather than invalidating the whole component.',
    },
    'profile.properties.name': {
        id: 'app.lab.command-generator.nbt.desc.item.profile.properties.name',
        defaultMessage: 'The name of the property. Can only be "textures".',
    },
    'profile.properties.value': {
        id: 'app.lab.command-generator.nbt.desc.item.profile.properties.value',
        defaultMessage: 'The texture data json, encoded in base64.',
    },
    'profile.properties.signature': {
        id: 'app.lab.command-generator.nbt.desc.item.profile.properties.signature',
        defaultMessage: "Optional. Mojang's signature of the value, encoded in base64.",
    },
    'profile.texture': {
        id: 'app.lab.command-generator.nbt.desc.item.profile.texture',
        defaultMessage:
            'Optional. Namespaced path to a player skin texture, relative to the textures folder in a resource pack. If specified, this texture is used when rendering a player model or skin, overriding the skin texture provided by a profile.',
    },
    'profile.cape': {
        id: 'app.lab.command-generator.nbt.desc.item.profile.cape',
        defaultMessage:
            'Optional. Namespaced path to a cape texture, relative to the textures folder in a resource pack. If specified, this texture is used when rendering a cape, overriding the cape texture provided by a profile. This does not affect the elytra texture.',
    },
    'profile.elytra': {
        id: 'app.lab.command-generator.nbt.desc.item.profile.elytra',
        defaultMessage:
            'Optional. Namespaced path to an elytra texture, relative to the textures folder in a resource pack. If specified, this texture is used when rendering an elytra, overriding the elytra texture provided by a profile. This does not affect the cape texture.',
    },
    'profile.model': {
        id: 'app.lab.command-generator.nbt.desc.item.profile.model',
        defaultMessage:
            'Optional. The type of player model to use. Either "wide" or "slim". If specified, this overrides the model type provided by a profile.',
    },
    provides_banner_patterns: {
        id: 'app.lab.command-generator.nbt.desc.item.provides_banner_patterns',
        defaultMessage:
            'Any number of banner pattern(s) (an [String] ID, or a [String] tag with #, or an [NBT List / JSON Array] array containing [String] IDs). When present, this item can be placed in the pattern slot of a loom and provides the specified banner patterns.',
    },
    provides_pottery_pattern: {
        id: 'app.lab.command-generator.nbt.desc.item.provides_pottery_pattern',
        defaultMessage:
            'One decorated pot pattern (an [String] ID). When present, this item provides the specified pattern when used in a decorated pot side.',
    },
    provides_trim_material: {
        id: 'app.lab.command-generator.nbt.desc.item.provides_trim_material',
        defaultMessage:
            'One trim material (an [String] ID, or a new [NBT Compound / JSON Object] trim material definition). When present, this item provides the specified trim material when used in a trimming recipe.',
    },
    rarity: {
        id: 'app.lab.command-generator.nbt.desc.item.rarity',
        defaultMessage:
            'Sets the rarity of this item, which affects the default color of its name. Can be common, uncommon, rare, or epic. If this component does not exist on the item, then common is used.',
    },
    recipes: {
        id: 'app.lab.command-generator.nbt.desc.item.recipes',
        defaultMessage: 'The recipes that a player unlocks when this knowledge book is used.',
    },
    repair_cost: {
        id: 'app.lab.command-generator.nbt.desc.item.repair_cost',
        defaultMessage:
            'The number of experience levels to add to the base level cost when repairing, combining, or renaming this item with an anvil. Must be a non-negative integer, defaults to 0.',
    },
    repairable: {
        id: 'app.lab.command-generator.nbt.desc.item.repairable',
        defaultMessage:
            'Allows the item to be repaired, if damageable, in an anvil using the specified ingredient. Also repairs equipped items in the body slot of a tamed wolf.',
    },
    sign_text_back: {
        id: 'app.lab.command-generator.nbt.desc.item.sign_text_back',
        defaultMessage: 'The text shown on the back of a sign.',
    },
    sign_text_front: {
        id: 'app.lab.command-generator.nbt.desc.item.sign_text_front',
        defaultMessage: 'The text shown on the front of a sign.',
    },
    stored_enchantments: {
        id: 'app.lab.command-generator.nbt.desc.item.stored_enchantments',
        defaultMessage:
            "Contains a map of inactive enchantments and their levels. Adding this component to any item other than an enchanted book does nothing except add text to the item's tooltip. If this component is removed from an enchanted book, the item can no longer be combined with other enchanted books. If present on an item, the enchantments are listed in the item's tooltip. The order of the enchantments in the tooltip is defined by the #minecraft:tooltip_order enchantment tag.",
    },
    sulfur_cube_content: {
        id: 'app.lab.command-generator.nbt.desc.item.sulfur_cube_content',
        defaultMessage: 'The item stored inside the sulfur cube.',
    },
    'sulfur_cube_content.id': {
        id: 'app.lab.command-generator.nbt.desc.item.sulfur_cube_content.id',
        defaultMessage: 'The identifier of the item. Must not be minecraft:air.',
    },
    'sulfur_cube_content.count': {
        id: 'app.lab.command-generator.nbt.desc.item.sulfur_cube_content.count',
        defaultMessage:
            'Number of items stacked in this inventory slot. Defaults to 1 if not specified.',
    },
    'sulfur_cube_content.components': {
        id: 'app.lab.command-generator.nbt.desc.item.sulfur_cube_content.components',
        defaultMessage: 'Optional map of additional (non-default) data components.',
    },
    suspicious_stew_effects: {
        id: 'app.lab.command-generator.nbt.desc.item.suspicious_stew_effects',
        defaultMessage:
            "A list of unamplified mob effects given to the player or mob upon consuming this item (normally a suspicious stew). minecraft:potion_duration_scale is ignored. When present on an item, text is added to the item's tooltip only when it is inside of the item selection screen.",
    },
    'suspicious_stew_effects.id': {
        id: 'app.lab.command-generator.nbt.desc.item.suspicious_stew_effects.id',
        defaultMessage: 'The ID of the effect.',
    },
    'suspicious_stew_effects.duration': {
        id: 'app.lab.command-generator.nbt.desc.item.suspicious_stew_effects.duration',
        defaultMessage: 'The duration of the effect in ticks. Defaults to 160.',
    },
    swing_animation: {
        id: 'app.lab.command-generator.nbt.desc.item.swing_animation',
        defaultMessage:
            'This component was split into attack_animation and interact_animation in 26.3.',
    },
    tag: {
        id: 'app.lab.command-generator.nbt.desc.item.tag',
        defaultMessage: 'Item data before 1.20.5, replaced by data components.',
    },
    'tag.display': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.display',
        defaultMessage: 'Display properties.',
    },
    'tag.display.Name': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.display.Name',
        defaultMessage: 'The JSON text component to use to display the item.',
    },
    'tag.display.Lore': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.display.Lore',
        defaultMessage: 'List of strings to display as lore for the item.',
    },
    'tag.display.color': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.display.color',
        defaultMessage:
            "The color of the leather armor. The tooltip displays \"Dyed\" if advanced tooltips are disabled, otherwise it displays the hexadecimal color value. Color codes are the hex code of the color converted to a decimal number, or can be calculated from the Red, Green and Blue components using this formula: Red'<<'16 + Green'<<'8 + Blue.",
    },
    'tag.display.MapColor': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.display.MapColor',
        defaultMessage: "The color of the markings on the item's texture.",
    },
    'tag.HideFlags': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.HideFlags',
        defaultMessage: 'Bit field determining which parts of the tooltip to hide on an item.',
    },
    'tag.CustomModelData': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.CustomModelData',
        defaultMessage:
            'A value used in the "custom_model_data" item tag in the overrides of item models.',
    },
    'tag.AttributeModifiers': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.AttributeModifiers',
        defaultMessage:
            'Contains Attribute Modifiers on this item which modify Attributes of the wearer or holder (if the item is not in the hand or armor slots, it has no effect).',
    },
    'tag.Enchantments': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.Enchantments',
        defaultMessage: 'Contains enchantments on this item that affect the way the item works.',
    },
    'tag.StoredEnchantments': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.StoredEnchantments',
        defaultMessage: 'Contains enchantments for enchanted books.',
    },
    'tag.RepairCost': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.RepairCost',
        defaultMessage:
            'Number of experience levels to add to the base level cost when repairing, combining, or renaming this item with an Anvil.',
    },
    'tag.CanDestroy': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.CanDestroy',
        defaultMessage:
            'The only blocks this item may break when used by a player in adventure mode.',
    },
    'tag.CanPlaceOn': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.CanPlaceOn',
        defaultMessage:
            'Determines which blocks that blocks with this tag can be placed against in adventure mode.',
    },
    'tag.BlockEntityTag': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.BlockEntityTag',
        defaultMessage: 'Block entity NBT tags which are applied when this block is placed.',
    },
    'tag.BlockStateTag': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.BlockStateTag',
        defaultMessage:
            'A compound where each key is a block state key, and the value is the block state value to force place for this block.',
    },
    'tag.BlockStateTag.Block state key': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.BlockStateTag.Block state key',
        defaultMessage: 'The block state value of the block to edit, for example, facing: "east".',
    },
    'tag.Damage': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.Damage',
        defaultMessage: "The number of uses consumed (not remaining) of the item's durability.",
    },
    'tag.Unbreakable': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.Unbreakable',
        defaultMessage:
            "1 or 0 (true/false) - if true, the item doesn't lose durability when used.",
    },
    'tag.custom_potion_effects': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.custom_potion_effects',
        defaultMessage:
            'The custom potion effects (status effects) this potion or tipped arrow has.',
    },
    'tag.Potion': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.Potion',
        defaultMessage:
            'The name of the default potion effect. This name differs from the status effect name. For example, the value for an "Instant Health II" potion is "minecraft:strong_healing". A potion or tipped arrow getting its effects from this tag is named with the proper effect. The default value is "minecraft:empty", which gives it the "Uncraftable" name.',
    },
    'tag.CustomPotionColor': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.CustomPotionColor',
        defaultMessage:
            "The item uses this custom color, and area-of-effect clouds, arrows, and splash and lingering potions use it for their particle effects. This color does not extend, however, to the particles given off by entities who ultimately receive the effect. The numeric color codes are equal to the hex code of the color converted to a decimal number, or can alternatively be calculated from the Red, Green and Blue components using this formula: Red'<<'16 + Green'<<'8 + Blue. For positive values larger than 0x00FFFFFF, the top byte is ignored. All negative values produce white.",
    },
    'tag.Trim': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.Trim',
        defaultMessage: 'Properties of Armor Trim.',
    },
    'tag.Trim.material': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.Trim.material',
        defaultMessage: 'The material which decides the color of armor trim.',
    },
    'tag.Trim.pattern': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.Trim.pattern',
        defaultMessage: 'The pattern of armor trim.',
    },
    'tag.EntityTag': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.EntityTag',
        defaultMessage: 'Stores entity data that is applied to the entity when created.',
    },
    'tag.filtered_pages': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.filtered_pages',
        defaultMessage:
            'Only in Realms. The pages shown to players with the profanity filter turned on instead of some of the regular pages. This compound is automatically given a tag named after the page number and set to "" when a player with the profanity filter turned off closes a book with that specific page containing one or more blocked words, so players with the filter on cannot see the blocked words. If multiple pages have been edited before the book is closed, only the pages containing blocked words are blanked.',
    },
    'tag.pages': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.pages',
        defaultMessage: 'The list of pages in the book.',
    },
    'tag.filtered_title': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.filtered_title',
        defaultMessage:
            'Only in Realms. The title shown to players with the profanity filter turned on instead of the regular title. This tag is automatically set to "" when a player with the profanity filter turned off signs a book with a title containing one or more blocked words, so players with the filter on cannot see the blocked words.',
    },
    'tag.resolved': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.resolved',
        defaultMessage:
            'Optional. Created and set to 1 when the book (or a book from the stack) is opened for the first time after being created. Used to determine whether to resolve advanced components within JSON, because their values become fixed at that point.',
    },
    'tag.generation': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.generation',
        defaultMessage:
            'Optional. The copy tier of the book. 0 = original, 1 = copy of original, 2 = copy of copy, 3 = tattered. If the value is greater than 1, the book cannot be copied. Does not exist for original books. If this tag is missing, it is assumed the book is an original. "Tattered" is unused in normal gameplay, and functions identically to the "copy of copy" tier.',
    },
    'tag.author': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.author',
        defaultMessage: 'The author of the written book.',
    },
    'tag.title': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.title',
        defaultMessage:
            'The title of the written book. Lower priority than the item name set by tag.display.Name. Cannot be used to open locked containers.',
    },
    'tag.Recipes': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.Recipes',
        defaultMessage: 'The list of recipes this book contains.',
    },
    'tag.BucketVariantTag': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.BucketVariantTag',
        defaultMessage: 'The variant data of the tropical fish in the bucket.',
    },
    'tag.Items': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.Items',
        defaultMessage: 'List of items in the bundle.',
    },
    'tag.LodestoneTracked': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.LodestoneTracked',
        defaultMessage:
            'Optional. 1 or 0 (true/false) - true if the compass is connected to a lodestone. When false, LodestoneDimension and LodestonePos is not automatically removed when the lodestone is destroyed, but the compass still points to that location.',
    },
    'tag.LodestoneDimension': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.LodestoneDimension',
        defaultMessage:
            'Optional. The dimension of the lodestone to which the compass is pointing.',
    },
    'tag.LodestonePos': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.LodestonePos',
        defaultMessage:
            'Optional. The coordinates of the lodestone to which the compass is pointing.',
    },
    'tag.LodestonePos.X': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.LodestonePos.X',
        defaultMessage: 'x coordinate',
    },
    'tag.LodestonePos.Y': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.LodestonePos.Y',
        defaultMessage: 'y coordinate',
    },
    'tag.LodestonePos.Z': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.LodestonePos.Z',
        defaultMessage: 'z coordinate',
    },
    'tag.ChargedProjectiles': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.ChargedProjectiles',
        defaultMessage:
            'The items this crossbow has charged, normally one entry (three if enchanted with multishot)',
    },
    'tag.Charged': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.Charged',
        defaultMessage: 'If the crossbow is charged.',
    },
    'tag.DebugProperty': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.DebugProperty',
        defaultMessage:
            'A compound where each key is a block ID, and the value is the block state key to edit for that block ID.',
    },
    'tag.DebugProperty.Block ID': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.DebugProperty.Block ID',
        defaultMessage:
            'The block state key of the block to edit, for example "minecraft:oak_fence": "east".',
    },
    'tag.Fireworks': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.Fireworks',
        defaultMessage: 'One of these may appear on a firework rocket.',
    },
    'tag.Fireworks.Explosions': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.Fireworks.Explosions',
        defaultMessage: 'List of compounds representing each explosion this firework causes.',
    },
    'tag.Fireworks.Flight': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.Fireworks.Flight',
        defaultMessage:
            'Indicates the flight duration of the firework (equals the amount of gunpowder used in crafting the rocket).',
    },
    'tag.Explosion': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.Explosion',
        defaultMessage: 'The explosion effect contributed by this firework star.',
    },
    'tag.instrument': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.instrument',
        defaultMessage: 'The identifier of the goat horn variant.',
    },
    'tag.map': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.map',
        defaultMessage: 'The map number.',
    },
    'tag.map_scale_direction': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.map_scale_direction',
        defaultMessage:
            'Only internally used when scaling a map, after that directly removed: The amount to increase the current map scale by when crafting. Always 1.',
    },
    'tag.map_to_lock': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.map_to_lock',
        defaultMessage:
            '1 or 0 (true/false) - true if the map should be locked after being taken out of the cartography table.',
    },
    'tag.Decorations': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.Decorations',
        defaultMessage: 'A list of optional icons to display on the map.',
    },
    'tag.SkullOwner': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.SkullOwner',
        defaultMessage: 'The username of the player this is a skull of.',
    },
    'tag.SkullOwner.Id': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.SkullOwner.Id',
        defaultMessage:
            "UUID of owner, stored as four ints. Optional. Used to update the other tags when the chunk loads or the holder logs in, in case the owner's name has changed.",
    },
    'tag.SkullOwner.Name': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.SkullOwner.Name',
        defaultMessage:
            'Username of owner. If missing or empty, the head appears as a Steve head. Otherwise, used to store or retrieve the downloaded skin in the cache. Need not be a valid player name, but must not be all spaces.',
    },
    'tag.SkullOwner.Properties': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.SkullOwner.Properties',
        defaultMessage: 'The skin properties of the head owner.',
    },
    'tag.SkullOwner.Properties.textures': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.SkullOwner.Properties.textures',
        defaultMessage:
            'The individual texture entries of the head owner, each holding a Base64-encoded JSON object in Value.',
    },
    'tag.effects': {
        id: 'app.lab.command-generator.nbt.desc.item.tag.effects',
        defaultMessage: 'The status effects this suspicious stew has.',
    },
    tool: {
        id: 'app.lab.command-generator.nbt.desc.item.tool',
        defaultMessage: 'If set, this item is considered as a tool.',
    },
    'tool.default_mining_speed': {
        id: 'app.lab.command-generator.nbt.desc.item.tool.default_mining_speed',
        defaultMessage:
            'The default mining speed of this tool, used if no rules override it. Defaults to 1.0.',
    },
    'tool.damage_per_block': {
        id: 'app.lab.command-generator.nbt.desc.item.tool.damage_per_block',
        defaultMessage:
            'The amount of durability to remove each time a block is broken with this tool. Must be a non-negative integer. Defaults to 1.',
    },
    'tool.can_destroy_blocks_in_creative': {
        id: 'app.lab.command-generator.nbt.desc.item.tool.can_destroy_blocks_in_creative',
        defaultMessage:
            'Whether players can break blocks while holding this tool in Creative mode. Defaults to true.',
    },
    'tool.rules': {
        id: 'app.lab.command-generator.nbt.desc.item.tool.rules',
        defaultMessage:
            'A list of rules for the blocks that this tool has a special behavior with. If a field is overridden by multiple matched rules, the one that comes first in the list is chosen.',
    },
    'tool.rules.blocks': {
        id: 'app.lab.command-generator.nbt.desc.item.tool.rules.blocks',
        defaultMessage:
            'The blocks to match with. Can be a block ID or a block tag with a #, or a list of block IDs.',
    },
    'tool.rules.speed': {
        id: 'app.lab.command-generator.nbt.desc.item.tool.rules.speed',
        defaultMessage: 'If the blocks match, overrides the default mining speed. Optional.',
    },
    'tool.rules.correct_for_drops': {
        id: 'app.lab.command-generator.nbt.desc.item.tool.rules.correct_for_drops',
        defaultMessage:
            "If the blocks match, overrides whether or not this tool is considered correct to mine at its most efficient speed, and to drop items if the block's loot table requires it. If not set by any rules, defaults to false. Optional.",
    },
    tooltip_display: {
        id: 'app.lab.command-generator.nbt.desc.item.tooltip_display',
        defaultMessage:
            'Allows the tooltips provided specifically by any given item component to be suppressed.',
    },
    'tooltip_display.hide_tooltip': {
        id: 'app.lab.command-generator.nbt.desc.item.tooltip_display.hide_tooltip',
        defaultMessage: 'If true, the item has no tooltip when hovered.',
    },
    'tooltip_display.hidden_components': {
        id: 'app.lab.command-generator.nbt.desc.item.tooltip_display.hidden_components',
        defaultMessage:
            'The tooltips provided by any component in this list are hidden. If that component provides no tooltip, it has no effect.',
    },
    tooltip_style: {
        id: 'app.lab.command-generator.nbt.desc.item.tooltip_style',
        defaultMessage:
            "The identifier of the custom sprites for the tooltip background and frame which references textures /assets/'<namespace>'/textures/gui/sprites/tooltip/'<id>'_background and /assets/'<namespace>'/textures/gui/sprites/tooltip/'<id>'_frame. Instead of falling back to the default value, invalid specifications will use the missing texture.",
    },
    trim: {
        id: 'app.lab.command-generator.nbt.desc.item.trim',
        defaultMessage:
            "Contains the trim applied to this armor piece. If present on an item, information about the armor trim is added to the item's tooltip.",
    },
    'trim.pattern': {
        id: 'app.lab.command-generator.nbt.desc.item.trim.pattern',
        defaultMessage: 'One trim pattern (an ID, or a new trim pattern definition).',
    },
    'trim.material': {
        id: 'app.lab.command-generator.nbt.desc.item.trim.material',
        defaultMessage: 'One trim material (an ID, or a new trim material definition).',
    },
    unbreakable: {
        id: 'app.lab.command-generator.nbt.desc.item.unbreakable',
        defaultMessage:
            'If present on an item, the item cannot lose durability, its durability bar is hidden, and a blue "Unbreakable" text is added to its tooltip.',
    },
    use_cooldown: {
        id: 'app.lab.command-generator.nbt.desc.item.use_cooldown',
        defaultMessage:
            'If present, this item applies a use cooldown to all items of the same type when it has been used.',
    },
    'use_cooldown.seconds': {
        id: 'app.lab.command-generator.nbt.desc.item.use_cooldown.seconds',
        defaultMessage: 'The use cooldown duration in seconds.',
    },
    'use_cooldown.cooldown_group': {
        id: 'app.lab.command-generator.nbt.desc.item.use_cooldown.cooldown_group',
        defaultMessage:
            'The unique identifier to identify this cooldown group. If present, the item is included in a use cooldown group and no longer shares cooldowns with its base item type, but instead with any other items that are part of the same use cooldown group. Optional.',
    },
    use_effects: {
        id: 'app.lab.command-generator.nbt.desc.item.use_effects',
        defaultMessage:
            'Defines the vibrations and player movement penalties when an item with a continuous use action is being used.',
    },
    use_remainder: {
        id: 'app.lab.command-generator.nbt.desc.item.use_remainder',
        defaultMessage:
            'If present, replaces the item with a remainder item if its stack count has decreased after use.',
    },
    'use_remainder.id': {
        id: 'app.lab.command-generator.nbt.desc.item.use_remainder.id',
        defaultMessage: 'The identifier of the item. Must not be minecraft:air.',
    },
    'use_remainder.count': {
        id: 'app.lab.command-generator.nbt.desc.item.use_remainder.count',
        defaultMessage:
            'Number of items stacked in this inventory slot. Defaults to 1 if not specified.',
    },
    'use_remainder.components': {
        id: 'app.lab.command-generator.nbt.desc.item.use_remainder.components',
        defaultMessage: 'Optional map of additional (non-default) data components.',
    },
    villager_food: {
        id: 'app.lab.command-generator.nbt.desc.item.villager_food',
        defaultMessage: 'If present, villagers will be able to pick up and eat the item.',
    },
    waxed: {
        id: 'app.lab.command-generator.nbt.desc.item.waxed',
        defaultMessage:
            "If present, the block's contents are waxed. Does not apply to blocks with separate waxed variants; for those, use the appropriate block_state instead.",
    },
    weapon: {
        id: 'app.lab.command-generator.nbt.desc.item.weapon',
        defaultMessage:
            'If present, the item acts as a weapon. For attack damage see the attribute_modifiers component.',
    },
    writable_book_content: {
        id: 'app.lab.command-generator.nbt.desc.item.writable_book_content',
        defaultMessage:
            'The contents of this book and quill. Adding this component to any item other than a writable_book does nothing. If this component is removed from a writable_book item, attempting to use the item will still swing the arm and increment its used statistic, but no UI will appear.',
    },
    written_book_content: {
        id: 'app.lab.command-generator.nbt.desc.item.written_book_content',
        defaultMessage:
            'The contents and metadata of this written book. Adding this component to any item other than a written_book does nothing except add text to the item tooltip. If this component is removed from a written_book item, attempting to use the item will still swing the arm and increment its used statistic, but no UI will appear.',
    },
    'written_book_content.pages': {
        id: 'app.lab.command-generator.nbt.desc.item.written_book_content.pages',
        defaultMessage: 'A list of the pages in the book.',
    },
    'written_book_content.title': {
        id: 'app.lab.command-generator.nbt.desc.item.written_book_content.title',
        defaultMessage:
            'The title of this written book. Overrides the base item name. If set to an empty string, it is ignored and does not override the base item name.',
    },
    'written_book_content.title.raw': {
        id: 'app.lab.command-generator.nbt.desc.item.written_book_content.title.raw',
        defaultMessage: 'The plain text title. Has a maximum length of 32 characters.',
    },
    'written_book_content.title.filtered': {
        id: 'app.lab.command-generator.nbt.desc.item.written_book_content.title.filtered',
        defaultMessage:
            'The filtered title. Optional. Shown only to players with chat filter enabled, instead of .',
    },
    'written_book_content.author': {
        id: 'app.lab.command-generator.nbt.desc.item.written_book_content.author',
        defaultMessage: 'The author of this written book. This is shown in the item tooltip.',
    },
    'written_book_content.generation': {
        id: 'app.lab.command-generator.nbt.desc.item.written_book_content.generation',
        defaultMessage:
            'The number of times this written book has been copied. 0 = original, 1 = copy of original, 2 = copy of copy, 3 = tattered. Defaults to 0. If the value is greater than 1, the book cannot be copied. If specified, this is shown in the item tooltip.',
    },
    'written_book_content.resolved': {
        id: 'app.lab.command-generator.nbt.desc.item.written_book_content.resolved',
        defaultMessage:
            "If true, the text components have already been resolved by the server. If false, they are resolved either when the book is opened by an operator, when the book is placed into a lectern by an operator, or when the item stack is written to a lectern's Book tag by a command. Defaults to false.",
    },
})
