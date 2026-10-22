import './meta'

import type { RouteRecordRaw } from 'vue-router'

/**
 * Lab hub is a listing page without a RouterView outlet, so tools stay sibling
 * records (same URLs as before) grouped in this domain module.
 */
export const labRoutes: RouteRecordRaw[] = [
    {
        path: '/lab',
        name: 'Lab',
        component: () => import('@/pages/Lab.vue'),
        meta: {
            breadcrumb: [{ name: 'Lab' }],
            discordActivity: 'Messing with labs...',
        },
    },
    {
        path: '/lab/skin-editor',
        name: 'SkinEditor',
        component: () => import('@/pages/LabSkinEditor.vue'),
        meta: {
            breadcrumb: [{ name: 'Lab', link: '/lab' }, { name: 'Skin editor' }],
            discordActivity: 'Editing a skin...',
        },
    },
    {
        path: '/lab/gradient-text',
        name: 'GradientTextGenerator',
        component: () => import('@/pages/LabGradientText.vue'),
        meta: {
            breadcrumb: [{ name: 'Lab', link: '/lab' }, { name: 'Gradient text generator' }],
            discordActivity: 'Messing with labs...',
        },
    },
    {
        path: '/lab/recipe-generator',
        name: 'RecipeGenerator',
        component: () => import('@/pages/LabRecipeGenerator.vue'),
        meta: {
            breadcrumb: [{ name: 'Lab', link: '/lab' }, { name: 'Recipe generator' }],
            discordActivity: 'Messing with labs...',
        },
    },
    {
        path: '/lab/seed-map',
        name: 'SeedMap',
        component: () => import('@/pages/LabSeedMap.vue'),
        meta: {
            breadcrumb: [{ name: 'Lab', link: '/lab' }, { name: 'Seed map' }],
            discordActivity: 'Messing with labs...',
        },
    },
    {
        path: '/lab/schematic-preview',
        name: 'SchematicWorkshop',
        component: () => import('@/pages/LabSchematicPreview.vue'),
        meta: {
            breadcrumb: [{ name: 'Lab', link: '/lab' }, { name: 'Schematic workshop' }],
            discordActivity: 'Messing with labs...',
        },
    },
    {
        path: '/lab/mod-translation',
        name: 'ModTranslation',
        component: () => import('@/pages/LabModTranslation.vue'),
        meta: {
            breadcrumb: [{ name: 'Lab', link: '/lab' }, { name: 'Mod translation' }],
            discordActivity: 'Messing with labs...',
        },
    },
    {
        path: '/lab/command-generator',
        name: 'CommandGenerator',
        component: () => import('@/pages/LabCommandGenerator.vue'),
        meta: {
            breadcrumb: [{ name: 'Lab', link: '/lab' }, { name: 'Command generator' }],
            discordActivity: 'Generating a command...',
        },
    },
]
