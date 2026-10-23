import { defineMessages } from '@modrinth/ui/src/composables/i18n.ts'

import zhCN from '~/locales/launcher-preview/instance-icons.zh-CN'

import { registerPreviewMessages } from './i18n'

registerPreviewMessages('zh-CN', zhCN)

export interface BuiltInInstanceIcon {
    id: string
    name: { id: string; defaultMessage: string }
    url: string
}

/**
 * Port of the app's built-in instance icons (apps/app-frontend/src/helpers/
 * instance-icons.ts): the same ids, names and order, with the bundled images
 * converted to 128px WebP.
 */
const names = defineMessages({
    bread: { id: 'app.instance.icon-picker.icon.bread', defaultMessage: 'Bread' },
    carrot: { id: 'app.instance.icon-picker.icon.carrot', defaultMessage: 'Carrot' },
    'cooked-chicken': {
        id: 'app.instance.icon-picker.icon.cooked-chicken',
        defaultMessage: 'Cooked Chicken',
    },
    'crafting-table': {
        id: 'app.instance.icon-picker.icon.crafting-table',
        defaultMessage: 'Crafting Table',
    },
    'diamond-axe': {
        id: 'app.instance.icon-picker.icon.diamond-axe',
        defaultMessage: 'Diamond Axe',
    },
    'diamond-block': {
        id: 'app.instance.icon-picker.icon.diamond-block',
        defaultMessage: 'Diamond Block',
    },
    'diamond-sword': {
        id: 'app.instance.icon-picker.icon.diamond-sword',
        defaultMessage: 'Diamond Sword',
    },
    'end-stone': { id: 'app.instance.icon-picker.icon.end-stone', defaultMessage: 'End Stone' },
    furnace: { id: 'app.instance.icon-picker.icon.furnace', defaultMessage: 'Furnace' },
    'glass-bottle': {
        id: 'app.instance.icon-picker.icon.glass-bottle',
        defaultMessage: 'Glass Bottle',
    },
    'golden-apple': {
        id: 'app.instance.icon-picker.icon.golden-apple',
        defaultMessage: 'Golden Apple',
    },
    'gold-block': { id: 'app.instance.icon-picker.icon.gold-block', defaultMessage: 'Gold Block' },
    'grass-block': {
        id: 'app.instance.icon-picker.icon.grass-block',
        defaultMessage: 'Grass Block',
    },
    'iron-block': { id: 'app.instance.icon-picker.icon.iron-block', defaultMessage: 'Iron Block' },
    'item-frame': { id: 'app.instance.icon-picker.icon.item-frame', defaultMessage: 'Item Frame' },
    netherrack: { id: 'app.instance.icon-picker.icon.netherrack', defaultMessage: 'Netherrack' },
    'oak-sapling': {
        id: 'app.instance.icon-picker.icon.oak-sapling',
        defaultMessage: 'Oak Sapling',
    },
    stone: { id: 'app.instance.icon-picker.icon.stone', defaultMessage: 'Stone' },
    'totem-of-undying': {
        id: 'app.instance.icon-picker.icon.totem-of-undying',
        defaultMessage: 'Totem of Undying',
    },
    'water-bucket': {
        id: 'app.instance.icon-picker.icon.water-bucket',
        defaultMessage: 'Water Bucket',
    },
    anvil: { id: 'app.instance.icon-picker.icon.anvil', defaultMessage: 'Anvil' },
    fabric: { id: 'app.instance.icon-picker.icon.fabric', defaultMessage: 'Fabric' },
    cleanroom: { id: 'app.instance.icon-picker.icon.cleanroom', defaultMessage: 'Cleanroom' },
    liteloader: { id: 'app.instance.icon-picker.icon.liteloader', defaultMessage: 'LiteLoader' },
    neoforge: { id: 'app.instance.icon-picker.icon.neoforge', defaultMessage: 'NeoForge' },
    quilt: { id: 'app.instance.icon-picker.icon.quilt', defaultMessage: 'Quilt' },
    optifine: { id: 'app.instance.icon-picker.icon.optifine', defaultMessage: 'OptiFine' },
    'modrinth-3d-backpack': {
        id: 'app.instance.icon-picker.modrinth-3d.backpack',
        defaultMessage: 'Backpack',
    },
    'modrinth-3d-beacon': {
        id: 'app.instance.icon-picker.modrinth-3d.beacon',
        defaultMessage: 'Beacon',
    },
    'modrinth-3d-blue-shark': {
        id: 'app.instance.icon-picker.modrinth-3d.blue-shark',
        defaultMessage: 'Blue Shark',
    },
    'modrinth-3d-bookshelf': {
        id: 'app.instance.icon-picker.modrinth-3d.bookshelf',
        defaultMessage: 'Bookshelf',
    },
    'modrinth-3d-brown-bear': {
        id: 'app.instance.icon-picker.modrinth-3d.brown-bear',
        defaultMessage: 'Brown Bear',
    },
    'modrinth-3d-cake': { id: 'app.instance.icon-picker.modrinth-3d.cake', defaultMessage: 'Cake' },
    'modrinth-3d-campfire': {
        id: 'app.instance.icon-picker.modrinth-3d.campfire',
        defaultMessage: 'Campfire',
    },
    'modrinth-3d-chest': {
        id: 'app.instance.icon-picker.modrinth-3d.chest',
        defaultMessage: 'Chest',
    },
    'modrinth-3d-cogwheel': {
        id: 'app.instance.icon-picker.modrinth-3d.cogwheel',
        defaultMessage: 'Cogwheel',
    },
    'modrinth-3d-command-block': {
        id: 'app.instance.icon-picker.modrinth-3d.command-block',
        defaultMessage: 'Command Block',
    },
    'modrinth-3d-cooking-pot': {
        id: 'app.instance.icon-picker.modrinth-3d.cooking-pot',
        defaultMessage: 'Cooking Pot',
    },
    'modrinth-3d-couch': {
        id: 'app.instance.icon-picker.modrinth-3d.couch',
        defaultMessage: 'Couch',
    },
    'modrinth-3d-crafting-table': {
        id: 'app.instance.icon-picker.modrinth-3d.crafting-table',
        defaultMessage: 'Crafting Table',
    },
    'modrinth-3d-creeper': {
        id: 'app.instance.icon-picker.modrinth-3d.creeper',
        defaultMessage: 'Creeper',
    },
    'modrinth-3d-enchanting-table': {
        id: 'app.instance.icon-picker.modrinth-3d.enchanting-table',
        defaultMessage: 'Enchanting Table',
    },
    'modrinth-3d-ender-chest': {
        id: 'app.instance.icon-picker.modrinth-3d.ender-chest',
        defaultMessage: 'Ender Chest',
    },
    'modrinth-3d-ender-dragon': {
        id: 'app.instance.icon-picker.modrinth-3d.ender-dragon',
        defaultMessage: 'Ender Dragon',
    },
    'modrinth-3d-engine': {
        id: 'app.instance.icon-picker.modrinth-3d.engine',
        defaultMessage: 'Engine',
    },
    'modrinth-3d-furnace': {
        id: 'app.instance.icon-picker.modrinth-3d.furnace',
        defaultMessage: 'Furnace',
    },
    'modrinth-3d-gizmo': {
        id: 'app.instance.icon-picker.modrinth-3d.gizmo',
        defaultMessage: 'Gizmo',
    },
    'modrinth-3d-globe': {
        id: 'app.instance.icon-picker.modrinth-3d.globe',
        defaultMessage: 'Globe',
    },
    'modrinth-3d-grass-block': {
        id: 'app.instance.icon-picker.modrinth-3d.grass-block',
        defaultMessage: 'Grass Block',
    },
    'modrinth-3d-lantern': {
        id: 'app.instance.icon-picker.modrinth-3d.lantern',
        defaultMessage: 'Lantern',
    },
    'modrinth-3d-moobloom': {
        id: 'app.instance.icon-picker.modrinth-3d.moobloom',
        defaultMessage: 'Moobloom',
    },
    'modrinth-3d-mr-pack': {
        id: 'app.instance.icon-picker.modrinth-3d.mr-pack',
        defaultMessage: 'Mr Pack',
    },
    'modrinth-3d-orb': {
        id: 'app.instance.icon-picker.modrinth-3d.orb',
        defaultMessage: 'Orb of Origins',
    },
    'modrinth-3d-oxygen-distributor': {
        id: 'app.instance.icon-picker.modrinth-3d.oxygen-distributor',
        defaultMessage: 'Oxygen Distributor',
    },
    'modrinth-3d-pancakes': {
        id: 'app.instance.icon-picker.modrinth-3d.pancakes',
        defaultMessage: 'Pancakes',
    },
    'modrinth-3d-pickaxe': {
        id: 'app.instance.icon-picker.modrinth-3d.pickaxe',
        defaultMessage: 'Pickaxe',
    },
    'modrinth-3d-poke-ball': {
        id: 'app.instance.icon-picker.modrinth-3d.poke-ball',
        defaultMessage: 'Poké Ball',
    },
    'modrinth-3d-redstone-block': {
        id: 'app.instance.icon-picker.modrinth-3d.redstone-block',
        defaultMessage: 'Redstone Block',
    },
    'modrinth-3d-sculk-sensor': {
        id: 'app.instance.icon-picker.modrinth-3d.sculk-sensor',
        defaultMessage: 'Sculk Sensor',
    },
    'modrinth-3d-skeleton': {
        id: 'app.instance.icon-picker.modrinth-3d.skeleton',
        defaultMessage: 'Skeleton',
    },
    'modrinth-3d-skillet': {
        id: 'app.instance.icon-picker.modrinth-3d.skillet',
        defaultMessage: 'Skillet',
    },
    'modrinth-3d-slime-block': {
        id: 'app.instance.icon-picker.modrinth-3d.slime-block',
        defaultMessage: 'Slime Block',
    },
    'modrinth-3d-space-helmet': {
        id: 'app.instance.icon-picker.modrinth-3d.space-helmet',
        defaultMessage: 'Space Helmet',
    },
    'modrinth-3d-sticky-piston': {
        id: 'app.instance.icon-picker.modrinth-3d.sticky-piston',
        defaultMessage: 'Sticky Piston',
    },
    'modrinth-3d-sword': {
        id: 'app.instance.icon-picker.modrinth-3d.sword',
        defaultMessage: 'Sword',
    },
    'modrinth-3d-terminal': {
        id: 'app.instance.icon-picker.modrinth-3d.terminal',
        defaultMessage: 'Terminal',
    },
    'modrinth-3d-tiny-potato': {
        id: 'app.instance.icon-picker.modrinth-3d.tiny-potato',
        defaultMessage: 'Tiny Potato',
    },
    'modrinth-3d-tire': { id: 'app.instance.icon-picker.modrinth-3d.tire', defaultMessage: 'Tire' },
    'modrinth-3d-tnt': { id: 'app.instance.icon-picker.modrinth-3d.tnt', defaultMessage: 'TNT' },
    'modrinth-3d-wrench-rinth': {
        id: 'app.instance.icon-picker.modrinth-3d.wrench-rinth',
        defaultMessage: 'Modrinth Wrench',
    },
    'modrinth-3d-wrench': {
        id: 'app.instance.icon-picker.modrinth-3d.wrench',
        defaultMessage: 'Wrench',
    },
    'modrinth-3d-zombie': {
        id: 'app.instance.icon-picker.modrinth-3d.zombie',
        defaultMessage: 'Zombie',
    },
})

function icon(id: keyof typeof names, url: string): BuiltInInstanceIcon {
    return { id, name: names[id], url }
}

export const builtInInstanceIcons: BuiltInInstanceIcon[] = [
    icon('bread', '/launcher-preview/instance-icons/bread.webp'),
    icon('carrot', '/launcher-preview/instance-icons/carrot.webp'),
    icon('cooked-chicken', '/launcher-preview/instance-icons/cooked-chicken.webp'),
    icon('crafting-table', '/launcher-preview/instance-icons/crafting-table.webp'),
    icon('diamond-axe', '/launcher-preview/instance-icons/diamond-axe.webp'),
    icon('diamond-block', '/launcher-preview/instance-icons/diamond-block.webp'),
    icon('diamond-sword', '/launcher-preview/instance-icons/diamond-sword.webp'),
    icon('end-stone', '/launcher-preview/instance-icons/end-stone.webp'),
    icon('furnace', '/launcher-preview/instance-icons/furnace.webp'),
    icon('glass-bottle', '/launcher-preview/instance-icons/glass-bottle.webp'),
    icon('golden-apple', '/launcher-preview/instance-icons/golden-apple.webp'),
    icon('gold-block', '/launcher-preview/instance-icons/gold-block.webp'),
    icon('grass-block', '/launcher-preview/instance-icons/grass-block.webp'),
    icon('iron-block', '/launcher-preview/instance-icons/iron-block.webp'),
    icon('item-frame', '/launcher-preview/instance-icons/item-frame.webp'),
    icon('netherrack', '/launcher-preview/instance-icons/netherrack.webp'),
    icon('oak-sapling', '/launcher-preview/instance-icons/oak-sapling.webp'),
    icon('stone', '/launcher-preview/instance-icons/stone.webp'),
    icon('totem-of-undying', '/launcher-preview/instance-icons/totem-of-undying.webp'),
    icon('water-bucket', '/launcher-preview/instance-icons/water-bucket.webp'),
    icon('anvil', '/launcher-preview/instance-icons/anvil.webp'),
    icon('fabric', '/launcher-preview/instance-icons/fabric.webp'),
    icon('cleanroom', '/launcher-preview/instance-icons/cleanroom.webp'),
    icon('liteloader', '/launcher-preview/instance-icons/liteloader.webp'),
    icon('neoforge', '/launcher-preview/instance-icons/neoforge.webp'),
    icon('quilt', '/launcher-preview/instance-icons/quilt.webp'),
    icon('optifine', '/launcher-preview/instance-icons/optifine.webp'),
]

export const modrinth3DInstanceIcons: BuiltInInstanceIcon[] = [
    icon('modrinth-3d-backpack', '/launcher-preview/instance-icons/modrinth-3d/backpack.webp'),
    icon('modrinth-3d-beacon', '/launcher-preview/instance-icons/modrinth-3d/beacon.webp'),
    icon('modrinth-3d-blue-shark', '/launcher-preview/instance-icons/modrinth-3d/blue-shark.webp'),
    icon('modrinth-3d-bookshelf', '/launcher-preview/instance-icons/modrinth-3d/bookshelf.webp'),
    icon('modrinth-3d-brown-bear', '/launcher-preview/instance-icons/modrinth-3d/brown-bear.webp'),
    icon('modrinth-3d-cake', '/launcher-preview/instance-icons/modrinth-3d/cake.webp'),
    icon('modrinth-3d-campfire', '/launcher-preview/instance-icons/modrinth-3d/campfire.webp'),
    icon('modrinth-3d-chest', '/launcher-preview/instance-icons/modrinth-3d/chest.webp'),
    icon('modrinth-3d-cogwheel', '/launcher-preview/instance-icons/modrinth-3d/cogwheel.webp'),
    icon(
        'modrinth-3d-command-block',
        '/launcher-preview/instance-icons/modrinth-3d/command-block.webp',
    ),
    icon(
        'modrinth-3d-cooking-pot',
        '/launcher-preview/instance-icons/modrinth-3d/cooking-pot.webp',
    ),
    icon('modrinth-3d-couch', '/launcher-preview/instance-icons/modrinth-3d/couch.webp'),
    icon(
        'modrinth-3d-crafting-table',
        '/launcher-preview/instance-icons/modrinth-3d/crafting-table.webp',
    ),
    icon('modrinth-3d-creeper', '/launcher-preview/instance-icons/modrinth-3d/creeper.webp'),
    icon(
        'modrinth-3d-enchanting-table',
        '/launcher-preview/instance-icons/modrinth-3d/enchanting-table.webp',
    ),
    icon(
        'modrinth-3d-ender-chest',
        '/launcher-preview/instance-icons/modrinth-3d/ender-chest.webp',
    ),
    icon(
        'modrinth-3d-ender-dragon',
        '/launcher-preview/instance-icons/modrinth-3d/ender-dragon.webp',
    ),
    icon('modrinth-3d-engine', '/launcher-preview/instance-icons/modrinth-3d/engine.webp'),
    icon('modrinth-3d-furnace', '/launcher-preview/instance-icons/modrinth-3d/furnace.webp'),
    icon('modrinth-3d-gizmo', '/launcher-preview/instance-icons/modrinth-3d/gizmo.webp'),
    icon('modrinth-3d-globe', '/launcher-preview/instance-icons/modrinth-3d/globe.webp'),
    icon(
        'modrinth-3d-grass-block',
        '/launcher-preview/instance-icons/modrinth-3d/grass-block.webp',
    ),
    icon('modrinth-3d-lantern', '/launcher-preview/instance-icons/modrinth-3d/lantern.webp'),
    icon('modrinth-3d-moobloom', '/launcher-preview/instance-icons/modrinth-3d/moobloom.webp'),
    icon('modrinth-3d-mr-pack', '/launcher-preview/instance-icons/modrinth-3d/mr-pack.webp'),
    icon('modrinth-3d-orb', '/launcher-preview/instance-icons/modrinth-3d/orb.webp'),
    icon(
        'modrinth-3d-oxygen-distributor',
        '/launcher-preview/instance-icons/modrinth-3d/oxygen-distributor.webp',
    ),
    icon('modrinth-3d-pancakes', '/launcher-preview/instance-icons/modrinth-3d/pancakes.webp'),
    icon('modrinth-3d-pickaxe', '/launcher-preview/instance-icons/modrinth-3d/pickaxe.webp'),
    icon('modrinth-3d-poke-ball', '/launcher-preview/instance-icons/modrinth-3d/poke-ball.webp'),
    icon(
        'modrinth-3d-redstone-block',
        '/launcher-preview/instance-icons/modrinth-3d/redstone-block.webp',
    ),
    icon(
        'modrinth-3d-sculk-sensor',
        '/launcher-preview/instance-icons/modrinth-3d/sculk-sensor.webp',
    ),
    icon('modrinth-3d-skeleton', '/launcher-preview/instance-icons/modrinth-3d/skeleton.webp'),
    icon('modrinth-3d-skillet', '/launcher-preview/instance-icons/modrinth-3d/skillet.webp'),
    icon(
        'modrinth-3d-slime-block',
        '/launcher-preview/instance-icons/modrinth-3d/slime-block.webp',
    ),
    icon(
        'modrinth-3d-space-helmet',
        '/launcher-preview/instance-icons/modrinth-3d/space-helmet.webp',
    ),
    icon(
        'modrinth-3d-sticky-piston',
        '/launcher-preview/instance-icons/modrinth-3d/sticky-piston.webp',
    ),
    icon('modrinth-3d-sword', '/launcher-preview/instance-icons/modrinth-3d/sword.webp'),
    icon('modrinth-3d-terminal', '/launcher-preview/instance-icons/modrinth-3d/terminal.webp'),
    icon(
        'modrinth-3d-tiny-potato',
        '/launcher-preview/instance-icons/modrinth-3d/tiny-potato.webp',
    ),
    icon('modrinth-3d-tire', '/launcher-preview/instance-icons/modrinth-3d/tire.webp'),
    icon('modrinth-3d-tnt', '/launcher-preview/instance-icons/modrinth-3d/tnt.webp'),
    icon(
        'modrinth-3d-wrench-rinth',
        '/launcher-preview/instance-icons/modrinth-3d/wrench-rinth.webp',
    ),
    icon('modrinth-3d-wrench', '/launcher-preview/instance-icons/modrinth-3d/wrench.webp'),
    icon('modrinth-3d-zombie', '/launcher-preview/instance-icons/modrinth-3d/zombie.webp'),
]

const builtInInstanceIconMap = new Map(builtInInstanceIcons.map((entry) => [entry.id, entry]))

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

/** The app recognises its bundled icons by content hash; the preview by their folder. */
export function isBuiltInInstanceIcon(iconPath: string | null | undefined): boolean {
    return !!iconPath && iconPath.startsWith('/launcher-preview/instance-icons/')
}

export function getDisplayInstanceIcon(
    iconPath: string | null | undefined,
    loader: string | null | undefined,
): { url: string | null; frameless: boolean } {
    const fallbackIcon = getLoaderInstanceIcon(loader)
    return {
        url: iconPath || (fallbackIcon?.url ?? null),
        frameless: isBuiltInInstanceIcon(iconPath) || !!fallbackIcon,
    }
}
