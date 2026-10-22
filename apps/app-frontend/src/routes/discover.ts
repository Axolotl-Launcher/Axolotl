import './meta'

import type { RouteRecordRaw } from 'vue-router'

/** Discover vertical: browse, favorites, and multi-provider project pages. */
export const discoverRoutes: RouteRecordRaw[] = [
    {
        path: '/resource/:id',
        name: 'ResourceProject',
        component: () => import('@/pages/project/ResourceRoute.vue'),
        meta: { useContext: true, pageTransitionGroup: 'project' },
    },
    {
        path: '/browse/favorites',
        name: 'Favorites',
        component: () => import('@/pages/Favorites.vue'),
        meta: {
            useContext: true,
            breadcrumb: [{ name: '?FavoritesTitle' }],
            discordActivity: 'Browsing favorites...',
            pageTransitionGroup: 'browse',
        },
    },
    {
        path: '/browse/:projectType',
        name: 'DiscoverContent',
        component: () => import('@/pages/Browse.vue'),
        meta: {
            useContext: true,
            breadcrumb: [{ name: '?BrowseTitle' }],
            discordActivity: 'Browsing mods...',
            pageTransitionGroup: 'browse',
        },
    },
    {
        path: '/:projectType(mod|plugin|datapack|resourcepack|shader|modpack)/:id/:rest(.*)*',
        redirect: (to) => {
            const rest = to.params.rest ? `/${[].concat(to.params.rest).join('/')}` : ''
            return `/project/${to.params.id}${rest}${to.hash}`
        },
    },
    {
        path: '/project/curseforge/:id',
        name: 'CurseForgeProject',
        component: () => import('@/pages/project/CurseForge.vue'),
        props: true,
        meta: {
            useContext: true,
            breadcrumb: [{ name: '?Project' }],
            discordActivity: 'Browsing mods...',
            pageTransitionGroup: 'curseforge-project',
        },
    },
    {
        path: '/project/curseforge/:id/versions',
        name: 'CurseForgeProjectVersions',
        component: () => import('@/pages/project/CurseForge.vue'),
        props: true,
        meta: {
            useContext: true,
            breadcrumb: [
                { name: '?Project', link: '/project/curseforge/{id}' },
                { name: 'Versions' },
            ],
            discordActivity: 'Browsing mods...',
            pageTransitionGroup: 'curseforge-project',
        },
    },
    {
        path: '/project/curseforge/:id/changelog',
        name: 'CurseForgeProjectChangelog',
        component: () => import('@/pages/project/CurseForge.vue'),
        props: true,
        meta: { useContext: true, pageTransitionGroup: 'curseforge-project' },
    },
    {
        path: '/project/curseforge/:id/gallery',
        name: 'CurseForgeProjectGallery',
        component: () => import('@/pages/project/CurseForge.vue'),
        props: true,
        meta: {
            useContext: true,
            breadcrumb: [
                { name: '?Project', link: '/project/curseforge/{id}' },
                { name: 'Gallery' },
            ],
            discordActivity: 'Browsing mods...',
            pageTransitionGroup: 'curseforge-project',
        },
    },
    {
        path: '/project/mcarchive/:slug',
        name: 'McArchiveProject',
        component: () => import('@/pages/project/McArchive.vue'),
        props: true,
        meta: {
            useContext: true,
            breadcrumb: [{ name: '?Project' }],
            discordActivity: 'Browsing mods...',
            pageTransitionGroup: 'mcarchive-project',
        },
    },
    {
        path: '/project/planet-minecraft/:id',
        name: 'PlanetMinecraftProject',
        component: () => import('@/pages/project/PlanetMinecraft.vue'),
        props: true,
        meta: {
            useContext: true,
            breadcrumb: [{ name: '?Project' }],
            discordActivity: 'Browsing mods...',
            pageTransitionGroup: 'planet-minecraft-project',
        },
    },
    {
        path: '/project/:id',
        name: 'Project',
        component: () => import('@/pages/project/Index.vue'),
        props: true,
        meta: {
            discordActivity: 'Browsing mods...',
            pageTransitionGroup: 'project',
        },
        children: [
            {
                path: '',
                name: 'Description',
                component: () => import('@/pages/project/Description.vue'),
                meta: {
                    useContext: true,
                    breadcrumb: [{ name: '?Project' }],
                },
            },
            {
                path: 'versions',
                name: 'Versions',
                component: () => import('@/pages/project/Versions.vue'),
                meta: {
                    useContext: true,
                    breadcrumb: [
                        { name: '?Project', link: '/project/{id}/' },
                        { name: 'Versions' },
                    ],
                },
            },
            {
                path: 'version/:version',
                name: 'Version',
                component: () => import('@/pages/project/Version.vue'),
                props: true,
                meta: {
                    useContext: true,
                    breadcrumb: [
                        { name: '?Project', link: '/project/{id}/' },
                        { name: 'Versions', link: '/project/{id}/versions' },
                        { name: '?Version' },
                    ],
                },
            },
            {
                path: 'changelog',
                name: 'Changelog',
                component: () => import('@/pages/project/Changelog.vue'),
                meta: { useContext: true },
            },
            {
                path: 'gallery',
                name: 'Gallery',
                component: () => import('@/pages/project/Gallery.vue'),
                meta: {
                    useContext: true,
                    breadcrumb: [{ name: '?Project', link: '/project/{id}/' }, { name: 'Gallery' }],
                },
            },
        ],
    },
]
