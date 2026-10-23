<script setup lang="ts">
import LeftArrowIcon from '@modrinth/assets/icons/arrow-left.svg?component'
import ChangeSkinIcon from '@modrinth/assets/icons/change-skin.svg?component'
import CompassIcon from '@modrinth/assets/icons/compass.svg?component'
import FlaskConicalIcon from '@modrinth/assets/icons/flask-conical.svg?component'
import HomeIcon from '@modrinth/assets/icons/home.svg?component'
import ImagesIcon from '@modrinth/assets/icons/images.svg?component'
import LibraryIcon from '@modrinth/assets/icons/library.svg?component'
import PlusIcon from '@modrinth/assets/icons/plus.svg?component'
import RightArrowIcon from '@modrinth/assets/icons/right-arrow.svg?component'
import SettingsIcon from '@modrinth/assets/icons/settings.svg?component'
import UsersIcon from '@modrinth/assets/icons/users.svg?component'
import WorldIcon from '@modrinth/assets/icons/world.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Checkbox from '@modrinth/ui/src/components/base/Checkbox.vue'
import ScrollToTopButton from '@modrinth/ui/src/components/base/ScrollToTopButton.vue'
import NotificationPanel from '@modrinth/ui/src/components/nav/NotificationPanel.vue'
import PopupNotificationPanel from '@modrinth/ui/src/components/nav/PopupNotificationPanel.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, provide, ref } from 'vue'
import { type RouteLocationNormalizedLoaded, RouterView, useRoute, useRouter } from 'vue-router'

import { breadcrumbMessages, shellMessages } from '../lib/messages'
import { createPreviewShell, providePreviewShell } from '../lib/shell'
import { usePreview } from '../lib/state'
import PreviewModal from '../overlays/PreviewModal.vue'
import AccountsCard from './AccountsCard.vue'
import ActionBar from './ActionBar.vue'
import Breadcrumbs from './Breadcrumbs.vue'
import NavButton from './NavButton.vue'
import NavRail from './NavRail.vue'
import QuickInstanceSwitcher from './QuickInstanceSwitcher.vue'
import TrafficLights from './TrafficLights.vue'

/**
 * Port of the desktop app's shell (apps/app-frontend/src/App.vue) in its macOS
 * form: a 48px titlebar whose left 5rem holds the window controls, the 64px
 * navigation rail, the rounded content pane and the 300px sidebar.
 */
const emit = defineEmits<{
    quit: []
    hideToTray: []
    minimize: []
    zoom: []
}>()

const { formatMessage } = useVIntl()
/** The app provides its account card to pages (Skins signs in and updates the head through it). */
const accountsCard = ref<InstanceType<typeof AccountsCard> | null>(null)
provide('accountsCard', accountsCard)
const route = useRoute()
const router = useRouter()

/**
 * App.vue's `replayOnboarding`: the tour overlay loads on demand and runs in
 * the app-overlay host, so it keeps going while it moves between pages.
 */
provide('replayOnboarding', async (mode: string) => {
    const { default: OnboardingOverlay } =
        await import('../screens/settings/categories/about/onboarding/OnboardingOverlay.vue')
    if (mode === 'main') await router.replace('/')
    if (mode === 'logagent') await router.replace('/settings#launch-defaults')
    state.ui.showAppOverlay({ key: 'onboarding', component: OnboardingOverlay, props: { mode } })
})
const state = usePreview()
const { settings } = state

const shell = createPreviewShell(settings)
providePreviewShell(shell)
const { sidebarVisible, forceSidebar, forceSidebarHidden, sidebarToggled, toggleSidebar } = shell

const onSettingsPage = computed(() => route.path.startsWith('/settings'))
const discoverContentPath = computed(() =>
    route.path.startsWith('/browse/') && !route.query.i ? route.fullPath : '/browse/mod',
)

function isNavItemHidden(id: string) {
    return settings.hidden_nav_items.includes(id)
}

function getPageTransitionKey(pageRoute: RouteLocationNormalizedLoaded) {
    const transitionGroup = pageRoute.meta.pageTransitionGroup
    if (typeof transitionGroup !== 'string') return pageRoute.fullPath
    const routeId = pageRoute.params.id
    if (routeId !== undefined) {
        return `${transitionGroup}:${Array.isArray(routeId) ? routeId.join('/') : routeId}`
    }
    if (pageRoute.name === 'Favorites') return `${transitionGroup}:favorites`
    return `${transitionGroup}:`
}

const closeChoiceModal = ref<InstanceType<typeof PreviewModal>>()
const closeChoiceRemember = ref(false)
const closeChoiceOpen = ref(false)

function requestClose(event: MouseEvent) {
    if (closeChoiceOpen.value) return
    if (settings.close_behavior === 'close') {
        emit('quit')
        return
    }
    if (settings.close_behavior === 'lightweight') {
        emit('hideToTray')
        return
    }
    closeChoiceOpen.value = true
    closeChoiceRemember.value = false
    closeChoiceModal.value?.show(event)
}

function applyCloseChoice(choice: 'close' | 'lightweight') {
    if (closeChoiceRemember.value) settings.close_behavior = choice
    closeChoiceModal.value?.hide()
    if (choice === 'close') emit('quit')
    else emit('hideToTray')
}

function onCloseChoiceModalHide() {
    closeChoiceOpen.value = false
    closeChoiceRemember.value = false
}
</script>

<template>
    <div
        class="app-grid-layout relative"
        :class="{ 'disable-advanced-rendering': !settings.advanced_rendering }"
    >
        <div
            class="app-grid-navbar flex w-[--left-bar-width] flex-col gap-[0.5rem] overflow-hidden bg-surface-bar p-[0.5rem] pt-0"
        >
            <NavRail>
                <NavButton
                    v-if="!isNavItemHidden('home')"
                    v-tooltip.right="formatMessage(breadcrumbMessages.home)"
                    to="/"
                >
                    <HomeIcon />
                </NavButton>
                <NavButton
                    v-if="!isNavItemHidden('screenshots')"
                    v-tooltip.right="formatMessage(breadcrumbMessages.screenshots)"
                    data-onboarding-id="nav-screenshots"
                    to="/screenshots"
                    :is-primary="(r) => r.path.startsWith('/screenshots')"
                >
                    <ImagesIcon />
                </NavButton>
                <NavButton
                    v-if="settings.feature_flags.worlds_tab && !isNavItemHidden('worlds')"
                    v-tooltip.right="formatMessage(breadcrumbMessages.worlds)"
                    to="/worlds"
                >
                    <WorldIcon />
                </NavButton>
                <NavButton
                    v-if="!isNavItemHidden('discover')"
                    v-tooltip.right="formatMessage(breadcrumbMessages.discoverContent)"
                    data-onboarding-id="nav-discover"
                    :to="discoverContentPath"
                    :is-primary="() => route.path.startsWith('/browse') && !route.query.i"
                    :is-subpage="(r) => r.path.startsWith('/project') && !r.query.i"
                >
                    <CompassIcon />
                </NavButton>
                <NavButton
                    v-if="!isNavItemHidden('skins')"
                    v-tooltip.right="formatMessage(breadcrumbMessages.skinSelector)"
                    data-onboarding-id="nav-skins"
                    to="/skins"
                >
                    <ChangeSkinIcon />
                </NavButton>
                <NavButton
                    v-if="!isNavItemHidden('multiplayer')"
                    v-tooltip.right="formatMessage(breadcrumbMessages.multiplayer)"
                    to="/multiplayer"
                    :is-primary="(r) => r.path.startsWith('/multiplayer')"
                >
                    <UsersIcon />
                </NavButton>
                <NavButton
                    v-if="!isNavItemHidden('library')"
                    v-tooltip.right="formatMessage(breadcrumbMessages.library)"
                    data-onboarding-id="nav-library"
                    to="/library"
                    :is-primary="(r) => r.path.startsWith('/library')"
                    :is-subpage="
                        () =>
                            route.path.startsWith('/instance') ||
                            ((route.path.startsWith('/browse') ||
                                route.path.startsWith('/project')) &&
                                !!route.query.i)
                    "
                >
                    <LibraryIcon />
                </NavButton>
                <NavButton
                    v-if="!isNavItemHidden('lab')"
                    v-tooltip.right="formatMessage(breadcrumbMessages.lab)"
                    data-onboarding-id="nav-lab"
                    to="/lab"
                    :is-primary="(r) => r.path.startsWith('/lab')"
                >
                    <FlaskConicalIcon />
                </NavButton>
            </NavRail>
            <div class="mx-auto my-2 h-px w-6 bg-surface-5"></div>
            <div class="quick-instance-scroll min-h-0 flex-1 overflow-y-auto overflow-x-hidden">
                <QuickInstanceSwitcher />
            </div>
            <NavButton
                v-tooltip.right="formatMessage(shellMessages.createInstance)"
                data-onboarding-id="create-instance"
                to="/create"
            >
                <PlusIcon />
            </NavButton>
            <NavButton
                v-tooltip.right="formatMessage(commonMessages.settingsLabel)"
                data-onboarding-id="nav-settings"
                to="/settings"
            >
                <SettingsIcon />
            </NavButton>
        </div>
        <div
            class="app-grid-statusbar relative flex h-[--top-bar-height] min-w-0 items-center bg-surface-bar pl-20"
        >
            <TrafficLights
                @close="requestClose"
                @minimize="emit('minimize')"
                @zoom="emit('zoom')"
            />
            <div class="flex h-full shrink-0 items-center gap-2 p-3">
                <div
                    aria-label="Axolotl"
                    class="pointer-events-none flex h-full w-auto shrink-0 items-center gap-2 font-extrabold text-[var(--color-text-primary)]"
                >
                    <img
                        aria-hidden="true"
                        class="aspect-square h-full object-contain"
                        src="/axolotl.png"
                        alt=""
                    />
                    <span class="inline text-sm tracking-wide">Axolotl</span>
                </div>
            </div>
            <div class="flex shrink-0 items-center gap-1">
                <button
                    type="button"
                    :aria-label="formatMessage(commonMessages.backButton)"
                    :title="formatMessage(commonMessages.backButton)"
                    class="m-0 flex size-6 cursor-pointer items-center justify-center rounded-full border-none bg-surface-4 p-0 text-[var(--color-text-primary)] transition-all hover:brightness-75 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand"
                    @click="router.back()"
                >
                    <LeftArrowIcon aria-hidden="true" />
                </button>
                <button
                    type="button"
                    :aria-label="formatMessage(shellMessages.forward)"
                    :title="formatMessage(shellMessages.forward)"
                    class="m-0 flex size-6 cursor-pointer items-center justify-center rounded-full border-none bg-surface-4 p-0 text-[var(--color-text-primary)] transition-all hover:brightness-75 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand"
                    @click="router.forward()"
                >
                    <RightArrowIcon aria-hidden="true" />
                </button>
            </div>
            <Breadcrumbs class="flex-1 pr-3 pt-[2px]" />
            <section class="ml-auto flex min-w-0 items-center">
                <div class="mr-3 flex min-w-0">
                    <ActionBar />
                </div>
            </section>
        </div>
    </div>
    <div
        class="app-contents"
        :class="{
            'sidebar-enabled': sidebarVisible,
            'disable-advanced-rendering': !settings.advanced_rendering,
        }"
    >
        <div class="app-viewport router-view flex-grow" tabindex="-1">
            <div
                id="background-teleport-target"
                class="absolute -z-10 h-full overflow-hidden rounded-tl-[--radius-xl]"
                :style="{ width: 'calc(100% - var(--right-bar-width))' }"
            ></div>
            <div class="page-transition-grid grid min-h-full">
                <RouterView v-slot="{ Component, route: pageRoute }">
                    <Transition
                        name="page-slide"
                        :css="!!settings.feature_flags.page_transitions && !onSettingsPage"
                        appear
                    >
                        <div
                            :key="
                                onSettingsPage ? 'settings-shell' : getPageTransitionKey(pageRoute)
                            "
                            class="page-transition-layer"
                        >
                            <Suspense v-if="Component">
                                <component :is="Component" />
                            </Suspense>
                        </div>
                    </Transition>
                </RouterView>
            </div>
            <ScrollToTopButton />
        </div>
        <div
            class="app-sidebar mt-px flex shrink-0 flex-col border-0 border-l-[1px] border-solid border-[--brand-gradient-border]"
        >
            <button
                v-if="!forceSidebar && !forceSidebarHidden"
                v-tooltip.left="
                    sidebarToggled
                        ? formatMessage(shellMessages.collapseSidebar)
                        : formatMessage(shellMessages.expandSidebar)
                "
                class="sidebar-toggle-handle"
                :aria-label="
                    sidebarToggled
                        ? formatMessage(shellMessages.collapseSidebar)
                        : formatMessage(shellMessages.expandSidebar)
                "
                type="button"
                @click="toggleSidebar"
            >
                <RightArrowIcon
                    class="size-2.5 -translate-x-[1px] transition-transform duration-300"
                    :class="{ 'rotate-180': !sidebarToggled }"
                />
            </button>
            <div class="app-sidebar-scrollable relative min-h-0 flex-1 overflow-y-auto">
                <div id="sidebar-teleport-target" class="sidebar-teleport-content contents"></div>
                <div
                    class="sidebar-default-content hidden"
                    :class="{ 'sidebar-enabled': sidebarVisible }"
                >
                    <div
                        class="border-0 border-b-[1px] border-solid border-[--brand-gradient-border] p-4"
                    >
                        <h3 class="m-0 text-base font-medium text-[var(--color-text-default)]">
                            {{ formatMessage(shellMessages.playingAs) }}
                        </h3>
                        <AccountsCard ref="accountsCard" />
                    </div>
                    <div id="sidebar-default-teleport-target"></div>
                </div>
            </div>
        </div>
    </div>
    <NotificationPanel
        :has-sidebar="sidebarVisible"
        :error-action-label="formatMessage(shellMessages.exportErrorLogs)"
    />
    <PopupNotificationPanel
        :has-sidebar="sidebarVisible"
        :error-action-label="formatMessage(shellMessages.exportErrorLogs)"
    />
    <component
        :is="overlay.component"
        v-for="overlay in state.ui.appOverlays.value"
        :key="overlay.key"
        v-bind="overlay.props"
        @close="state.ui.hideAppOverlay(overlay.key)"
    />
    <PreviewModal
        ref="closeChoiceModal"
        :header="formatMessage(shellMessages.closeLauncherTitle)"
        :closable="true"
        :close-on-click-outside="true"
        :on-hide="onCloseChoiceModalHide"
        max-width="30rem"
    >
        <div class="grid grid-cols-2 gap-3">
            <Button type="colored" color="brand" @click="applyCloseChoice('close')"
                >{{ formatMessage(shellMessages.closeLauncherDirect) }}
            </Button>
            <Button @click="applyCloseChoice('lightweight')"
                >{{ formatMessage(shellMessages.closeLauncherTray) }}
            </Button>
        </div>
        <div class="mt-4">
            <Checkbox
                v-model="closeChoiceRemember"
                :label="formatMessage(shellMessages.closeLauncherRemember)"
            />
        </div>
    </PreviewModal>
</template>

<style lang="scss" scoped>
.app-grid-layout,
.app-contents {
    --top-bar-height: 3rem;
    --left-bar-width: 4rem;
    --right-bar-width: 300px;
}

.app-grid-layout {
    display: grid;
    grid-template: 'status status' 'nav dummy';
    grid-template-columns: auto 1fr;
    grid-template-rows: auto 1fr;
    position: relative;
    background-color: var(--surface-3);
    height: 100%;
}

.quick-instance-scroll {
    -ms-overflow-style: none;
    scrollbar-width: none;

    &::-webkit-scrollbar {
        display: none;
    }
}

.app-grid-navbar {
    grid-area: nav;
    position: relative;
    z-index: 2;
    user-select: none;
}

.app-grid-statusbar {
    grid-area: status;
    position: relative;
    z-index: 200;
}

.app-contents {
    position: absolute;
    z-index: 1;
    left: var(--left-bar-width);
    top: var(--top-bar-height);
    right: 0;
    bottom: 0;
    background-color: var(--color-bg);
    border-top-left-radius: var(--radius-xl);
    overflow: hidden;
    isolation: isolate;
    --right-bar-width: 0px;

    display: grid;
    grid-template-columns: 1fr var(--right-bar-width);
    grid-template-rows: 1fr;

    &.sidebar-enabled {
        --right-bar-width: 300px;
    }
}

@media (prefers-reduced-motion: no-preference) {
    .app-contents {
        transition: --right-bar-width 320ms cubic-bezier(0.22, 1, 0.36, 1);
    }
}

.app-sidebar {
    overflow: visible;
    width: 300px;
    position: relative;
    z-index: 11;
    height: calc(800px - var(--top-bar-height));
    background: var(--brand-gradient-bg);

    --sidebar-control-surface: var(--brand-gradient-button);
    --sidebar-control-surface-hover: var(--brand-gradient-border);
    --sidebar-stroke: var(--brand-gradient-border);
}

.app-sidebar-scrollable {
    :deep(.bg-surface-4) {
        background-color: var(--sidebar-control-surface);
    }

    :deep(.hover\:bg-surface-4:hover) {
        background-color: var(--sidebar-control-surface-hover);
    }

    :deep(.bg-divider) {
        background-color: var(--sidebar-stroke);
    }

    :deep(.border-divider),
    :deep(.border-button-border) {
        border-color: var(--sidebar-stroke);
    }
}

.disable-advanced-rendering {
    .app-sidebar::before {
        box-shadow: none;
    }

    &.app-contents::before {
        box-shadow: none;
    }

    *,
    :deep(*) {
        box-shadow: none !important;
        --tw-drop-shadow: initial;
    }
}

.app-sidebar::before {
    content: '';
    box-shadow: -15px 0 15px -15px rgba(0, 0, 0, 0.1) inset;
    top: 0;
    bottom: 0;
    left: -2rem;
    width: 2rem;
    position: absolute;
    pointer-events: none;
}

.sidebar-toggle-handle {
    --handle-bg: color-mix(
        in srgb,
        var(--color-brand) var(--opacity-ratio-keep-12),
        var(--color-bg)
    );
    --handle-bg-hover: color-mix(
        in srgb,
        var(--color-brand) var(--opacity-ratio-keep-20),
        var(--color-bg)
    );
    --handle-border: var(--brand-gradient-border);
    --handle-border-hover: color-mix(
        in srgb,
        var(--color-brand) var(--opacity-ratio-keep-45),
        transparent
    );

    position: absolute;
    top: 50%;
    left: -15px;
    transform: translateY(-50%);
    z-index: 12;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 15px;
    height: 40px;
    padding: 0;
    border: 1px solid var(--handle-border);
    border-right: none;
    border-radius: 12px 0 0 12px;
    background-color: var(--handle-bg);
    color: var(--color-text-primary);
    cursor: pointer;
    box-shadow: -4px 0 10px rgba(0, 0, 0, 0.08);
    transition:
        background-color 180ms ease,
        border-color 180ms ease,
        color 180ms ease;

    &:hover {
        color: var(--color-button-text-selected);
        background-color: var(--handle-bg-hover);
        border-color: var(--handle-border-hover);
    }

    &:hover svg {
        transform: scale(1.12);
    }

    &:active svg {
        transform: scale(0.9);
    }

    &:focus-visible {
        outline: 2px solid var(--color-button-text-selected);
        outline-offset: 1px;
    }

    svg {
        transform-origin: center;
        transition: transform 180ms ease;
    }
}

.app-viewport {
    flex-grow: 1;
    height: 100%;
    overflow: auto;
    overflow-x: hidden;
    scrollbar-gutter: stable;

    &:focus,
    &:focus-visible {
        outline: none;
    }
}

.app-contents::before {
    z-index: 30;
    content: '';
    position: absolute;
    inset: 0;
    border-top-left-radius: var(--radius-xl);
    box-shadow: 1px 1px 15px rgba(0, 0, 0, 0.1) inset;
    border-color: var(--surface-5);
    border-width: 1px;
    border-style: solid;
    pointer-events: none;
}

.sidebar-teleport-content:empty + .sidebar-default-content.sidebar-enabled {
    display: contents;
}
</style>

<style>
@property --right-bar-width {
    syntax: '<length>';
    inherits: true;
    initial-value: 0px;
}
</style>
