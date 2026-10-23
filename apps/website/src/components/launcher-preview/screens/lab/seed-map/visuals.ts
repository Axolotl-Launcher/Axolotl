import CompassIcon from '@modrinth/assets/icons/compass.svg?component'
import DatabaseIcon from '@modrinth/assets/icons/database.svg?component'
import GridIcon from '@modrinth/assets/icons/grid.svg?component'
import HomeIcon from '@modrinth/assets/icons/home.svg?component'
import LandmarkIcon from '@modrinth/assets/icons/landmark.svg?component'
import LayersIcon from '@modrinth/assets/icons/layers.svg?component'
import PickaxeIcon from '@modrinth/assets/icons/pickaxe.svg?component'
import PinIcon from '@modrinth/assets/icons/pin.svg?component'
import TagCategoryFlagIcon from '@modrinth/assets/icons/tags/categories/flag.svg?component'
import WorldIcon from '@modrinth/assets/icons/world.svg?component'
import type { Component } from 'vue'

import type { SeedMapFeatureKind } from './features.ts'

export const SEED_MAP_STRUCTURE_ASSET_ROOT = '/launcher-preview/seed-map-assets/structures'

export const SEED_MAP_END_CITY_IMAGE_SOURCES = {
    ship: `${SEED_MAP_STRUCTURE_ASSET_ROOT}/end_city_ship.webp`,
    noShip: `${SEED_MAP_STRUCTURE_ASSET_ROOT}/end_city_no_ship.webp`,
} as const

export const SEED_MAP_FEATURE_ICONS: Record<SeedMapFeatureKind, Component> = {
    village: HomeIcon,
    outpost: TagCategoryFlagIcon,
    shipwreck: CompassIcon,
    monument: LandmarkIcon,
    mansion: HomeIcon,
    'ancient-city': DatabaseIcon,
    'trail-ruins': LayersIcon,
    'trial-chambers': LandmarkIcon,
    'ruined-portal': WorldIcon,
    stronghold: CompassIcon,
    'slime-chunk': GridIcon,
    'desert-pyramid': LandmarkIcon,
    'jungle-temple': LandmarkIcon,
    'swamp-hut': HomeIcon,
    igloo: HomeIcon,
    'ocean-ruin': LayersIcon,
    'buried-treasure': PinIcon,
    mineshaft: PickaxeIcon,
    'desert-well': CompassIcon,
    geode: DatabaseIcon,
    fortress: LandmarkIcon,
    bastion: HomeIcon,
    'end-city': HomeIcon,
    'end-gateway': WorldIcon,
    'abandoned-camp': HomeIcon,
    'nether-fossil': LayersIcon,
}

export const SEED_MAP_FEATURE_IMAGE_SOURCES: Partial<Record<SeedMapFeatureKind, string>> = {
    village: `${SEED_MAP_STRUCTURE_ASSET_ROOT}/village.webp`,
    outpost: `${SEED_MAP_STRUCTURE_ASSET_ROOT}/pillager_outpost.webp`,
    shipwreck: `${SEED_MAP_STRUCTURE_ASSET_ROOT}/shipwreck.webp`,
    monument: `${SEED_MAP_STRUCTURE_ASSET_ROOT}/ocean_monument.webp`,
    mansion: `${SEED_MAP_STRUCTURE_ASSET_ROOT}/woodland_mansion.webp`,
    'ancient-city': `${SEED_MAP_STRUCTURE_ASSET_ROOT}/ancient_city.webp`,
    'trail-ruins': `${SEED_MAP_STRUCTURE_ASSET_ROOT}/trail_ruin.webp`,
    'trial-chambers': `${SEED_MAP_STRUCTURE_ASSET_ROOT}/trial_chambers.webp`,
    'ruined-portal': `${SEED_MAP_STRUCTURE_ASSET_ROOT}/ruined_portal.webp`,
    stronghold: `${SEED_MAP_STRUCTURE_ASSET_ROOT}/stronghold.webp`,
    'slime-chunk': `${SEED_MAP_STRUCTURE_ASSET_ROOT}/slime_chunks.webp`,
    'desert-pyramid': `${SEED_MAP_STRUCTURE_ASSET_ROOT}/desert_pyramid.webp`,
    'jungle-temple': `${SEED_MAP_STRUCTURE_ASSET_ROOT}/jungle_temple.webp`,
    'swamp-hut': `${SEED_MAP_STRUCTURE_ASSET_ROOT}/swamp_hut.webp`,
    igloo: `${SEED_MAP_STRUCTURE_ASSET_ROOT}/igloo.webp`,
    'ocean-ruin': `${SEED_MAP_STRUCTURE_ASSET_ROOT}/ocean_ruin.webp`,
    'buried-treasure': `${SEED_MAP_STRUCTURE_ASSET_ROOT}/buried_treasure.webp`,
    mineshaft: `${SEED_MAP_STRUCTURE_ASSET_ROOT}/mineshaft.webp`,
    'desert-well': `${SEED_MAP_STRUCTURE_ASSET_ROOT}/desert_well.webp`,
    geode: `${SEED_MAP_STRUCTURE_ASSET_ROOT}/amethyst_geode.webp`,
    fortress: `${SEED_MAP_STRUCTURE_ASSET_ROOT}/nether_fortress.webp`,
    bastion: `${SEED_MAP_STRUCTURE_ASSET_ROOT}/bastion_remnant.webp`,
    'end-city': `${SEED_MAP_STRUCTURE_ASSET_ROOT}/end_city.webp`,
    'end-gateway': `${SEED_MAP_STRUCTURE_ASSET_ROOT}/end_gateway.webp`,
    'abandoned-camp': `${SEED_MAP_STRUCTURE_ASSET_ROOT}/abandoned_camp.png`,
    'nether-fossil': `${SEED_MAP_STRUCTURE_ASSET_ROOT}/nether_fossil.png`,
}

export const SEED_MAP_FEATURE_COLORS: Record<SeedMapFeatureKind, string> = {
    village: '#D58B35',
    outpost: '#C94B52',
    shipwreck: '#3B9CC6',
    monument: '#34A99A',
    mansion: '#8C67C5',
    'ancient-city': '#6677D9',
    'trail-ruins': '#C96F3C',
    'trial-chambers': '#4DAE72',
    'ruined-portal': '#C95B92',
    stronghold: '#8793A4',
    'slime-chunk': '#83B735',
    'desert-pyramid': '#D0A23E',
    'jungle-temple': '#4B9A56',
    'swamp-hut': '#6A8D58',
    igloo: '#80B6CA',
    'ocean-ruin': '#4F9EAB',
    'buried-treasure': '#E0B84E',
    mineshaft: '#9B7657',
    'desert-well': '#D6AF62',
    geode: '#9969C7',
    fortress: '#A94A51',
    bastion: '#7E6658',
    'end-city': '#A57BC5',
    'abandoned-camp': '#D58B35',
    'nether-fossil': '#D0A23E',
    'end-gateway': '#70559F',
}
