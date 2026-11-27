import './meta'

import type { RouteRecordRaw } from 'vue-router'

export const instanceRoutes: RouteRecordRaw[] = [
    {
        path: '/instance/:id',
        name: 'Instance',
        component: () => import('@/pages/instance/Index.vue'),
        props: true,
        meta: {
            discordActivity: 'Browsing instances...',
            pageTransitionGroup: 'instance',
        },
        children: [
            {
                path: 'upgrade',
                component: () => import('@/pages/instance/upgrade/UpgradeShell.vue'),
                meta: {
                    useRootContext: true,
                    hideInstanceTabs: true,
                    breadcrumb: [{ name: 'Upgrade' }],
                },
                children: [
                    {
                        path: '',
                        name: 'InstanceUpgrade',
                        component: () => import('@/pages/instance/upgrade/Select.vue'),
                    },
                    {
                        path: 'compatibility',
                        name: 'InstanceUpgradeCompatibility',
                        component: () => import('@/pages/instance/upgrade/Compatibility.vue'),
                        meta: { upgradeRequirement: 'plan' },
                    },
                    {
                        path: 'customize',
                        name: 'InstanceUpgradeCustomize',
                        component: () => import('@/pages/instance/upgrade/Customize.vue'),
                        meta: { upgradeRequirement: 'customize' },
                    },
                    {
                        path: 'confirm',
                        name: 'InstanceUpgradeConfirm',
                        component: () => import('@/pages/instance/upgrade/Confirm.vue'),
                        meta: { upgradeRequirement: 'selection' },
                    },
                    {
                        path: 'progress',
                        name: 'InstanceUpgradeProgress',
                        component: () => import('@/pages/instance/upgrade/Progress.vue'),
                        meta: { upgradeRequirement: 'job' },
                    },
                    {
                        path: 'result',
                        name: 'InstanceUpgradeResult',
                        component: () => import('@/pages/instance/upgrade/Result.vue'),
                        meta: { upgradeRequirement: 'result' },
                    },
                ],
            },
            {
                path: 'worlds',
                name: 'InstanceWorlds',
                component: () => import('@/pages/instance/Worlds.vue'),
                meta: {
                    useRootContext: true,
                    breadcrumb: [{ name: 'Worlds' }],
                },
            },
            {
                path: 'worlds/:world/edit',
                name: 'InstanceWorldEditor',
                component: () => import('@/pages/instance/WorldEditor.vue'),
                meta: {
                    useRootContext: true,
                    breadcrumb: [
                        { name: 'Worlds', link: '/instance/{id}/worlds' },
                        { name: 'Edit world' },
                    ],
                },
            },
            {
                path: 'screenshots',
                name: 'InstanceScreenshots',
                component: () => import('@/pages/instance/Screenshots.vue'),
                meta: {
                    useRootContext: true,
                    breadcrumb: [{ name: 'Screenshots' }],
                },
            },
            {
                path: '',
                name: 'Mods',
                component: () => import('@/pages/instance/Mods.vue'),
                meta: {
                    useRootContext: true,
                    breadcrumb: [{ name: 'Content' }],
                },
            },
            {
                path: 'projects/:type',
                name: 'ModsFilter',
                component: () => import('@/pages/instance/Mods.vue'),
                meta: {
                    useRootContext: true,
                    breadcrumb: [{ name: 'Content' }],
                },
            },
            {
                path: 'files',
                name: 'Files',
                component: () => import('@/pages/instance/Files.vue'),
                meta: {
                    useRootContext: true,
                    breadcrumb: [{ name: 'Files' }],
                },
            },
            {
                path: 'files/studio',
                name: 'FileStudio',
                component: () => import('@/pages/instance/FileStudio.vue'),
                meta: {
                    renderMode: 'fixed',
                    useRootContext: true,
                    breadcrumb: [
                        { name: 'Files', link: '/instance/{id}/files' },
                        { name: 'Studio' },
                    ],
                },
            },
            {
                path: 'logs',
                name: 'Logs',
                component: () => import('@/pages/instance/Logs.vue'),
                meta: {
                    renderMode: 'fixed',
                    useRootContext: true,
                    breadcrumb: [{ name: 'Logs' }],
                },
            },
        ],
    },
]
