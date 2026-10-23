import { reactive } from 'vue'

import type { BreadcrumbCrumb } from '../router'

export interface BreadcrumbContext extends BreadcrumbCrumb {
    query?: Record<string, string>
    iconUrl?: string | null
}

/**
 * Port of the app's breadcrumbs store (apps/app-frontend/src/store/breadcrumbs.js):
 * pages publish dynamic crumb names (`?Project`, `?Instance`, …) and a context
 * crumb that is prepended to the route's own breadcrumb trail.
 */
export function createBreadcrumbsState() {
    const state = reactive({
        names: new Map<string, string>(),
        icons: new Map<string, string>(),
        context: null as BreadcrumbContext | null,
        rootContext: null as BreadcrumbContext | null,
    })

    return {
        state,
        getName(route: string) {
            return state.names.get(route) ?? ''
        },
        setName(route: string, title: string) {
            state.names.set(route, title)
        },
        setNameIcon(route: string, iconUrl: string | null | undefined) {
            if (iconUrl) state.icons.set(route, iconUrl)
            else state.icons.delete(route)
        },
        getIcon(route: string) {
            return state.icons.get(route) ?? null
        },
        /** Drops dynamic names that the current trail no longer shows. */
        resetToNames(breadcrumbs: BreadcrumbCrumb[]) {
            const names = breadcrumbs
                .filter((breadcrumb) => breadcrumb.name.charAt(0) === '?')
                .map((breadcrumb) => breadcrumb.name.slice(1))
            for (const route of [...state.names.keys()]) {
                if (!names.includes(route)) state.names.delete(route)
            }
            for (const route of [...state.icons.keys()]) {
                if (!names.includes(route)) state.icons.delete(route)
            }
        },
        setContext(context: BreadcrumbContext | null) {
            state.context = context
        },
        setContextIcon(iconUrl: string | null) {
            if (state.context) state.context = { ...state.context, iconUrl }
        },
        setRootContext(context: BreadcrumbContext | null) {
            state.rootContext = context
        },
        setRootContextIcon(iconUrl: string | null) {
            if (state.rootContext) state.rootContext = { ...state.rootContext, iconUrl }
        },
    }
}

export type BreadcrumbsState = ReturnType<typeof createBreadcrumbsState>
