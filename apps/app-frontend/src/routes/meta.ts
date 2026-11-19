import type { RouteMeta as VueRouterRouteMeta } from 'vue-router'

export type UpgradeRouteRequirement = 'plan' | 'unblocked-plan' | 'selection' | 'job' | 'result'

export interface BreadcrumbCrumb {
    name: string
    link?: string
}

declare module 'vue-router' {
    interface RouteMeta {
        breadcrumb?: BreadcrumbCrumb[]
        discordActivity?: string
        pageTransitionGroup?: string
        useContext?: boolean
        useRootContext?: boolean
        /** Render connection recovery before mounting pages that require remote resources. */
        requiresNetwork?: boolean
        /** Hide instance tabs while this route is active (upgrade shell). */
        hideInstanceTabs?: boolean
        renderMode?: 'fixed'
        upgradeRequirement?: UpgradeRouteRequirement
    }
}

export type AppRouteMeta = VueRouterRouteMeta
