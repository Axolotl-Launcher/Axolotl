import { defineMessage, defineMessages, type MessageDescriptor } from '@modrinth/ui'
import { convertFileSrc } from '@tauri-apps/api/core'

import { isBuiltInInstanceIcon } from './instance-icon-frame'

export interface BuiltInInstanceIcon {
    id: string
    name: MessageDescriptor
    url: string
}

export const builtInInstanceIcons: BuiltInInstanceIcon[] = [
    {
        id: 'bread',
        name: defineMessage({ id: 'app.instance.icon-picker.icon.bread', defaultMessage: 'Bread' }),
        url: new URL('../assets/instance-icons/bread.png', import.meta.url).href,
    },
    {
        id: 'carrot',
        name: defineMessage({
            id: 'app.instance.icon-picker.icon.carrot',
            defaultMessage: 'Carrot',
        }),
        url: new URL('../assets/instance-icons/carrot.png', import.meta.url).href,
    },
    {
        id: 'cooked-chicken',
        name: defineMessage({
            id: 'app.instance.icon-picker.icon.cooked-chicken',
            defaultMessage: 'Cooked Chicken',
        }),
        url: new URL('../assets/instance-icons/cooked-chicken.png', import.meta.url).href,
    },
    {
        id: 'crafting-table',
        name: defineMessage({
            id: 'app.instance.icon-picker.icon.crafting-table',
            defaultMessage: 'Crafting Table',
        }),
        url: new URL('../assets/instance-icons/crafting-table.webp', import.meta.url).href,
    },
    {
        id: 'diamond-axe',
        name: defineMessage({
            id: 'app.instance.icon-picker.icon.diamond-axe',
            defaultMessage: 'Diamond Axe',
        }),
        url: new URL('../assets/instance-icons/diamond-axe.png', import.meta.url).href,
    },
    {
        id: 'diamond-block',
        name: defineMessage({
            id: 'app.instance.icon-picker.icon.diamond-block',
            defaultMessage: 'Diamond Block',
        }),
        url: new URL('../assets/instance-icons/diamond-block.webp', import.meta.url).href,
    },
    {
        id: 'diamond-sword',
        name: defineMessage({
            id: 'app.instance.icon-picker.icon.diamond-sword',
            defaultMessage: 'Diamond Sword',
        }),
        url: new URL('../assets/instance-icons/diamond-sword.png', import.meta.url).href,
    },
    {
        id: 'end-stone',
        name: defineMessage({
            id: 'app.instance.icon-picker.icon.end-stone',
            defaultMessage: 'End Stone',
        }),
        url: new URL('../assets/instance-icons/end-stone.webp', import.meta.url).href,
    },
    {
        id: 'furnace',
        name: defineMessage({
            id: 'app.instance.icon-picker.icon.furnace',
            defaultMessage: 'Furnace',
        }),
        url: new URL('../assets/instance-icons/furnace.webp', import.meta.url).href,
    },
    {
        id: 'glass-bottle',
        name: defineMessage({
            id: 'app.instance.icon-picker.icon.glass-bottle',
            defaultMessage: 'Glass Bottle',
        }),
        url: new URL('../assets/instance-icons/glass-bottle.png', import.meta.url).href,
    },
    {
        id: 'golden-apple',
        name: defineMessage({
            id: 'app.instance.icon-picker.icon.golden-apple',
            defaultMessage: 'Golden Apple',
        }),
        url: new URL('../assets/instance-icons/golden-apple.png', import.meta.url).href,
    },
    {
        id: 'gold-block',
        name: defineMessage({
            id: 'app.instance.icon-picker.icon.gold-block',
            defaultMessage: 'Gold Block',
        }),
        url: new URL('../assets/instance-icons/gold-block.webp', import.meta.url).href,
    },
    {
        id: 'grass-block',
        name: defineMessage({
            id: 'app.instance.icon-picker.icon.grass-block',
            defaultMessage: 'Grass Block',
        }),
        url: new URL('../assets/instance-icons/grass-block.webp', import.meta.url).href,
    },
    {
        id: 'iron-block',
        name: defineMessage({
            id: 'app.instance.icon-picker.icon.iron-block',
            defaultMessage: 'Iron Block',
        }),
        url: new URL('../assets/instance-icons/iron-block.webp', import.meta.url).href,
    },
    {
        id: 'item-frame',
        name: defineMessage({
            id: 'app.instance.icon-picker.icon.item-frame',
            defaultMessage: 'Item Frame',
        }),
        url: new URL('../assets/instance-icons/item-frame.png', import.meta.url).href,
    },
    {
        id: 'netherrack',
        name: defineMessage({
            id: 'app.instance.icon-picker.icon.netherrack',
            defaultMessage: 'Netherrack',
        }),
        url: new URL('../assets/instance-icons/netherrack.webp', import.meta.url).href,
    },
    {
        id: 'oak-sapling',
        name: defineMessage({
            id: 'app.instance.icon-picker.icon.oak-sapling',
            defaultMessage: 'Oak Sapling',
        }),
        url: new URL('../assets/instance-icons/oak-sapling.png', import.meta.url).href,
    },
    {
        id: 'stone',
        name: defineMessage({ id: 'app.instance.icon-picker.icon.stone', defaultMessage: 'Stone' }),
        url: new URL('../assets/instance-icons/stone.webp', import.meta.url).href,
    },
    {
        id: 'totem-of-undying',
        name: defineMessage({
            id: 'app.instance.icon-picker.icon.totem-of-undying',
            defaultMessage: 'Totem of Undying',
        }),
        url: new URL('../assets/instance-icons/totem-of-undying.png', import.meta.url).href,
    },
    {
        id: 'water-bucket',
        name: defineMessage({
            id: 'app.instance.icon-picker.icon.water-bucket',
            defaultMessage: 'Water Bucket',
        }),
        url: new URL('../assets/instance-icons/water-bucket.png', import.meta.url).href,
    },
    {
        id: 'anvil',
        name: defineMessage({
            id: 'app.instance.icon-picker.icon.anvil',
            defaultMessage: 'Anvil',
        }),
        url: new URL('../assets/instance-icons/anvil.png', import.meta.url).href,
    },
    {
        id: 'fabric',
        name: defineMessage({
            id: 'app.instance.icon-picker.icon.fabric',
            defaultMessage: 'Fabric',
        }),
        url: new URL('../assets/instance-icons/Fabric.png', import.meta.url).href,
    },
    {
        id: 'cleanroom',
        name: defineMessage({
            id: 'app.instance.icon-picker.icon.cleanroom',
            defaultMessage: 'Cleanroom',
        }),
        url: new URL('../assets/instance-icons/Cleanroom.webp', import.meta.url).href,
    },
    {
        id: 'liteloader',
        name: defineMessage({
            id: 'app.instance.icon-picker.icon.liteloader',
            defaultMessage: 'LiteLoader',
        }),
        url: new URL('../assets/instance-icons/LiteLoader.png', import.meta.url).href,
    },
    {
        id: 'neoforge',
        name: defineMessage({
            id: 'app.instance.icon-picker.icon.neoforge',
            defaultMessage: 'NeoForge',
        }),
        url: new URL('../assets/instance-icons/NeoForge.png', import.meta.url).href,
    },
    {
        id: 'quilt',
        name: defineMessage({
            id: 'app.instance.icon-picker.icon.quilt',
            defaultMessage: 'Quilt',
        }),
        url: new URL('../assets/instance-icons/Quilt.png', import.meta.url).href,
    },
    {
        id: 'optifine',
        name: defineMessage({
            id: 'app.instance.icon-picker.icon.optifine',
            defaultMessage: 'OptiFine',
        }),
        url: new URL('../assets/instance-icons/OptiFine.webp', import.meta.url).href,
    },
]

const modrinth3DIconAssets = import.meta.glob<string>(
    '../assets/instance-icons/modrinth-3d/*.{png,webp}',
    {
        eager: true,
        query: '?url',
        import: 'default',
    },
)

const modrinth3DNames = defineMessages({
    backpack: { id: 'app.instance.icon-picker.modrinth-3d.backpack', defaultMessage: 'Backpack' },
    beacon: { id: 'app.instance.icon-picker.modrinth-3d.beacon', defaultMessage: 'Beacon' },
    blueShark: {
        id: 'app.instance.icon-picker.modrinth-3d.blue-shark',
        defaultMessage: 'Blue Shark',
    },
    bookshelf: {
        id: 'app.instance.icon-picker.modrinth-3d.bookshelf',
        defaultMessage: 'Bookshelf',
    },
    brownBear: {
        id: 'app.instance.icon-picker.modrinth-3d.brown-bear',
        defaultMessage: 'Brown Bear',
    },
    cake: { id: 'app.instance.icon-picker.modrinth-3d.cake', defaultMessage: 'Cake' },
    campfire: { id: 'app.instance.icon-picker.modrinth-3d.campfire', defaultMessage: 'Campfire' },
    chest: { id: 'app.instance.icon-picker.modrinth-3d.chest', defaultMessage: 'Chest' },
    cogwheel: { id: 'app.instance.icon-picker.modrinth-3d.cogwheel', defaultMessage: 'Cogwheel' },
    commandBlock: {
        id: 'app.instance.icon-picker.modrinth-3d.command-block',
        defaultMessage: 'Command Block',
    },
    cookingPot: {
        id: 'app.instance.icon-picker.modrinth-3d.cooking-pot',
        defaultMessage: 'Cooking Pot',
    },
    couch: { id: 'app.instance.icon-picker.modrinth-3d.couch', defaultMessage: 'Couch' },
    craftingTable: {
        id: 'app.instance.icon-picker.modrinth-3d.crafting-table',
        defaultMessage: 'Crafting Table',
    },
    creeper: { id: 'app.instance.icon-picker.modrinth-3d.creeper', defaultMessage: 'Creeper' },
    enchantingTable: {
        id: 'app.instance.icon-picker.modrinth-3d.enchanting-table',
        defaultMessage: 'Enchanting Table',
    },
    enderChest: {
        id: 'app.instance.icon-picker.modrinth-3d.ender-chest',
        defaultMessage: 'Ender Chest',
    },
    enderDragon: {
        id: 'app.instance.icon-picker.modrinth-3d.ender-dragon',
        defaultMessage: 'Ender Dragon',
    },
    engine: { id: 'app.instance.icon-picker.modrinth-3d.engine', defaultMessage: 'Engine' },
    furnace: { id: 'app.instance.icon-picker.modrinth-3d.furnace', defaultMessage: 'Furnace' },
    gizmo: { id: 'app.instance.icon-picker.modrinth-3d.gizmo', defaultMessage: 'Gizmo' },
    globe: { id: 'app.instance.icon-picker.modrinth-3d.globe', defaultMessage: 'Globe' },
    grassBlock: {
        id: 'app.instance.icon-picker.modrinth-3d.grass-block',
        defaultMessage: 'Grass Block',
    },
    lantern: { id: 'app.instance.icon-picker.modrinth-3d.lantern', defaultMessage: 'Lantern' },
    moobloom: { id: 'app.instance.icon-picker.modrinth-3d.moobloom', defaultMessage: 'Moobloom' },
    mrPack: { id: 'app.instance.icon-picker.modrinth-3d.mr-pack', defaultMessage: 'Mr Pack' },
    orb: { id: 'app.instance.icon-picker.modrinth-3d.orb', defaultMessage: 'Orb of Origins' },
    oxygenDistributor: {
        id: 'app.instance.icon-picker.modrinth-3d.oxygen-distributor',
        defaultMessage: 'Oxygen Distributor',
    },
    pancakes: { id: 'app.instance.icon-picker.modrinth-3d.pancakes', defaultMessage: 'Pancakes' },
    pickaxe: { id: 'app.instance.icon-picker.modrinth-3d.pickaxe', defaultMessage: 'Pickaxe' },
    pokeBall: { id: 'app.instance.icon-picker.modrinth-3d.poke-ball', defaultMessage: 'Poké Ball' },
    redstoneBlock: {
        id: 'app.instance.icon-picker.modrinth-3d.redstone-block',
        defaultMessage: 'Redstone Block',
    },
    sculkSensor: {
        id: 'app.instance.icon-picker.modrinth-3d.sculk-sensor',
        defaultMessage: 'Sculk Sensor',
    },
    skeleton: { id: 'app.instance.icon-picker.modrinth-3d.skeleton', defaultMessage: 'Skeleton' },
    skillet: { id: 'app.instance.icon-picker.modrinth-3d.skillet', defaultMessage: 'Skillet' },
    slimeBlock: {
        id: 'app.instance.icon-picker.modrinth-3d.slime-block',
        defaultMessage: 'Slime Block',
    },
    spaceHelmet: {
        id: 'app.instance.icon-picker.modrinth-3d.space-helmet',
        defaultMessage: 'Space Helmet',
    },
    stickyPiston: {
        id: 'app.instance.icon-picker.modrinth-3d.sticky-piston',
        defaultMessage: 'Sticky Piston',
    },
    sword: { id: 'app.instance.icon-picker.modrinth-3d.sword', defaultMessage: 'Sword' },
    terminal: { id: 'app.instance.icon-picker.modrinth-3d.terminal', defaultMessage: 'Terminal' },
    tinyPotato: {
        id: 'app.instance.icon-picker.modrinth-3d.tiny-potato',
        defaultMessage: 'Tiny Potato',
    },
    tire: { id: 'app.instance.icon-picker.modrinth-3d.tire', defaultMessage: 'Tire' },
    tnt: { id: 'app.instance.icon-picker.modrinth-3d.tnt', defaultMessage: 'TNT' },
    wrenchRinth: {
        id: 'app.instance.icon-picker.modrinth-3d.wrench-rinth',
        defaultMessage: 'Modrinth Wrench',
    },
    wrench: { id: 'app.instance.icon-picker.modrinth-3d.wrench', defaultMessage: 'Wrench' },
    zombie: { id: 'app.instance.icon-picker.modrinth-3d.zombie', defaultMessage: 'Zombie' },
})

function modrinth3DIcon(
    id: string,
    name: MessageDescriptor,
    fileName: string,
): BuiltInInstanceIcon {
    const assetPath = `../assets/instance-icons/modrinth-3d/${fileName}`
    const url = modrinth3DIconAssets[assetPath]
    if (!url) throw new Error(`Missing Modrinth 3D instance icon: ${fileName}`)

    return {
        id: `modrinth-3d-${id}`,
        name,
        url,
    }
}

export const modrinth3DInstanceIcons: BuiltInInstanceIcon[] = [
    modrinth3DIcon('backpack', modrinth3DNames.backpack, 'backpack.webp'),
    modrinth3DIcon('beacon', modrinth3DNames.beacon, 'beacon.webp'),
    modrinth3DIcon('blue-shark', modrinth3DNames.blueShark, 'blue-shark.webp'),
    modrinth3DIcon('bookshelf', modrinth3DNames.bookshelf, 'bookshelf.webp'),
    modrinth3DIcon('brown-bear', modrinth3DNames.brownBear, 'brown-bear.webp'),
    modrinth3DIcon('cake', modrinth3DNames.cake, 'cake.webp'),
    modrinth3DIcon('campfire', modrinth3DNames.campfire, 'campfire.webp'),
    modrinth3DIcon('chest', modrinth3DNames.chest, 'chest.webp'),
    modrinth3DIcon('cogwheel', modrinth3DNames.cogwheel, 'cogwheel.webp'),
    modrinth3DIcon('command-block', modrinth3DNames.commandBlock, 'command-block.webp'),
    modrinth3DIcon('cooking-pot', modrinth3DNames.cookingPot, 'cooking-pot.webp'),
    modrinth3DIcon('couch', modrinth3DNames.couch, 'couch.webp'),
    modrinth3DIcon('crafting-table', modrinth3DNames.craftingTable, 'crafting-table.webp'),
    modrinth3DIcon('creeper', modrinth3DNames.creeper, 'creeper.webp'),
    modrinth3DIcon('enchanting-table', modrinth3DNames.enchantingTable, 'enchanting-table.webp'),
    modrinth3DIcon('ender-chest', modrinth3DNames.enderChest, 'ender-chest.webp'),
    modrinth3DIcon('ender-dragon', modrinth3DNames.enderDragon, 'ender-dragon.webp'),
    modrinth3DIcon('engine', modrinth3DNames.engine, 'engine.webp'),
    modrinth3DIcon('furnace', modrinth3DNames.furnace, 'furnace.webp'),
    modrinth3DIcon('gizmo', modrinth3DNames.gizmo, 'gizmo.webp'),
    modrinth3DIcon('globe', modrinth3DNames.globe, 'globe.webp'),
    modrinth3DIcon('grass-block', modrinth3DNames.grassBlock, 'grass-block.webp'),
    modrinth3DIcon('lantern', modrinth3DNames.lantern, 'lantern.webp'),
    modrinth3DIcon('moobloom', modrinth3DNames.moobloom, 'moobloom.webp'),
    modrinth3DIcon('mr-pack', modrinth3DNames.mrPack, 'mr-pack.webp'),
    modrinth3DIcon('orb', modrinth3DNames.orb, 'orb.webp'),
    modrinth3DIcon(
        'oxygen-distributor',
        modrinth3DNames.oxygenDistributor,
        'oxygen-distributor.webp',
    ),
    modrinth3DIcon('pancakes', modrinth3DNames.pancakes, 'pancakes.webp'),
    modrinth3DIcon('pickaxe', modrinth3DNames.pickaxe, 'pickaxe.webp'),
    modrinth3DIcon('poke-ball', modrinth3DNames.pokeBall, 'poke-ball.webp'),
    modrinth3DIcon('redstone-block', modrinth3DNames.redstoneBlock, 'redstone-block.webp'),
    modrinth3DIcon('sculk-sensor', modrinth3DNames.sculkSensor, 'sculk-sensor.webp'),
    modrinth3DIcon('skeleton', modrinth3DNames.skeleton, 'skeleton.webp'),
    modrinth3DIcon('skillet', modrinth3DNames.skillet, 'skillet.webp'),
    modrinth3DIcon('slime-block', modrinth3DNames.slimeBlock, 'slime-block.webp'),
    modrinth3DIcon('space-helmet', modrinth3DNames.spaceHelmet, 'space-helmet.webp'),
    modrinth3DIcon('sticky-piston', modrinth3DNames.stickyPiston, 'sticky-piston.webp'),
    modrinth3DIcon('sword', modrinth3DNames.sword, 'sword.webp'),
    modrinth3DIcon('terminal', modrinth3DNames.terminal, 'terminal.webp'),
    modrinth3DIcon('tiny-potato', modrinth3DNames.tinyPotato, 'tiny-potato.webp'),
    modrinth3DIcon('tire', modrinth3DNames.tire, 'tire.webp'),
    modrinth3DIcon('tnt', modrinth3DNames.tnt, 'tnt.webp'),
    modrinth3DIcon('wrench-rinth', modrinth3DNames.wrenchRinth, 'wrench-rinth.webp'),
    modrinth3DIcon('wrench', modrinth3DNames.wrench, 'wrench.webp'),
    modrinth3DIcon('zombie', modrinth3DNames.zombie, 'zombie.webp'),
]

const builtInInstanceIconMap = new Map(builtInInstanceIcons.map((icon) => [icon.id, icon]))

const loaderIconIds: Record<string, string> = {
    vanilla: 'grass-block',
    fabric: 'fabric',
    forge: 'anvil',
    neoforge: 'neoforge',
    quilt: 'quilt',
    optifine: 'optifine',
    cleanroom: 'cleanroom',
    lite_loader: 'liteloader',
    legacy_fabric: 'fabric',
    babric: 'fabric',
}

export interface DisplayInstanceIcon {
    url: string | null
    frameless: boolean
}

export function getLoaderInstanceIcon(
    loader: string | null | undefined,
): BuiltInInstanceIcon | undefined {
    if (!loader) return undefined
    const iconId = loaderIconIds[loader]
    return (
        (iconId ? builtInInstanceIconMap.get(iconId) : undefined) ??
        builtInInstanceIconMap.get('stone')
    )
}

export function getDisplayInstanceIcon(
    iconPath: string | null | undefined,
    loader: string | null | undefined,
): DisplayInstanceIcon {
    const fallbackIcon = getLoaderInstanceIcon(loader)
    return {
        url: iconPath ? convertFileSrc(iconPath) : (fallbackIcon?.url ?? null),
        frameless: isBuiltInInstanceIcon(iconPath) || !!fallbackIcon,
    }
}
