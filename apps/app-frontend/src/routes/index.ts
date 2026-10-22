import './meta'

import { createPinia, getActivePinia, setActivePinia } from 'pinia'
import { createRouter, createWebHistory } from 'vue-router'

import { discard_instance_upgrade_plan } from '@/helpers/instance-upgrade'
import {
    clearUpgradeFlow,
    hasBrowseReturnSnapshot,
    isBrowseReturnNavigation,
    peekUpgradeFlow,
    prepareBrowseReturnNavigation,
} from '@/store/navigation-return'

import { discoverRoutes } from './discover'
import { headlessDemoRoutes } from './headless-demo'
import { homeRoutes } from './home'
import { instanceRoutes } from './instance'
import { labRoutes } from './lab'
import { libraryRoutes } from './library'
import { multiplayerRoutes } from './multiplayer'
import { utilityRoutes } from './utility'

/**
 * Application router. Domain tables live in sibling modules; URLs and most
 * names match the legacy `routes.js` table (names are PascalCase).
 */
export default createRouter({
    history: createWebHistory(),
    routes: [
        ...homeRoutes,
        ...utilityRoutes,
        ...discoverRoutes,
        ...multiplayerRoutes,
        ...labRoutes,
        ...libraryRoutes,
        ...instanceRoutes,
        ...headlessDemoRoutes,
    ],
    linkActiveClass: 'router-link-active',
    linkExactActiveClass: 'router-link-exact-active',
    beforeEach(to, from) {
        // Use store helpers that bootstrap pinia if missing. Throwing here would
        // abort navigation and leave the user stuck on the current page.
        try {
            if (!getActivePinia()) {
                setActivePinia(createPinia())
            }
            const parkedUpgrade = peekUpgradeFlow()
            if (
                parkedUpgrade &&
                !to.path.startsWith('/project/') &&
                !to.fullPath.startsWith(parkedUpgrade.returnFullPath)
            ) {
                if (parkedUpgrade.plan?.id)
                    void discard_instance_upgrade_plan(parkedUpgrade.plan.id).catch(console.error)
                clearUpgradeFlow()
            }
            if (to.path.startsWith('/browse/')) {
                prepareBrowseReturnNavigation(to.fullPath, from.path)
            }
        } catch (error) {
            console.warn('[router] navigation-return guard failed; allowing navigation', error)
        }
    },
    scrollBehavior(to, from) {
        try {
            if (
                to.path.startsWith('/browse/') &&
                (isBrowseReturnNavigation(to.fullPath) || hasBrowseReturnSnapshot(to.fullPath))
            ) {
                return false
            }
        } catch {
            // ignore store errors; fall through to scroll-to-top
        }
        if (to.path === from.path) return
        // Sometimes Vue's scroll behavior is not working as expected, so we need to manually scroll to top (especially on Linux)
        document.querySelector('.app-viewport')?.scrollTo(0, 0)
        return {
            el: '.app-viewport',
            top: 0,
        }
    },
})
