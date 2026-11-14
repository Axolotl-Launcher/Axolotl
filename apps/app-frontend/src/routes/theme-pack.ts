import './meta'

import type { RouteRecordRaw } from 'vue-router'

/** Page contributed by an installed theme pack, hosted in a child webview. */
export const themePackRoutes: RouteRecordRaw[] = [
    {
        path: '/theme-pack/:packId/:pageId',
        name: 'ThemePackPage',
        component: () => import('@/pages/ThemePackPage.vue'),
        meta: {
            useRootContext: true,
        },
    },
]
