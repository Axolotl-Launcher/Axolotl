import { defineComponent, provide, shallowReactive } from 'vue'
import {
    createMemoryHistory,
    createRouter,
    type RouteComponent,
    routeLocationKey,
    type RouteLocationNormalizedLoaded,
    type Router,
    type RouteRecordRaw,
    routerKey,
    routerViewLocationKey,
    START_LOCATION,
    viewDepthKey,
} from 'vue-router'

/** Child route body for pages that switch their content on the child route's name. */
const RouteOutlet = defineComponent({ name: 'RouteOutlet', render: () => null })

export interface BreadcrumbCrumb {
    name: string
    link?: string
}

declare module 'vue-router' {
    interface RouteMeta {
        breadcrumb?: BreadcrumbCrumb[]
        pageTransitionGroup?: string
        useContext?: boolean
        useRootContext?: boolean
        renderMode?: 'fixed'
        upgradeRequirement?: 'plan' | 'unblocked-plan' | 'selection' | 'job' | 'result'
    }
}

type ScreenLoader = () => Promise<RouteComponent>

/**
 * Screens registered by the preview, keyed by the desktop app's route names.
 * Each loader is a dynamic import, so a screen's code, data and translations
 * download only when that screen is first opened.
 */
export type PreviewScreens = Partial<Record<string, ScreenLoader>>

/**
 * The desktop app's route table (apps/app-frontend/src/routes), reduced to the
 * routes the preview reproduces. Paths, names and breadcrumb metadata match the
 * app so breadcrumbs, navigation highlighting and history behave the same.
 */
export function createPreviewRoutes(screens: PreviewScreens): RouteRecordRaw[] {
    const records: RouteRecordRaw[] = []

    function add(record: RouteRecordRaw & { name: string }, screen = record.name) {
        const component = screens[screen]
        if (!component) return
        records.push({ ...record, component } as RouteRecordRaw)
    }

    add({
        path: '/',
        name: 'Home',
        meta: { breadcrumb: [{ name: 'Home' }] },
    })
    add({
        path: '/screenshots',
        name: 'Screenshots',
        meta: { breadcrumb: [{ name: 'Screenshots' }] },
    })
    add({
        path: '/create',
        name: 'Create',
        meta: { useRootContext: false },
    })
    add({
        path: '/help/drop',
        name: 'DropHelp',
        meta: { breadcrumb: [{ name: 'Drop help' }] },
    })
    add({
        path: '/downloads',
        name: 'Downloads',
        meta: { breadcrumb: [{ name: 'Downloads' }] },
    })
    add({
        path: '/settings',
        name: 'Settings',
        meta: { breadcrumb: [{ name: 'Settings' }] },
    })
    add({
        path: '/skins',
        name: 'SkinSelector',
        meta: { breadcrumb: [{ name: 'Skin selector' }] },
    })
    add({
        path: '/browse/favorites',
        name: 'Favorites',
        meta: {
            useContext: true,
            breadcrumb: [{ name: '?FavoritesTitle' }],
            pageTransitionGroup: 'browse',
        },
    })
    add({
        path: '/browse/:projectType',
        name: 'DiscoverContent',
        meta: {
            useContext: true,
            breadcrumb: [{ name: '?BrowseTitle' }],
            pageTransitionGroup: 'browse',
        },
    })
    const project = screens.Project
    if (project) {
        // The app's project page renders its tab bodies through a child
        // RouterView; the preview's page switches on the child route's name,
        // so the children only carry the app's names and breadcrumb metadata.
        records.push({
            path: '/project/:id',
            component: project,
            meta: { pageTransitionGroup: 'project' },
            children: [
                {
                    path: '',
                    name: 'Project',
                    component: RouteOutlet,
                    meta: { useContext: true, breadcrumb: [{ name: '?Project' }] },
                },
                {
                    path: 'versions',
                    name: 'Versions',
                    component: RouteOutlet,
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
                    component: RouteOutlet,
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
                    component: RouteOutlet,
                    meta: { useContext: true },
                },
                {
                    path: 'gallery',
                    name: 'Gallery',
                    component: RouteOutlet,
                    meta: {
                        useContext: true,
                        breadcrumb: [
                            { name: '?Project', link: '/project/{id}/' },
                            { name: 'Gallery' },
                        ],
                    },
                },
            ],
        } as RouteRecordRaw)
    }
    add({
        path: '/multiplayer',
        name: 'Multiplayer',
        redirect: { name: 'MultiplayerServers' },
        meta: { breadcrumb: [{ name: 'Multiplayer' }], pageTransitionGroup: 'multiplayer' },
    })
    add(
        {
            path: '/multiplayer/servers',
            name: 'MultiplayerServers',
            meta: { breadcrumb: [{ name: 'Multiplayer' }], pageTransitionGroup: 'multiplayer' },
        },
        'Multiplayer',
    )
    add(
        {
            path: '/multiplayer/servers/:id',
            name: 'MultiplayerServerDetail',
            meta: { breadcrumb: [{ name: 'Multiplayer' }], pageTransitionGroup: 'multiplayer' },
        },
        'Multiplayer',
    )
    add(
        {
            path: '/multiplayer/servers/:id/studio',
            name: 'MultiplayerServerFileStudio',
            meta: {
                renderMode: 'fixed',
                breadcrumb: [
                    { name: 'Multiplayer', link: '/multiplayer/servers' },
                    { name: 'Studio' },
                ],
                pageTransitionGroup: 'multiplayer',
            },
        },
        'Multiplayer',
    )
    add(
        {
            path: '/multiplayer/rooms',
            name: 'MultiplayerRooms',
            meta: { breadcrumb: [{ name: 'Multiplayer' }], pageTransitionGroup: 'multiplayer' },
        },
        'Multiplayer',
    )
    add({
        path: '/lab',
        name: 'Lab',
        meta: { breadcrumb: [{ name: 'Lab' }] },
    })
    add({
        path: '/lab/skin-editor',
        name: 'SkinEditor',
        meta: { breadcrumb: [{ name: 'Lab', link: '/lab' }, { name: 'Skin editor' }] },
    })
    add({
        path: '/lab/gradient-text',
        name: 'GradientTextGenerator',
        meta: {
            breadcrumb: [{ name: 'Lab', link: '/lab' }, { name: 'Gradient text generator' }],
        },
    })
    add({
        path: '/lab/recipe-generator',
        name: 'RecipeGenerator',
        meta: { breadcrumb: [{ name: 'Lab', link: '/lab' }, { name: 'Recipe generator' }] },
    })
    add({
        path: '/lab/command-generator',
        name: 'CommandGenerator',
        meta: { breadcrumb: [{ name: 'Lab', link: '/lab' }, { name: 'Command generator' }] },
    })
    add({
        path: '/lab/seed-map',
        name: 'SeedMap',
        meta: { breadcrumb: [{ name: 'Lab', link: '/lab' }, { name: 'Seed map' }] },
    })
    add({
        path: '/lab/schematic-preview',
        name: 'SchematicWorkshop',
        meta: { breadcrumb: [{ name: 'Lab', link: '/lab' }, { name: 'Schematic workshop' }] },
    })
    add({
        path: '/lab/mod-translation',
        name: 'ModTranslation',
        meta: { breadcrumb: [{ name: 'Lab', link: '/lab' }, { name: 'Mod translation' }] },
    })
    add({
        path: '/library/:tab(downloaded|modpacks|servers|custom)?',
        name: 'Library',
        meta: { breadcrumb: [{ name: 'Library' }], pageTransitionGroup: 'library' },
    })
    const instance = screens.Instance
    if (instance) {
        const tab = (
            path: string,
            name: string,
            screen: string,
            breadcrumb: BreadcrumbCrumb[],
        ): RouteRecordRaw | null => {
            const component = screens[screen]
            return component
                ? ({
                      path,
                      name,
                      component,
                      meta: { useRootContext: true, breadcrumb },
                  } as RouteRecordRaw)
                : null
        }
        records.push({
            path: '/instance/:id',
            component: instance,
            meta: { pageTransitionGroup: 'instance' },
            children: [
                tab('', 'Mods', 'InstanceContent', [{ name: 'Content' }]),
                tab('projects/:type', 'ModsFilter', 'InstanceContent', [{ name: 'Content' }]),
                tab('worlds', 'InstanceWorlds', 'InstanceWorlds', [{ name: 'Worlds' }]),
                tab('worlds/:world/edit', 'InstanceWorldEditor', 'InstanceWorldEditor', [
                    { name: 'Worlds', link: '/instance/{id}/worlds' },
                    { name: 'Edit world' },
                ]),
                tab('screenshots', 'InstanceScreenshots', 'InstanceScreenshots', [
                    { name: 'Screenshots' },
                ]),
                tab('files', 'Files', 'InstanceFiles', [{ name: 'Files' }]),
                tab('files/studio', 'FileStudio', 'InstanceFileStudio', [
                    { name: 'Files', link: '/instance/{id}/files' },
                    { name: 'Studio' },
                ]),
                tab('logs', 'Logs', 'InstanceLogs', [{ name: 'Logs' }]),
                ...(screens.InstanceUpgrade
                    ? [
                          {
                              path: 'upgrade',
                              component: screens.InstanceUpgrade,
                              meta: {
                                  useRootContext: true,
                                  breadcrumb: [{ name: 'Upgrade instance' }],
                              },
                              children: [
                                  ['', 'InstanceUpgradeSelect', undefined],
                                  ['compatibility', 'InstanceUpgradeCompatibility', 'plan'],
                                  ['customize', 'InstanceUpgradeCustomize', 'unblocked-plan'],
                                  ['confirm', 'InstanceUpgradeConfirm', 'selection'],
                                  ['progress', 'InstanceUpgradeProgress', 'job'],
                                  ['result', 'InstanceUpgradeResult', 'result'],
                              ].flatMap(([path, name, requirement]) =>
                                  name && screens[name]
                                      ? [
                                            {
                                                path,
                                                name,
                                                component: screens[name],
                                                meta: {
                                                    useRootContext: true,
                                                    breadcrumb: [{ name: 'Upgrade instance' }],
                                                    upgradeRequirement: requirement,
                                                },
                                            },
                                        ]
                                      : [],
                              ),
                          } as RouteRecordRaw,
                      ]
                    : []),
            ].filter((record): record is RouteRecordRaw => record !== null),
        } as RouteRecordRaw)
    }

    return records
}

export function createPreviewRouter(screens: PreviewScreens): Router {
    return createRouter({
        history: createMemoryHistory(),
        routes: createPreviewRoutes(screens),
        linkActiveClass: 'router-link-active',
        linkExactActiveClass: 'router-link-exact-active',
    })
}

/**
 * Makes `router` the router of the calling component's subtree.
 *
 * `app.use(router)` would replace the website's own router for the whole page,
 * so the injections it performs are reproduced on the preview root instead:
 * `useRouter`, `useRoute`, `RouterLink` and `RouterView` below this component
 * (including the shared UI components) then resolve to the preview's in-memory
 * router while the page URL never changes. The view depth restarts at zero
 * because the preview is itself rendered inside the site's page `RouterView`.
 */
export function providePreviewRouter(router: Router) {
    const reactiveRoute = {} as RouteLocationNormalizedLoaded
    for (const key of Object.keys(START_LOCATION) as (keyof RouteLocationNormalizedLoaded)[]) {
        Object.defineProperty(reactiveRoute, key, {
            get: () => router.currentRoute.value[key],
            enumerable: true,
        })
    }
    provide(routerKey, router)
    provide(routeLocationKey, shallowReactive(reactiveRoute))
    provide(routerViewLocationKey, router.currentRoute)
    provide(viewDepthKey, 0)
}
