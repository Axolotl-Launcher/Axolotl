import './meta'

import type { RouteRecordRaw } from 'vue-router'

/** Page contributed by an installed theme pack, hosted in a sandboxed iframe. */
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
