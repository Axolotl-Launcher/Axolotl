/**
 * 物品组件的说明文案。
 *
 * 目录数据里的 `desc` 是 wiki 英文原文（搜索也会用到），但界面要给两种语言，
 * 所以显示走这里的消息描述符。id 必须写成字面量，formatjs 才能静态抽取。
 */

import { defineMessages } from '@modrinth/ui'

export const ITEM_COMPONENT_DESCRIPTIONS = defineMessages({
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
    can_break: {
        id: 'app.lab.command-generator.nbt.desc.item.can_break',
        defaultMessage:
            'When present, the player holding the item can break the specified blocks in Adventure mode.',
    },
    can_place_on: {
        id: 'app.lab.command-generator.nbt.desc.item.can_place_on',
        defaultMessage:
            'Determines which blocks that blocks with this component can be placed against in Adventure mode. If defined as a compound, corresponds to [NBT Compound / JSON Object] A single block predicate. Items are listed in the tooltip in the order specified (when viewed in inventory).',
    },
    charged_projectiles: {
        id: 'app.lab.command-generator.nbt.desc.item.charged_projectiles',
        defaultMessage:
            'The items loaded as projectiles into this crossbow. If there are item stacks in this component, the item name for item stack is listed in the tooltip of the item as "Projectile: [\'<name>\']", and any duplicate item stack entries are grouped together as "Projectile: N x [\'<name>\']". If not present, this crossbow is not charged.',
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
    container: {
        id: 'app.lab.command-generator.nbt.desc.item.container',
        defaultMessage:
            "The items contained in this container's slots. If present on an item, the first 5 item slots in the container are listed in the tooltip, and the number of any extra slots are listed as \"and '<N>' more...\" underneath (only seen on shulker boxes in normal gameplay or ctrl-middle-clicked containers in creative mode). This component supports up to 256 item slots but, when present on some block entities, the number of slots may be artificially limited based on the number of slots the block needs (e.g. 1 for decorated pots, and 27 for chests).",
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
            'The RGB color of the tint applied to a dyed item, typically as a result of combining the item with dyes in a minecraft:crafting_dyed type crafting recipe. More generally, this component is used as the minecraft:dye color provider in item model definitions (so only item models that explicitly use this color provider will be tinted). If present on an item, gray text is added to the tooltip showing either an italic "Dyed" when advanced tooltips is off, or a non-italic "Color: #\'<hex color code>\'" instead when advanced tooltips is on. The color is stored as an integer which packs the color\'s red, green and blue channels using this formula:( Red ＜＜ 16) + ( Green ＜＜ 8) + Blue',
    },
    enchantable: {
        id: 'app.lab.command-generator.nbt.desc.item.enchantable',
        defaultMessage:
            'If this and minecraft:enchantments are present on an item, and applicable enchantments are available, the item can be enchanted in an enchanting table.',
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
            'NBT applied to an entity when created from an item. If this component is present on an item, the game\'s difficulty is set to peaceful and the [String] id in this component is that of a hostile mob, red text is added to the tooltip that says "Disabled in Peaceful". If the item is a spawn egg and this [String] id is falling_block, command_block_minecart, or spawner_minecart, this component adds a red message to the item\'s tooltip (visible for operator players only) warning the player that using it may result in command execution.',
    },
    equippable: {
        id: 'app.lab.command-generator.nbt.desc.item.equippable',
        defaultMessage: 'If present, this item can be equipped in the specified slot.',
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
    glider: {
        id: 'app.lab.command-generator.nbt.desc.item.glider',
        defaultMessage:
            "If present, this item allows living entities to glide (as with elytra) when equipped. If the item is damageable, it will only allow entities to glide if it has minecraft:damage ＜ minecraft:max_damage − 1. Every second while an entity is gliding, the game will select a random equipped glider item that isn't too damaged to function and try to apply one point of durability damage.",
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
            'Provides the textures needed for rendering player models (player head blocks, player head item models, and mannequins) and player face sprites. Each texture can be provided either as the namespaced path to a texture in a resource pack, or from a player profile. A player profile can be specified either as player information whose profile properties must be requested on the fly, or a list of already-resolved profile properties.',
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
    suspicious_stew_effects: {
        id: 'app.lab.command-generator.nbt.desc.item.suspicious_stew_effects',
        defaultMessage:
            "A list of unamplified mob effects given to the player or mob upon consuming this item (normally a suspicious stew). minecraft:potion_duration_scale is ignored. When present on an item, text is added to the item's tooltip only when it is inside of the item selection screen.",
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
    tool: {
        id: 'app.lab.command-generator.nbt.desc.item.tool',
        defaultMessage: 'If set, this item is considered as a tool.',
    },
    tooltip_display: {
        id: 'app.lab.command-generator.nbt.desc.item.tooltip_display',
        defaultMessage:
            'Allows the tooltips provided specifically by any given item component to be suppressed.',
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
})
