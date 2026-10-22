<script setup lang="ts">
import '@/assets/stylesheets/window-frame.css'

import { AuthFeature, TauriModrinthClient, VerboseLoggingFeature } from '@modrinth/api-client'
import {
    ChangeSkinIcon,
    CompassIcon,
    ExternalIcon,
    FlaskConicalIcon,
    FolderOpenIcon,
    HomeIcon,
    ImagesIcon,
    LibraryIcon,
    LogInIcon,
    LogOutIcon,
    PlusIcon,
    RefreshCwIcon,
    RightArrowIcon,
    RotateCounterClockwiseIcon,
    SettingsIcon,
    SpinnerIcon,
    UserIcon,
    UsersIcon,
    WorldIcon,
    XIcon,
} from '@modrinth/assets'
import {
    Admonition,
    Avatar,
    BatchScanOverlay,
    BigOptionButton,
    bindingMatchesKeyboardEvent,
    bindingMatchesMouseEvent,
    bindingMatchesWheelEvent,
    Button,
    Checkbox,
    clientInstallableLoaders,
    commonMessages,
    ConfirmDropTypeModal,
    ContentInstallModal,
    ContentUpdaterModal,
    CreationFlowModal,
    defineMessages,
    GenericContentInstallModal,
    I18nDebugPanel,
    type KeyBinding,
    LauncherImportModal,
    LoadingBar,
    NewModal,
    NotificationPanel,
    OverflowMenu,
    PopupNotificationPanel,
    provideModalBehavior,
    provideModrinthClient,
    provideNotificationManager,
    providePageContext,
    providePopupNotificationManager,
    ScrollToTopButton,
    SymlinkMethodCards,
    useFormatBytes,
    useModalStack,
    useVIntl,
} from '@modrinth/ui'
import { useQueryClient } from '@tanstack/vue-query'
import { getVersion } from '@tauri-apps/api/app'
import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { Effect, getCurrentWindow } from '@tauri-apps/api/window'
import { fetch as tauriFetch } from '@tauri-apps/plugin-http'
import { openUrl } from '@tauri-apps/plugin-opener'
import { type as getOsType } from '@tauri-apps/plugin-os'
import { saveWindowState, StateFlags } from '@tauri-apps/plugin-window-state'
import { computed, nextTick, onMounted, onUnmounted, provide, ref, watch } from 'vue'
import { type RouteLocationNormalizedLoaded, RouterView, useRoute, useRouter } from 'vue-router'

import { getAnnouncementByVersion } from '@/announcements/catalog'
import InstanceExportModal from '@/components/lab/recipe-generator/InstanceExportModal.vue'
import ImportServerModal from '@/components/multiplayer/servers/ImportServerModal.vue'
import AccountsCard from '@/components/ui/AccountsCard.vue'
import UpdateAnnouncementModal from '@/components/ui/announcement/UpdateAnnouncementModal.vue'
import AppActionBar from '@/components/ui/AppActionBar.vue'
import AppTitlebar from '@/components/ui/AppTitlebar.vue'
import ContentInstallPreviewModal from '@/components/ui/ContentInstallPreviewModal.vue'
import ErrorModal from '@/components/ui/ErrorModal.vue'
import AddServerToInstanceModal from '@/components/ui/install_flow/AddServerToInstanceModal.vue'
import UnknownPackWarningModal from '@/components/ui/install_flow/UnknownPackWarningModal.vue'
import MinecraftAuthErrorModal from '@/components/ui/minecraft-auth-error-modal/MinecraftAuthErrorModal.vue'
import MinecraftCrashModal from '@/components/ui/MinecraftCrashModal.vue'
import AuthGrantFlowWaitModal from '@/components/ui/modal/AuthGrantFlowWaitModal.vue'
import CurseForgeManualDownloadsModal from '@/components/ui/modal/CurseForgeManualDownloadsModal.vue'
import InstanceIconPickerModal from '@/components/ui/modal/InstanceIconPickerModal.vue'
import JavaDownloadConfirmationModal from '@/components/ui/modal/JavaDownloadConfirmationModal.vue'
import ModpackAlreadyInstalledModal from '@/components/ui/modal/ModpackAlreadyInstalledModal.vue'
import ModpackInstallModal from '@/components/ui/modal/ModpackInstallModal.vue'
import PrivacyConsentModal from '@/components/ui/modal/PrivacyConsentModal.vue'
import NavButton from '@/components/ui/NavButton.vue'
import NavRail from '@/components/ui/NavRail.vue'
import OfflineAccountRestrictionModal from '@/components/ui/OfflineAccountRestrictionModal.vue'
import OnboardingOverlay from '@/components/ui/onboarding/OnboardingOverlay.vue'
import QuickInstanceSwitcher from '@/components/ui/QuickInstanceSwitcher.vue'
import RemoteAnnouncements from '@/components/ui/RemoteAnnouncements.vue'
import ResourceOperationPreviewModal from '@/components/ui/ResourceOperationPreviewModal.vue'
import SplashScreen from '@/components/ui/SplashScreen.vue'
import WindowControls from '@/components/ui/WindowControls.vue'
import { useCheckDisableMouseover } from '@/composables/macCssFix.js'
import { isOfflineAccountRestrictedError } from '@/composables/useAntiPiracyStatus'
import { useDropImport } from '@/composables/useDropImport'
import { minecraftLaunchErrorKey } from '@/composables/useMinecraftLaunchError'
import { setupMojangAuthMonitor, useMojangAuthStatus } from '@/composables/useMojangAuth'
import { useNetworkStatus } from '@/composables/useNetworkStatus'
import { AxolotlBrandConfig, config, getOfficialLabrinthBaseUrl } from '@/config'
import { trackEvent } from '@/helpers/analytics'
import { get_user, get_version } from '@/helpers/cache.js'
import { configureCurseForgeManualDownloadWatcher } from '@/helpers/curseforge'
import { applySettingChanges } from '@/helpers/deep-link-settings.ts'
import { resolveOpenRoute, resolveSettingsRoute } from '@/helpers/deep-links.ts'
import {
    DIRECT_LINKS_SYNCED_EVENT,
    startConfiguredDirectLinkSync,
} from '@/helpers/direct-link-sync'
import { getMissingContentScannerSettings } from '@/helpers/downloads-scanner'
import { classifyDroppedItem } from '@/helpers/drop'
import {
    command_listener,
    drop_classify_progress_listener,
    java_download_confirmation_listener,
    warning_listener,
} from '@/helpers/events.js'
import { fileOperationError } from '@/helpers/file-operation-errors'
import { install_create_modpack_instance, install_get_modpack_preview } from '@/helpers/install'
import { type DirectLinkSyncReport, get as getInstance, run } from '@/helpers/instance'
import { type BackupOperation, cancelBackup, listBackupOperations } from '@/helpers/instance-backup'
import { cancelLogin, get as getCreds, login, logout } from '@/helpers/mr_auth.ts'
import { installNativeEditMenuBoundary } from '@/helpers/native-edit-menu'
import { getNavShortcutEnabled } from '@/helpers/nav-shortcut-state'
import { runWhenIdle } from '@/helpers/page-transition'
import { get_by_instance_id, kill as killProcess } from '@/helpers/process.js'
import { getExternalLinkUrl, mergeUrlQuery, parseModrinthLink } from '@/helpers/project-links.ts'
import {
    isResourceOperationError,
    resourceOperationError,
} from '@/helpers/resource-operation-logic'
import { getQuickScrollEnabled, getShowScrollTop } from '@/helpers/scroll-top-state'
import {
    appSettingsKeys,
    get as getSettings,
    getLastBrowseContentProjectType,
    getPrivacySettings,
    getUpdateChannel,
    getUpdatePreferences,
    type PrivacySettings,
    savePrivacySettings,
    update as updateSettings,
} from '@/helpers/settings.ts'
import { diffSettings } from '@/helpers/settings-patch'
import {
    discoverContentTarget,
    SHORTCUT_ACTIONS,
    type ShortcutAction,
} from '@/helpers/shortcut-actions'
import { resolveAllBindings } from '@/helpers/shortcut-bindings'
import { getSidebarExpanded, setSidebarExpanded } from '@/helpers/sidebar-state.ts'
import { get_opening_command, initialize_state, set_discord_activity } from '@/helpers/state'
import {
    areUpdatesEnabled,
    backupAppDbForUpdate,
    checkAppUpdate,
    enqueueUpdateForInstallation,
    exportErrorLogs,
    getOS,
    getUpdateSize,
    isDev,
    isElevated,
    isNetworkMetered,
    setRestartAfterPendingUpdate,
} from '@/helpers/utils.js'
import {
    createWindowAppearanceController,
    type WindowFrameResult,
} from '@/helpers/window-appearance'
import { start_join_server, start_join_singleplayer_world } from '@/helpers/worlds.ts'
import { applyLocalePreference, setFollowSystemLocale } from '@/i18n.config'
import {
    appUpdateState,
    downloadAvailableAppUpdate,
    getNextAppUpdatePopupTime,
    installAvailableAppUpdate,
    markAppUpdateActionable,
    markAppUpdatePopupShown,
    openAppUpdateChangelog,
    setAppUpdateActions,
} from '@/providers/app-update.ts'
import { createContentInstall, provideContentInstall } from '@/providers/content-install'
import { createContentSelection, provideContentSelection } from '@/providers/content-selection'
import { createDownloadManager, provideDownloadManager } from '@/providers/download-manager'
import {
    provideAppUpdateDownloadProgress,
    subscribeToDownloadProgress,
} from '@/providers/download-progress.ts'
import { createServerInstall, provideServerInstall } from '@/providers/server-install'
import { setupProviders } from '@/providers/setup'
import { setupAuthProvider } from '@/providers/setup/auth'
import { setupLoadingStateProvider } from '@/providers/setup/loading-state'
import { useError } from '@/store/error.js'
import { useTheming } from '@/store/state'

import { AppNotificationManager } from './providers/app-notifications'
import { AppPopupNotificationManager } from './providers/app-popup-notifications'

const themeStore = useTheming()
/** While a dialog is open no shortcut fires, including the one being recorded. */
const { hasModal } = useModalStack()
const router = useRouter()
const route = useRoute()
const onSkinsPage = computed(() => route.path === '/skins')
const onSchematicWorkshopPage = computed(() => route.path === '/lab/schematic-preview')
const onSettingsPage = computed(() => route.path.startsWith('/settings'))
const isSchematicFile = (path: string) => /\.(litematic|schematic|schem)$/i.test(path)
const APP_LEFT_NAV_WIDTH = '4rem'

const discoverContentPath = computed(() => discoverContentTarget(route))

const pagePathCopied = ref(false)
let pagePathCopiedTimer: ReturnType<typeof window.setTimeout> | undefined

async function copyPagePath() {
    try {
        await navigator.clipboard.writeText(route.fullPath)
    } catch {
        return
    }

    pagePathCopied.value = true
    if (pagePathCopiedTimer) window.clearTimeout(pagePathCopiedTimer)
    pagePathCopiedTimer = window.setTimeout(() => {
        pagePathCopied.value = false
    }, 2000)
}

function getPageTransitionKey(route: RouteLocationNormalizedLoaded) {
    const transitionGroup = route.meta.pageTransitionGroup
    if (typeof transitionGroup !== 'string') return route.fullPath

    const routeId = route.params.id
    if (routeId !== undefined) {
        return `${transitionGroup}:${Array.isArray(routeId) ? routeId.join('/') : routeId}`
    }

    // Browse-style routes use :projectType instead of :id. Keep tab switches on
    // the same SPA instance (Browse already watches the param) so only the
    // results area refreshes — no full page transition. Favorites is a different
    // component under the same group; give it its own key so it still remounts.
    if (route.name === 'Favorites') {
        return `${transitionGroup}:favorites`
    }

    return `${transitionGroup}:`
}
const APP_SIDEBAR_WIDTH = 300
const credentials = ref()
const sidebarToggled = ref(getSidebarExpanded())

function toggleSidebar() {
    sidebarToggled.value = !sidebarToggled.value
    setSidebarExpanded(sidebarToggled.value)
}

const forceSidebar = computed(
    () => route.path.startsWith('/browse') || route.path.startsWith('/project'),
)
const forceSidebarHidden = computed(() => route.path === '/settings')
/** Console fullscreen collapses the account sidebar so its controls stay reachable. */
const consoleFullscreenActive = ref(false)
const sidebarVisible = computed(
    () =>
        !forceSidebarHidden.value &&
        !consoleFullscreenActive.value &&
        (sidebarToggled.value || forceSidebar.value),
)
const customBackgroundStyle = computed(() => {
    // A custom image would sit between the desktop and the UI, defeating the
    // transparent window entirely, so the two are mutually exclusive.
    if (themeStore.transparentBackground || !themeStore.customBackgroundPath) return undefined

    return {
        backgroundImage: `url("${convertFileSrc(themeStore.customBackgroundPath)}")`,
        filter: `blur(${themeStore.customBackgroundBlur}px)`,
        // "Background visibility" is applied as the alpha of the page layer above
        // this image (see `global.scss`), not as an opacity here. Dimming the
        // image as well would apply the setting twice and wash the picture out
        // against its own backing.
    }
})

const notificationManager = new AppNotificationManager()
provideNotificationManager(notificationManager)
const { handleError, addNotification } = notificationManager
const downloadManager = createDownloadManager(handleError)
provideDownloadManager(downloadManager)
const contentSelection = createContentSelection({
    addNotification,
    handleError,
    downloadManager,
})
provideContentSelection(contentSelection)

const popupNotificationManager = new AppPopupNotificationManager()
providePopupNotificationManager(popupNotificationManager)
const { addPopupNotification } = popupNotificationManager

const appVersion = getVersion()
const isBetaBuild = ref(false)
void appVersion
    .then((version) => {
        isBetaBuild.value = version.includes('-beta')
    })
    .catch((error) => {
        console.warn('Failed to read the app version for the Beta label', error)
    })
const tauriApiClient = new TauriModrinthClient({
    userAgent: async () => AxolotlBrandConfig.userAgent(await appVersion, await getOsType()),
    labrinthBaseUrl: config.labrinthBaseUrl,
    features: [
        ...(AxolotlBrandConfig.capabilities.privateModrinthServices
            ? [
                  new AuthFeature({
                      token: async () => (await getCreds())?.session,
                  }),
              ]
            : []),
        new VerboseLoggingFeature(),
    ],
})
provideModrinthClient(tauriApiClient)
providePageContext({
    hierarchicalSidebarAvailable: ref(true),
    showAds: ref(false),
    floatingActionBarOffsets: {
        left: ref(APP_LEFT_NAV_WIDTH),
        right: computed(() => (sidebarVisible.value ? `${APP_SIDEBAR_WIDTH}px` : '0px')),
    },
    featureFlags: {
        serverRamAsBytesAlwaysOn: computed(() =>
            themeStore.getFeatureFlag('server_ram_as_bytes_always_on'),
        ),
    },
    openExternalUrl: (url) => openUrl(url),
})
provideModalBehavior({
    noblur: computed(() => !themeStore.advancedRendering),
})

const stateInitialization = initialize_state()
const {
    instanceIconPickerModal,
    installationModal,
    unknownPackWarningModal,
    fetchExistingInstanceNames,
    handleCreate,
    handleBrowseModpacks,
    searchModpacks,
    getProjectVersions,
    hasCompatibleOptiFabric,
    getLoaderManifest,
    installModpackFromPath,
    setModpackAlreadyInstalledModal,
    handleModpackDuplicateCreateAnyway,
    handleModpackDuplicateGoToInstance,
    fileDrop,
} = setupProviders(notificationManager, popupNotificationManager, stateInitialization)

const { offline } = useNetworkStatus()

const showOnboarding = ref(false)
const onboardingMode = ref('main')
const onboardingSettings = ref(null)
const onboardingReplay = ref(false)
const nativeDecorations = ref(false)

const os = ref('')
const isDevEnvironment = ref(false)

const isMaximized = ref(false)
const appearanceReady = ref(false)
const cssWindowBorder = ref(false)
const windowAppearance = createWindowAppearanceController(
    async (state) => {
        if (state.os === 'Linux') return
        const window = getCurrentWindow()
        if (state.transparent && state.blur) {
            await window.setEffects({
                effects: [state.os === 'MacOS' ? Effect.UnderWindowBackground : Effect.Acrylic],
            })
        } else await window.clearEffects()
    },
    (state) =>
        invoke<WindowFrameResult>('set_transparent_window_frame', {
            enabled: state.transparent,
        }),
    (enabled) => {
        cssWindowBorder.value = enabled
    },
    (error) => console.debug('Window appearance compatibility result', error),
)

function applyWindowAppearance() {
    return windowAppearance.update({
        os: os.value,
        transparent: themeStore.transparentBackground,
        blur: themeStore.transparentBackgroundBlur,
        decorated: nativeDecorations.value,
        maximized: isMaximized.value,
    })
}

watch(
    () => [
        themeStore.transparentBackground,
        themeStore.transparentBackgroundBlur,
        nativeDecorations.value,
    ],
    () => {
        if (appearanceReady.value) void applyWindowAppearance()
    },
)

const stateInitialized = ref(false)
const privacyConsentModal = ref<InstanceType<typeof PrivacyConsentModal>>()
const privacyConsentPending = ref(false)
const updateAnnouncementModal = ref()
const closeChoiceModal = ref<InstanceType<typeof NewModal>>()
const backupExitModal = ref<InstanceType<typeof NewModal>>()
const activeBackupOperations = ref<BackupOperation[]>([])
const closeChoiceOpen = ref(false)
const closeChoiceRemember = ref(false)
const closeRequestInProgress = ref(false)
let allowWindowClose = false
let unlistenCloseRequested: (() => void) | undefined
let unlistenLightweightModeError: (() => void) | undefined
let unlistenSystemAccentColor: (() => void) | undefined
let maximizedStateTimer: ReturnType<typeof setTimeout> | undefined
let unlistenWindowResize: (() => void) | undefined
let stopNativeEditMenu: (() => void) | undefined
const minecraftCrashModal = ref()
const javaDownloadConfirmationModal = ref()
const pendingUpdateAnnouncementVersion = ref(null)
const updateAnnouncementShowing = ref(false)

setupMojangAuthMonitor(stateInitialized)
const mojangAuth = useMojangAuthStatus()
const authUnreachable = computed(() => !offline.value && mojangAuth.reachable.value === false)

const appUpdateDownload = {
    progress: appUpdateState.progress,
    version: ref(),
}
let unlistenUpdateDownload

const {
    metered,
    finishedDownloading,
    downloading,
    restarting,
    availableUpdate,
    updateSize,
    updatesEnabled,
    updatesPaused,
} = appUpdateState
let delayedUpdatePopupTimeout = null

async function checkUpdates() {
    if (!(await areUpdatesEnabled())) {
        console.log('Skipping update check as updates are disabled in this build or environment')
        updatesEnabled.value = false

        return
    }

    updatesEnabled.value = true
    updatesPaused.value = (await getUpdatePreferences()).updatesPaused
    if (updatesPaused.value) {
        setTimeout(checkUpdates, 5 * 60 * 1000)
        return
    }
    if (!offline.value) {
        await performUpdateCheck().catch((error) => {
            console.warn('Failed to check for launcher updates', error)
        })
    }
    setTimeout(
        () => {
            checkUpdates()
        },
        5 /* min */ * 60 /* sec */ * 1000 /* ms */,
    )
}

/**
 * Keep browser/webview shortcuts from escaping the launcher UI. F12 remains
 * available when the in-app developer mode is enabled so development tools
 * can still be opened intentionally.
 */
function handleGlobalKeydown(event: KeyboardEvent) {
    const key = event.key.toLowerCase()
    const isFindShortcut = key === 'f' && (event.ctrlKey || event.metaKey)
    const isBlockedDevtoolsShortcut = event.key === 'F12' && !themeStore.devMode

    if (isFindShortcut || isBlockedDevtoolsShortcut) {
        event.preventDefault()
        event.stopPropagation()
    }

    handleScrollShortcutKey(event)
    handleNavShortcutKey(event)
}

/** Editing surfaces keep these keys for themselves. */
function isEditableTarget(target: EventTarget | null) {
    return (
        target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        target instanceof HTMLSelectElement ||
        (target instanceof HTMLElement && target.isContentEditable)
    )
}

function scrollsAtOwnLevel(element: Element) {
    const { overflowY } = getComputedStyle(element)
    return /(auto|scroll|overlay)/.test(overflowY) && element.scrollHeight > element.clientHeight
}

/** A container that scrolls the page itself rather than a widget on it. */
function isPageScroller(element: Element) {
    return scrollsAtOwnLevel(element) && element.clientHeight >= window.innerHeight / 2
}

/** Nearest scrolling ancestor, whatever its size. */
function nearestScrollContainer(target: EventTarget | null) {
    let element = target instanceof Element ? target : null
    while (element) {
        if (scrollsAtOwnLevel(element)) return element
        element = element.parentElement
    }
    return null
}

/**
 * The page's own scroller. Most pages scroll inside `.app-viewport`, but some
 * screens (settings, consoles, studios) turn that element's scrolling off and
 * host a full-height container inside it, so fall back to whatever fills the
 * middle of the viewport.
 */
function resolvePageScroller() {
    const viewport = document.querySelector<HTMLElement>('.app-viewport')
    if (!viewport) return null
    if (isPageScroller(viewport)) return viewport

    const bounds = viewport.getBoundingClientRect()
    let element: Element | null = document.elementFromPoint(
        bounds.left + bounds.width / 2,
        bounds.top + bounds.height / 2,
    )
    while (element) {
        if (isPageScroller(element)) return element
        element = element.parentElement
    }

    return viewport
}

/** The combination an action answers to now: recorded if there is one, default otherwise. */
function bindingFor(action: ShortcutAction): KeyBinding {
    return themeStore.shortcutBindings[action.id] ?? action.defaultBinding
}

/** Whether an action is switched on and reachable in the current state. */
function shortcutIsAvailable(action: ShortcutAction): boolean {
    if (!themeStore[action.enabledField]) return false
    return !action.unavailable?.({
        worldsTabEnabled: themeStore.featureFlags.worlds_tab,
        offline: offline.value,
    })
}

/**
 * Finds the action a combination belongs to. Shortcuts stand down entirely
 * while a dialog is open, so the one being recorded cannot fire, and editing
 * surfaces keep their keys. `includeDisabled` is for scrolling, which has to
 * recognise its keys even while it is off in order to hold them back.
 */
function findShortcut<E extends Event>(
    event: E,
    matches: (binding: KeyBinding, event: E) => boolean,
    { includeDisabled = false }: { includeDisabled?: boolean } = {},
): ShortcutAction | null {
    if (hasModal.value) return null
    if (isEditableTarget(event.target)) return null

    for (const action of SHORTCUT_ACTIONS) {
        if (!includeDisabled && !shortcutIsAvailable(action)) continue
        if (matches(bindingFor(action), event)) return action
    }

    return null
}

/** Does what an action is for. Scrolling actions move the page's own scroller. */
function runShortcut(action: ShortcutAction, event: Event) {
    if (action.target) {
        router.push(action.target(route))
        return
    }

    const focusedContainer = nearestScrollContainer(event.target)
    if (focusedContainer && !isPageScroller(focusedContainer)) return

    const scroller = resolvePageScroller()
    if (!scroller) return

    action.applyScroll?.(scroller)
}

/**
 * Quick scrolling for the page's own scroll container. The setting decides
 * whether the keys act at all: while it is off they stay inert, and while it is
 * on they move the page. A focused list or popover still scrolls itself, so
 * local keyboard scrolling keeps working.
 */
function handleScrollShortcutKey(event: KeyboardEvent) {
    if (event.isComposing) return

    const match = findShortcut(event, bindingMatchesKeyboardEvent, { includeDisabled: true })
    if (match?.group !== 'scroll') return

    const focusedContainer = nearestScrollContainer(event.target)
    if (focusedContainer && !isPageScroller(focusedContainer)) return

    const scroller = resolvePageScroller()
    if (!scroller) return

    // Take the key over even when the feature is off: the browser would scroll
    // a focused page container on its own, which would make the setting a lie.
    event.preventDefault()
    if (!shortcutIsAvailable(match)) return

    match.applyScroll?.(scroller)
}

/**
 * Jump to a menu item with the combination it is set to. Every shortcut is off
 * by default and enabled individually from the Shortcut settings page.
 */
function handleNavShortcutKey(event: KeyboardEvent) {
    if (event.isComposing) return

    const match = findShortcut(event, bindingMatchesKeyboardEvent)
    if (match?.group !== 'nav') return

    event.preventDefault()
    runShortcut(match, event)
}

/** Whether any action listens to the pointer, so the listeners can stay cheap. */
const hasPointerBindings = computed(() =>
    SHORTCUT_ACTIONS.some((action) => bindingFor(action).device === 'mouse'),
)

function handleShortcutMouse(event: MouseEvent) {
    if (!hasPointerBindings.value) return

    const match = findShortcut(event, bindingMatchesMouseEvent)
    if (!match) return

    // A shortcut owns this button, so the pointer event stops here.
    event.preventDefault()
    event.stopPropagation()
    runShortcut(match, event)
}

function handleShortcutWheel(event: WheelEvent) {
    if (!hasPointerBindings.value) return

    const match = findShortcut(event, bindingMatchesWheelEvent)
    if (!match) return

    event.preventDefault()
    event.stopPropagation()
    runShortcut(match, event)
}

/**
 * Hand the keyboard over to the page after the menu navigates. Without this the
 * focus stays on the nav button, so Tab keeps walking the rail and the page's
 * own shortcuts act on whatever happened to be focused before.
 */
watch(
    () => route.path,
    async () => {
        await nextTick()
        const active = document.activeElement
        const navRail = document.querySelector('.app-grid-navbar')
        const cameFromMenu =
            !active || active === document.body || (navRail?.contains(active) ?? false)
        if (!cameFromMenu) return
        document.querySelector<HTMLElement>('.app-viewport')?.focus({ preventScroll: true })
    },
)

onMounted(async () => {
    unlistenLightweightModeError = await listen<string>('lightweight-mode-error', ({ payload }) => {
        allowWindowClose = false
        closeRequestInProgress.value = false
        if (!closeChoiceOpen.value) {
            closeChoiceOpen.value = true
            closeChoiceRemember.value = false
            closeChoiceModal.value?.show()
        }
        handleError(payload)
    })
    await useCheckDisableMouseover()

    window.addEventListener('keydown', handleGlobalKeydown, true)
    window.addEventListener('mousedown', handleShortcutMouse, true)
    // Not passive: a bound wheel direction has to keep the page from scrolling.
    window.addEventListener('wheel', handleShortcutWheel, { capture: true, passive: false })
    unlistenCloseRequested = await getCurrentWindow().onCloseRequested(handleCloseRequested)
    document.querySelector('body').addEventListener('click', handleClick)
    document.querySelector('body').addEventListener('auxclick', handleAuxClick)
    window.addEventListener(DIRECT_LINKS_SYNCED_EVENT, handleDirectLinkSyncReport)
    window.addEventListener('modrinth-console-fullscreen', handleConsoleFullscreen as EventListener)

    // Background maintenance must not compete with first paint / route enter.
    runWhenIdle(() => {
        if (appUnmounted) return
        void checkUpdates()
        void warnIfRunningElevated()
        startDirectLinkSync()
    })
})

let appUnmounted = false
let stopDirectLinkSync: (() => void) | undefined
let directLinkSyncErrorSignature = ''

function handleConsoleFullscreen(event: CustomEvent<boolean>) {
    consoleFullscreenActive.value = Boolean(event.detail)
}

function handleDirectLinkSyncReport(event: Event) {
    if (!(event instanceof CustomEvent)) return
    const report = event.detail as DirectLinkSyncReport
    if (!Array.isArray(report?.errors)) return

    const details = report.errors.join('\n')
    if (!details) {
        directLinkSyncErrorSignature = ''
        return
    }
    if (details === directLinkSyncErrorSignature) return

    directLinkSyncErrorSignature = details
    addNotification({
        title: formatMessage(messages.directLinkSyncIssuesTitle),
        text: details,
        type: 'warning',
    })
}

function startDirectLinkSync() {
    const readRoots = () => {
        try {
            const parsed = JSON.parse(localStorage.getItem('axolotl-minecraft-directories') ?? '[]')
            return Array.isArray(parsed)
                ? parsed.flatMap((value) => {
                      if (typeof value === 'string' && value.trim()) {
                          return [{ path: value, mode: 'isolated' as const }]
                      }
                      if (
                          value &&
                          typeof value === 'object' &&
                          typeof value.path === 'string' &&
                          value.path.trim()
                      ) {
                          return [
                              {
                                  path: value.path,
                                  mode:
                                      value.mode === 'shared'
                                          ? ('shared' as const)
                                          : value.mode === 'automatic'
                                            ? ('automatic' as const)
                                            : ('isolated' as const),
                              },
                          ]
                      }
                      return []
                  })
                : []
        } catch {
            return []
        }
    }
    stopDirectLinkSync?.()
    stopDirectLinkSync = startConfiguredDirectLinkSync(readRoots, handleError)
}

onUnmounted(async () => {
    appUnmounted = true
    windowAppearance.dispose()
    stopNativeEditMenu?.()
    if (maximizedStateTimer) clearTimeout(maximizedStateTimer)
    unlistenWindowResize?.()
    window.removeEventListener('keydown', handleGlobalKeydown, true)
    window.removeEventListener('mousedown', handleShortcutMouse, true)
    window.removeEventListener('wheel', handleShortcutWheel, { capture: true, passive: false })
    unlistenCloseRequested?.()
    unlistenLightweightModeError?.()
    unlistenSystemAccentColor?.()
    document.querySelector('body').removeEventListener('click', handleClick)
    document.querySelector('body').removeEventListener('auxclick', handleAuxClick)
    window.removeEventListener(DIRECT_LINKS_SYNCED_EVENT, handleDirectLinkSyncReport)
    window.removeEventListener(
        'modrinth-console-fullscreen',
        handleConsoleFullscreen as EventListener,
    )
    clearDelayedUpdatePopup()
    stopDirectLinkSync?.()
    await unlistenUpdateDownload?.()
    downloadManager.dispose()
})

const { formatMessage } = useVIntl()
const formatBytes = useFormatBytes()

async function warnIfRunningElevated() {
    if (await isElevated().catch(() => false)) {
        addNotification({
            title: formatMessage(messages.runningAsAdmin),
            type: 'warning',
            autoCloseMs: null,
        })
    }
}

async function onImportFileReceived({
    file: _file,
    filePath,
    source: _source,
}: {
    file: File | null
    filePath: string | null
    source: 'file-picker' | 'drag-drop'
}) {
    if (!filePath) return

    const fileName = filePath.split(/[/\\]/).pop() || 'file'

    // ── Hide creation modal first ──
    installationModal.value?.hide()

    // ── Show "Processing..." (matches drag-drop behavior) ──
    const processingNotify = addNotification({
        title: formatMessage(messages.dropProcessing, { name: fileName }),
        type: 'info',
        autoCloseMs: null,
    })

    try {
        // ── Classify the file (same entry point as drag-drop) ──
        const classification = await classifyDroppedItem(filePath)
        clearDropProcessingNotification()
        notificationManager.removeNotification(processingNotify.id)

        // ── Set drop state so handleDropConfirm can read it ──
        dropClassification.value = classification
        dropFilePath.value = classification.file_path ?? classification.base_path ?? filePath
        dropFileName.value = fileName

        // ── Unknown + nested archives → confirm unpacking first ──
        if (
            classification.item_type === 'unknown' &&
            classification.reason?.toLowerCase().includes('nested')
        ) {
            showNestedUnpackPrompt(classification)
            return
        }

        // ── Unknown + extraction → force analysis prompt ──
        if (
            classification.item_type === 'unknown' &&
            classification.reason?.toLowerCase().includes('extraction')
        ) {
            showForceAnalysisPrompt(classification)
            return
        }

        // ── Unknown (no extraction) → error ──
        if (classification.item_type === 'unknown') {
            addNotification({
                title: formatMessage(messages.dropUnknownTitle),
                text: unknownReasonMessage(classification.reason),
                type: 'error',
            })
            return
        }

        // ── Known types → show the same confirm modal as drag-drop ──
        confirmDropModal.value?.show()
    } catch (e) {
        notificationManager.removeNotification(processingNotify?.id)
        addNotification({
            title: formatMessage(messages.dropProcessFailedTitle),
            text: e instanceof Error ? e.message : String(e),
            type: 'error',
        })
    }
}

const messages = defineMessages({
    modrinthAccount: { id: 'app.account.modrinth-account', defaultMessage: 'Modrinth account' },
    signInModrinth: {
        id: 'app.account.sign-in-modrinth',
        defaultMessage: 'Sign in to a Modrinth account',
    },
    updateInstalledToastTitle: {
        id: 'app.update.complete-toast.title',
        defaultMessage: 'Version {version} was successfully installed!',
    },
    updateInstalledToastText: {
        id: 'app.update.complete-toast.text',
        defaultMessage: 'Click here to view the changelog.',
    },
    directLinkSyncIssuesTitle: {
        id: 'app.direct-link-sync.issues-title',
        defaultMessage: 'External Minecraft instances need attention',
    },
    authUnreachableHeader: {
        id: 'app.auth-servers.unreachable.header',
        defaultMessage: 'Cannot reach authentication servers',
    },
    authUnreachableBody: {
        id: 'app.auth-servers.unreachable.body',
        defaultMessage:
            'Minecraft authentication servers may be down right now. Check your internet connection and try again later.',
    },
    quitLauncher: {
        id: 'app.initialization.quit',
        defaultMessage: 'Quit launcher',
    },
    runningAsAdmin: {
        id: 'app.warning.running-as-admin',
        defaultMessage:
            'Axolotl is running as administrator. Drag-and-drop file import is disabled in this mode; please restart the launcher without administrator privileges.',
    },
    restarting: {
        id: 'app.restarting',
        defaultMessage: 'Restarting...',
    },
    closeLauncherTitle: {
        id: 'app.close-launcher.title',
        defaultMessage: 'Choose how to close Axolotl Launcher',
    },
    closeLauncherDirect: {
        id: 'app.close-launcher.direct',
        defaultMessage: 'Close directly',
    },
    closeLauncherTray: {
        id: 'app.close-launcher.tray',
        defaultMessage: 'Hide to tray',
    },
    closeLauncherRemember: {
        id: 'app.close-launcher.remember',
        defaultMessage: 'Remember my choice',
    },
    backupExitTitle: {
        id: 'app.backup-exit.title',
        defaultMessage: 'Backup operations are still running',
    },
    backupExitBody: {
        id: 'app.backup-exit.body',
        defaultMessage:
            '{count, plural, one {# backup operation is} other {# backup operations are}} still running. Keep the launcher open or hide it to the tray to let them finish.',
    },
    backupExitRepositoryMoveBody: {
        id: 'app.backup-exit.repository-move-body',
        defaultMessage:
            'The backup repository is being moved. This task cannot be cancelled or force-stopped; keep the launcher open or hide it to the tray until it finishes.',
    },
    backupExitReturn: { id: 'app.backup-exit.return', defaultMessage: 'Return to launcher' },
    backupExitTray: { id: 'app.backup-exit.tray', defaultMessage: 'Hide to tray' },
    backupExitCancel: {
        id: 'app.backup-exit.cancel',
        defaultMessage: 'Cancel tasks and exit',
    },
    backupExitForce: { id: 'app.backup-exit.force', defaultMessage: 'Force exit' },
    backupExitTimeout: {
        id: 'app.backup-exit.timeout',
        defaultMessage: 'Timed out while waiting for backup operations to stop.',
    },
    home: {
        id: 'app.navigation.home',
        defaultMessage: 'Home',
    },
    worlds: {
        id: 'app.navigation.worlds',
        defaultMessage: 'Worlds',
    },
    discoverContent: {
        id: 'app.navigation.discover-content',
        defaultMessage: 'Discover content',
    },
    skinSelector: {
        id: 'app.navigation.skin-selector',
        defaultMessage: 'Skin selector',
    },
    library: {
        id: 'app.navigation.library',
        defaultMessage: 'Library',
    },
    multiplayer: {
        id: 'app.navigation.multiplayer',
        defaultMessage: 'Multiplayer',
    },
    downloads: {
        id: 'app.navigation.downloads',
        defaultMessage: 'Downloads',
    },
    screenshots: {
        id: 'app.navigation.screenshots',
        defaultMessage: 'Screenshots',
    },
    lab: {
        id: 'app.navigation.lab',
        defaultMessage: 'Lab',
    },
    createInstance: {
        id: 'app.navigation.create-instance',
        defaultMessage: 'Create new instance',
    },
    signedInAs: {
        id: 'app.account.signed-in-as',
        defaultMessage: 'Signed in as',
    },
    playingAs: {
        id: 'app.minecraft.playing-as',
        defaultMessage: 'Playing as',
    },
    collapseSidebar: {
        id: 'app.sidebar.collapse',
        defaultMessage: 'Collapse sidebar',
    },
    expandSidebar: {
        id: 'app.sidebar.expand',
        defaultMessage: 'Expand sidebar',
    },
    warning: {
        id: 'app.notification.warning',
        defaultMessage: 'Warning',
    },
    exportErrorLogs: {
        id: 'app.notification.export-error-logs',
        defaultMessage: 'Export error logs',
    },
    copyPagePath: {
        id: 'app.page-path.copy',
        defaultMessage: 'Copy page path',
    },
    pagePathCopied: {
        id: 'app.page-path.copied',
        defaultMessage: 'Copied',
    },

    // ── Drop / import notification messages ──
    dropOverlayTitle: {
        id: 'app.drop.overlay-title',
        defaultMessage: 'Drop to import',
    },
    dropOverlaySubtitle: {
        id: 'app.drop.overlay-subtitle',
        defaultMessage: 'Release to analyze',
    },
    dropProcessing: {
        id: 'app.drop.processing',
        defaultMessage: 'Processing {name}...',
    },
    dropMultipleFilesTitle: {
        id: 'app.drop.error.multiple-files-title',
        defaultMessage: 'Cannot import multiple files',
    },
    dropMultipleFilesText: {
        id: 'app.drop.error.multiple-files-text',
        defaultMessage: 'Please drop one file at a time.',
    },
    dropShortcutFailedTitle: {
        id: 'app.drop.error.shortcut-title',
        defaultMessage: 'Shortcut resolution failed',
    },
    dropShortcutFailedText: {
        id: 'app.drop.error.shortcut-text',
        defaultMessage: 'Could not resolve the shortcut target.',
    },
    dropUnknownTitle: {
        id: 'app.drop.error.unknown-title',
        defaultMessage: 'Unknown file type',
    },
    dropUnknownText: {
        id: 'app.drop.error.unknown-text',
        defaultMessage: 'Could not determine what kind of file this is.',
    },
    dropUnknownDepthText: {
        id: 'app.drop.error.unknown-depth-text',
        defaultMessage:
            'The archive is nested too deeply to analyze. Unpack it to a folder and try again.',
    },
    dropUnknownEncryptedText: {
        id: 'app.drop.error.unknown-encrypted-text',
        defaultMessage: 'The archive contains encrypted files and cannot be analyzed.',
    },
    dropNestedUnpackTitle: {
        id: 'app.drop.nested-unpack-title',
        defaultMessage: 'Nested archives detected',
    },
    dropNestedUnpackText: {
        id: 'app.drop.nested-unpack-text',
        defaultMessage:
            'This archive contains nested archives ({size}) that must be unpacked to analyze. This may take some time. Continue?',
    },
    dropNestedUnpackButton: {
        id: 'app.drop.nested-unpack-button',
        defaultMessage: 'Continue analysis',
    },
    dropErrorTitle: {
        id: 'app.drop.error.title',
        defaultMessage: 'Drop error',
    },
    dropWorldImportedTitle: {
        id: 'app.drop.world-imported-title',
        defaultMessage: 'World imported',
    },
    dropWorldImportedText: {
        id: 'app.drop.world-imported-text',
        defaultMessage: 'World save has been imported successfully.',
    },
    dropContentInstalledTitle: {
        id: 'app.drop.content-installed-title',
        defaultMessage: 'Content installed',
    },
    dropContentInstalledText: {
        id: 'app.drop.content-installed-text',
        defaultMessage: 'File has been installed to the instance.',
    },
    dropInstallFailedTitle: {
        id: 'app.drop.install-failed-title',
        defaultMessage: 'Installation failed',
    },
    dropInstanceImportedTitle: {
        id: 'app.drop.instance-imported-title',
        defaultMessage: 'Instance imported',
    },
    dropInstanceImportedText: {
        id: 'app.drop.instance-imported-text',
        defaultMessage: '{name} imported successfully.',
    },
    dropImportFailedTitle: {
        id: 'app.drop.import-failed-title',
        defaultMessage: 'Import failed',
    },
    dropImportFailedText: {
        id: 'app.drop.import-failed-text',
        defaultMessage: 'Failed to import {name}: {error}',
    },
    dropNoInstances: {
        id: 'app.drop.no-instances',
        defaultMessage: 'No instances found',
    },
    dropScanning: {
        id: 'app.drop.scanning',
        defaultMessage: 'Scanning for instances',
    },
    dropScanFailed: {
        id: 'app.drop.scan-failed',
        defaultMessage: 'Failed to scan for instances',
    },
    dropExtractFailed: {
        id: 'app.drop.extract-failed',
        defaultMessage: 'Failed to extract archive',
    },
    dropProcessFailedTitle: {
        id: 'app.drop.process-failed-title',
        defaultMessage: 'Failed to process file',
    },
    dropTemporaryFileTitle: {
        id: 'app.drop.temporary-file-title',
        defaultMessage: 'Temporary file detected',
    },
    dropTemporaryFileText: {
        id: 'app.drop.temporary-file-text',
        defaultMessage:
            'The file "{file}" appears to be a temporary copy. Try dragging the file from its original folder instead of from a browser, archive, or cloud storage.',
    },
    dropImportProgressTitle: {
        id: 'app.drop.import-progress-title',
        defaultMessage: 'Importing instances…',
    },
    dropImportProgressText: {
        id: 'app.drop.import-progress-text',
        defaultMessage: '{current} / {total} instances imported',
    },
    dropImportCompletedTitle: {
        id: 'app.drop.import-completed-title',
        defaultMessage: 'Import completed',
    },
    dropImportCompletedText: {
        id: 'app.drop.import-completed-text',
        defaultMessage: 'Successfully imported {count} instances',
    },
    dropImportCompletedPartialText: {
        id: 'app.drop.import-completed-partial-text',
        defaultMessage: 'Imported {completed} of {total} instances ({failed} failed)',
    },
    dropImportCancelledTitle: {
        id: 'app.drop.batch.import-cancelled-title',
        defaultMessage: 'Import cancelled',
    },
    dropImportCancelledText: {
        id: 'app.drop.batch.import-cancelled-text',
        defaultMessage: 'Nothing was imported.',
    },
    dropBatchNothingImportableTitle: {
        id: 'app.drop.batch.nothing-importable-title',
        defaultMessage: 'Nothing to import',
    },
    dropBatchNothingImportableText: {
        id: 'app.drop.batch.nothing-importable-text',
        defaultMessage: '{count, plural, one {# file} other {# files}} could not be recognized.',
    },
    dropBatchCompletedTitle: {
        id: 'app.drop.batch.completed-title',
        defaultMessage: 'Import finished',
    },
    dropBatchCompletedText: {
        id: 'app.drop.batch.completed-text',
        defaultMessage: 'Imported {succeeded} of {total} ({failed} failed, {skipped} skipped).',
    },
    dropBatchTargetLabel: {
        id: 'app.drop.batch.target-label',
        defaultMessage: 'Select target instance for this batch',
    },
    dropBatchGroupFileLabel: {
        id: 'app.drop.batch.group-file-label',
        defaultMessage: '{count, plural, one {# file} other {# files}}: {names}',
    },

    dropModpackInstallFailed: {
        id: 'app.drop.modpack-install-failed',
        defaultMessage: 'Failed to install modpack',
    },

    dropUnknownForceAnalysisTitle: {
        id: 'app.drop.unknown-force-analysis-title',
        defaultMessage: 'Unable to identify file type',
    },
    dropUnknownForceAnalysisText: {
        id: 'app.drop.unknown-force-analysis-text',
        defaultMessage:
            'This archive needs to be extracted and deeply analyzed to determine its content type. This may take a while. Force analysis?',
    },
    dropUnknownForceAnalysisButton: {
        id: 'app.drop.unknown-force-analysis-button',
        defaultMessage: 'Force analysis',
    },
    dropUnknownForceAnalyzing: {
        id: 'app.drop.unknown-force-analyzing',
        defaultMessage: 'Force analyzing archive...',
    },
    dropUnknownForceAnalysisFailedTitle: {
        id: 'app.drop.unknown-force-analysis-failed-title',
        defaultMessage: 'Analysis failed',
    },
    dropUnknownForceAnalysisFailedText: {
        id: 'app.drop.unknown-force-analysis-failed-text',
        defaultMessage: 'Could not identify the file type even after deep analysis.',
    },

    dropInstallModTitle: {
        id: 'app.drop.mod-compatibility-title',
        defaultMessage: 'Version Mismatch',
    },
    dropInstallModWarning: {
        id: 'app.drop.mod-compatibility-warning',
        defaultMessage:
            '{file} targets {modVersion} ({modLoader}), but the instance is {instVersion} ({instLoader}).',
    },

    // Compatible mode (pre-version-isolation) import
    dropCompatibleModeTitle: {
        id: 'app.drop.compatible-mode-title',
        defaultMessage: 'This appears to be a pre-version-isolation instance',
    },
    dropCompatibleModeDesc: {
        id: 'app.drop.compatible-mode-desc',
        defaultMessage: 'Would you like to import this as a compatible mode instance?',
    },
    dropCompatibleModeImport: {
        id: 'app.drop.compatible-mode-import',
        defaultMessage: 'Compatible Import',
    },
    dropCompatibleModeOldWay: {
        id: 'app.drop.compatible-mode-old-way',
        defaultMessage: 'Import as Old Version',
    },
    dropCompatibleModeCancel: {
        id: 'app.drop.compatible-mode-cancel',
        defaultMessage: 'Cancel',
    },
    errorOccurred: {
        id: 'app.notification.error-occurred',
        defaultMessage: 'An error occurred',
    },
})

notificationManager.errorTitleProvider = () => formatMessage(messages.errorOccurred)
notificationManager.errorTextFormatter = (error) =>
    fileOperationError(error, getOsType(), formatMessage) ??
    (isResourceOperationError(error) ? resourceOperationError(error, formatMessage) : null)

function getErrorNotificationDetails(notification) {
    const details = [notification.title, notification.text, notification.errorCode].filter(Boolean)
    if (notification.supportData) {
        details.push(JSON.stringify(notification.supportData, null, 2))
    }
    return details.join('\n\n')
}

async function exportNotificationErrorLogs(notification) {
    try {
        await exportErrorLogs(getErrorNotificationDetails(notification))
    } catch (error) {
        handleError(error)
    }
}

// 把设置同步到界面（主题/外观）；深链改完设置后复用同一套应用逻辑
async function applyThemeFromSettings(s: Awaited<ReturnType<typeof getSettings>>) {
    themeStore.setThemeState(s.theme)
    await initializeSystemAccentColor()
    themeStore.setAccentColor(s.accent_color)
    themeStore.collapsedNavigation = s.collapsed_navigation
    themeStore.advancedRendering = s.advanced_rendering
    themeStore.hideNametagSkinsPage = s.hide_nametag_skins_page
    themeStore.toggleSidebar = s.toggle_sidebar
    themeStore.customBackgroundPath = s.custom_background_path
    themeStore.customBackgroundBlur = s.custom_background_blur
    themeStore.customBackgroundOpacity = s.custom_background_opacity
    themeStore.customBackgroundComponentOpacity = s.custom_background_component_opacity ?? 100
    themeStore.setCustomBackgroundOpacity()
    themeStore.uiFont = s.ui_font ?? null
    themeStore.monoFont = s.mono_font ?? null
    themeStore.setUiFont()
    themeStore.setMonoFont()
    themeStore.transparentBackground = s.transparent_background
    themeStore.transparentBackgroundOpacity = s.transparent_background_opacity
    themeStore.transparentBackgroundBlur = s.transparent_background_blur
    themeStore.setTransparentBackgroundClass()
    themeStore.setCustomBackgroundClass()
}

const queryClient = useQueryClient()

async function setupApp() {
    const initialSettings = await getSettings()
    queryClient.setQueryData(appSettingsKeys.all, initialSettings)
    void downloadManager.start().catch(handleError)
    const {
        native_decorations,
        theme,
        accent_color,
        locale,
        telemetry,
        telemetry_consent_version,
        discord_rpc,
        collapsed_navigation,
        hide_nametag_skins_page,
        advanced_rendering,
        onboarded,
        default_page,
        toggle_sidebar,
        custom_background_path,
        custom_background_blur,
        custom_background_opacity,
        custom_background_component_opacity,
        ui_font,
        mono_font,
        transparent_background,
        transparent_background_opacity,
        transparent_background_blur,
        home_widget_background_opacity,
        hidden_nav_items,
        sidebar_instance_count,
        auto_hide_downloads_button,
        home_layout,
        minimal_home_instance_id,
        close_behavior,
        developer_mode,
        feature_flags,
        pending_update_toast_for_version,
    } = initialSettings

    // Initialize locale from saved settings. Empty/'system' (or the follow-system
    // flag) keeps tracking the OS language and stores a concrete locale for backend
    // checks that compare against codes like `zh-CN`.
    if (!locale) setFollowSystemLocale(true)
    const localePreparation = applyLocalePreference(locale).then(async (resolvedLocale) => {
        if (!locale || locale !== resolvedLocale) {
            initialSettings.locale = resolvedLocale
            await updateSettings({ locale: resolvedLocale })
        }
    })
    // Attach a handler immediately while window/theme setup proceeds in parallel.
    void localePreparation.catch(console.error)

    const defaultPageRoutes = {
        Home: '/',
        DiscoverContent: `/browse/${getLastBrowseContentProjectType()}`,
        Library: '/library',
    }
    const defaultPageRoute = offline.value ? '/library' : defaultPageRoutes[default_page]
    if (defaultPageRoute && defaultPageRoute !== '/') await router.push(defaultPageRoute)

    os.value = await getOS()
    const dev = await isDev()
    isDevEnvironment.value = dev
    pendingUpdateAnnouncementVersion.value = pending_update_toast_for_version
    if (!onboarded && route.path !== '/') await router.replace('/')
    privacyConsentPending.value = telemetry_consent_version < 1
    showOnboarding.value = false
    onboardingSettings.value = initialSettings

    nativeDecorations.value = native_decorations
    if (os.value !== 'MacOS') await getCurrentWindow().setDecorations(native_decorations)

    themeStore.setThemeState(theme)
    await initializeSystemAccentColor()
    themeStore.setAccentColor(accent_color)
    themeStore.collapsedNavigation = collapsed_navigation
    themeStore.advancedRendering = advanced_rendering
    themeStore.hideNametagSkinsPage = hide_nametag_skins_page
    themeStore.toggleSidebar = toggle_sidebar
    themeStore.customBackgroundPath = custom_background_path
    themeStore.customBackgroundBlur = custom_background_blur
    themeStore.customBackgroundOpacity = custom_background_opacity
    themeStore.customBackgroundComponentOpacity = custom_background_component_opacity ?? 100
    themeStore.setCustomBackgroundOpacity()
    themeStore.uiFont = ui_font ?? null
    themeStore.monoFont = mono_font ?? null
    themeStore.setUiFont()
    themeStore.setMonoFont()
    themeStore.transparentBackground = transparent_background
    themeStore.transparentBackgroundOpacity = transparent_background_opacity
    themeStore.transparentBackgroundBlur = transparent_background_blur
    themeStore.setTransparentBackgroundClass()
    themeStore.setCustomBackgroundClass()
    isMaximized.value = await getCurrentWindow().isMaximized()
    appearanceReady.value = true
    await applyWindowAppearance()
    themeStore.homeWidgetBackgroundOpacity = home_widget_background_opacity ?? 100
    themeStore.setHomeWidgetBackgroundOpacity()
    themeStore.hiddenNavItems = hidden_nav_items ?? []
    themeStore.sidebarInstanceCount = sidebar_instance_count
    themeStore.autoHideDownloadsButton = auto_hide_downloads_button
    themeStore.homeLayout = home_layout
    themeStore.minimalHomeInstanceId = minimal_home_instance_id
    themeStore.closeBehavior = close_behavior
    themeStore.showScrollTop = getShowScrollTop()
    themeStore.quickScrollEnabled = getQuickScrollEnabled()
    themeStore.devMode = developer_mode
    themeStore.featureFlags = feature_flags
    for (const shortcut of SHORTCUT_ACTIONS) {
        if (shortcut.group === 'nav')
            themeStore[shortcut.enabledField] = getNavShortcutEnabled(shortcut.id)
    }
    themeStore.shortcutBindings = resolveAllBindings()
    await localePreparation
    stateInitialized.value = true
    if (privacyConsentPending.value) {
        await nextTick()
        privacyConsentModal.value?.show({
            telemetry,
            discord_rpc,
            consent_version: telemetry_consent_version,
        })
    } else {
        showOnboarding.value = !onboarded
    }

    unlistenWindowResize = await getCurrentWindow().onResized(() => {
        // Display mode/DPI changes can emit a burst of resize events. Coalesce
        // them so WebView2 does not receive one IPC request per event.
        if (maximizedStateTimer) clearTimeout(maximizedStateTimer)
        maximizedStateTimer = setTimeout(async () => {
            maximizedStateTimer = undefined
            try {
                isMaximized.value = await getCurrentWindow().isMaximized()
                await applyWindowAppearance()
            } catch (error) {
                console.warn('Failed to refresh maximized state after resize', error)
            }
        }, 100)
    })

    stopNativeEditMenu?.()
    stopNativeEditMenu = installNativeEditMenuBoundary(!dev)

    const osType = await getOsType()
    if (osType === 'macos') {
        document.getElementsByTagName('html')[0].classList.add('mac')
    } else {
        document.getElementsByTagName('html')[0].classList.add('windows')
    }

    await warning_listener(async (e) => {
        if (e.kind === 'minecraft_crash') {
            await minecraftCrashModal.value?.handleWarning(e)
            return
        }

        addNotification({
            title: formatMessage(messages.warning),
            text: e.message,
            type: 'warning',
        })
    })
    await java_download_confirmation_listener((request) => {
        javaDownloadConfirmationModal.value?.show(request)
    })
    await drop_classify_progress_listener((payload) => {
        if (dropProcessingNotificationId.value === null) return
        const name = payload.currentItem?.split(/[/\\]/).pop() || 'file'
        notificationManager.removeNotification(dropProcessingNotificationId.value)
        dropProcessingNotificationId.value = addNotification({
            title: formatMessage(messages.dropProcessing, { name }),
            type: 'info',
            autoCloseMs: null,
        }).id
    })

    get_opening_command().then(handleCommand)
    fetchCredentials()
}

type SystemAccentColorPayload = {
    hex: string
    r: number
    g: number
    b: number
}

async function initializeSystemAccentColor() {
    try {
        unlistenSystemAccentColor = await listen<SystemAccentColorPayload>(
            'system-accent-color-changed',
            ({ payload }) => themeStore.setSystemAccentColor(payload.hex),
        )
    } catch (error) {
        console.warn('Failed to listen for system accent color changes', error)
    }

    try {
        const color = await invoke<SystemAccentColorPayload>(
            'plugin:system-accent|system_accent_color',
        )
        themeStore.setSystemAccentColor(color.hex)
    } catch (error) {
        themeStore.setSystemAccentUnavailable()
        console.warn('Failed to read the system accent color', error)
    }
}

function startOnboarding(mode = 'main') {
    onboardingReplay.value = false
    onboardingMode.value = mode
    showOnboarding.value = true
}

async function replayOnboarding(mode) {
    onboardingReplay.value = true
    onboardingMode.value = mode
    if (mode === 'main') await router.replace('/')
    if (mode === 'logagent') await router.replace('/settings#launch-defaults')
    showOnboarding.value = true
}

async function finishOnboarding() {
    const wasReplay = onboardingReplay.value
    const settings = onboardingSettings.value ?? (await getSettings())
    if (!onboardingReplay.value) {
        if (onboardingMode.value === 'instance') {
            settings.onboarding_instance_tour_completed = true
        } else if (onboardingMode.value === 'main') {
            settings.onboarded = true
            settings.onboarding_version = 1
        }
        await updateSettings(
            onboardingMode.value === 'instance'
                ? { onboarding_instance_tour_completed: true }
                : { onboarded: true, onboarding_version: 1 },
        )
        onboardingSettings.value = settings
    }
    showOnboarding.value = false
    onboardingReplay.value = false
    if (!wasReplay) await scheduleStartupDialogs()
}

async function skipOnboarding() {
    await finishOnboarding()
}

async function closeOnboardingSettings() {
    if (route.path === '/settings') await router.replace('/')
}

async function handleUpdateAnnouncementClosed(version) {
    if (pendingUpdateAnnouncementVersion.value !== version) return

    const settings = await getSettings()
    if (settings.pending_update_toast_for_version === version) {
        settings.pending_update_toast_for_version = null
        await updateSettings({ pending_update_toast_for_version: null })
    }
    pendingUpdateAnnouncementVersion.value = null
    updateAnnouncementShowing.value = false
    await new Promise((resolve) => setTimeout(resolve, 350))
    await scheduleStartupDialogs()
}

async function scheduleStartupDialogs() {
    if (
        !stateInitialized.value ||
        privacyConsentPending.value ||
        showOnboarding.value ||
        updateAnnouncementShowing.value
    )
        return

    if (pendingUpdateAnnouncementVersion.value && updateAnnouncementModal.value) {
        updateAnnouncementShowing.value = true
        await nextTick()
        updateAnnouncementModal.value.show(pendingUpdateAnnouncementVersion.value)
        return
    }
}

async function handlePrivacyConsentSaved(privacy: PrivacySettings) {
    privacyConsentPending.value = false
    if (onboardingSettings.value) {
        onboardingSettings.value.telemetry = privacy.telemetry
        onboardingSettings.value.discord_rpc = privacy.discord_rpc
        onboardingSettings.value.telemetry_consent_version = privacy.consent_version
    }
    if (!onboardingSettings.value?.onboarded) {
        startOnboarding('main')
    } else {
        await scheduleStartupDialogs()
    }
}

async function previewPrivacyConsentModal() {
    try {
        const current = await getPrivacySettings()
        const privacy = await savePrivacySettings({
            telemetry: false,
            discord_rpc: current.discord_rpc,
            consent_version: 0,
        })
        privacyConsentPending.value = true
        if (onboardingSettings.value) {
            onboardingSettings.value.telemetry = privacy.telemetry
            onboardingSettings.value.discord_rpc = privacy.discord_rpc
            onboardingSettings.value.telemetry_consent_version = privacy.consent_version
        }
        await nextTick()
        privacyConsentModal.value?.show(privacy)
    } catch (error) {
        handleError(error)
    }
}

provide('replayOnboarding', replayOnboarding)
provide(minecraftLaunchErrorKey, async (launchError, payload) => {
    if (isOfflineAccountRestrictedError(launchError)) {
        useError().showAntiPiracyNotice()
        return true
    }
    return (await minecraftCrashModal.value?.handleLaunchError(launchError, payload)) ?? false
})
provide('previewMinecraftCrashModal', () => minecraftCrashModal.value?.showPreview())
const remoteAnnouncementPreview = ref<InstanceType<typeof RemoteAnnouncements>>()
provide('previewRemoteAnnouncement', (type: 'modal' | 'notification', withAction = false) => {
    remoteAnnouncementPreview.value?.preview(type, withAction)
})
provide('previewPrivacyConsentModal', previewPrivacyConsentModal)
provide('previewUpdateAnnouncement', (version = null) => {
    const previewVersion = version ?? pendingUpdateAnnouncementVersion.value
    if (previewVersion) updateAnnouncementModal.value?.show(previewVersion)
})

const stateFailed = ref(false)
stateInitialization
    .then(() => {
        const scannerSettings = getMissingContentScannerSettings()
        void configureCurseForgeManualDownloadWatcher(
            scannerSettings.enabled,
            scannerSettings.directory,
        ).catch((error) => {
            console.warn('Failed to configure manual-download watcher', error)
        })
        setupApp().catch((err) => {
            stateFailed.value = true
            console.error(err)
            error.showError(err, null, true, 'state_init')
        })
    })
    .catch((err) => {
        stateFailed.value = true
        console.error('Failed to initialize app', err)
        error.showError(err, null, true, 'state_init')
    })

/**
 * Exits a launcher that failed to initialize.
 *
 * The window is frameless and the app shell (which owns the only window
 * controls) never rendered, so without this the user has no way out short of
 * the task manager. It deliberately skips `closeWindowImmediately`, which
 * saves window state first and would hit the same broken initialization.
 */
async function forceExit() {
    try {
        await invoke('exit_app', { force: true })
    } catch (error) {
        // Closing the window still reaches the exit path, one dialog later.
        console.error('Failed to exit the launcher; closing the window', error)
        await getCurrentWindow()
            .close()
            .catch(() => {})
    }
}

async function showBackupExitModalIfNeeded() {
    activeBackupOperations.value = await listBackupOperations(undefined, true)
    if (activeBackupOperations.value.length === 0) return false
    allowWindowClose = false
    closeRequestInProgress.value = false
    closeChoiceModal.value?.hide()
    backupExitModal.value?.show()
    return true
}

async function exitLauncher(force = false) {
    if (!force && (await showBackupExitModalIfNeeded())) return
    allowWindowClose = true
    await saveWindowState(StateFlags.ALL)
    await invoke('exit_app', { force })
}

async function closeWindowImmediately() {
    if (closeRequestInProgress.value) return
    closeRequestInProgress.value = true
    try {
        await exitLauncher()
    } catch (error) {
        allowWindowClose = false
        closeRequestInProgress.value = false
        handleError(error)
    }
}

async function enterLightweightModeOnClose() {
    closeRequestInProgress.value = true
    try {
        await saveWindowState(StateFlags.ALL)
        await invoke('lightweight_mode_enter')
    } catch (error) {
        allowWindowClose = false
        closeRequestInProgress.value = false
        handleError(error)
    }
}

async function applyCloseChoice(choice: 'close' | 'lightweight', remember: boolean) {
    if (closeRequestInProgress.value) return
    closeRequestInProgress.value = true
    const previousCloseBehavior = themeStore.closeBehavior
    let closeBehaviorPersisted = false
    try {
        if (remember) {
            await updateSettings({ close_behavior: choice })
            themeStore.closeBehavior = choice
            closeBehaviorPersisted = true
        }
        if (choice === 'close') {
            await exitLauncher()
        } else {
            await enterLightweightModeOnClose()
        }
    } catch (error) {
        if (remember && !closeBehaviorPersisted) {
            themeStore.closeBehavior = previousCloseBehavior
        }
        allowWindowClose = false
        closeRequestInProgress.value = false
        closeChoiceOpen.value = true
        handleError(error)
    }
}

async function cancelBackupsAndExit() {
    if (closeRequestInProgress.value) return
    closeRequestInProgress.value = true
    try {
        await Promise.all(
            activeBackupOperations.value
                .filter((operation) => operation.cancellable)
                .map((operation) => cancelBackup(operation.id)),
        )
        for (let attempt = 0; attempt < 300; attempt++) {
            const remaining = await listBackupOperations(undefined, true)
            if (remaining.length === 0) {
                await exitLauncher()
                return
            }
            await new Promise((resolve) => setTimeout(resolve, 100))
        }
        throw new Error(formatMessage(messages.backupExitTimeout))
    } catch (error) {
        closeRequestInProgress.value = false
        handleError(error)
    }
}

async function forceBackupExit() {
    if (closeRequestInProgress.value) return
    closeRequestInProgress.value = true
    try {
        await exitLauncher(true)
    } catch (error) {
        allowWindowClose = false
        closeRequestInProgress.value = false
        handleError(error)
    }
}

const hasActiveRepositoryMove = computed(() =>
    activeBackupOperations.value.some(
        (operation) => operation.operation_type === 'repository_move',
    ),
)

function returnFromBackupExit() {
    backupExitModal.value?.hide()
    activeBackupOperations.value = []
    closeRequestInProgress.value = false
}

async function hideDuringBackups() {
    backupExitModal.value?.hide()
    closeRequestInProgress.value = false
    await enterLightweightModeOnClose()
}

function onCloseChoiceModalHide() {
    // Closing the choice dialog is a cancellation; it must not trigger either
    // close behavior and must allow a subsequent close request to show it again.
    closeChoiceOpen.value = false
    closeChoiceRemember.value = false
}

async function handleCloseRequested(event: { preventDefault: () => void }) {
    if (allowWindowClose) return
    event.preventDefault()
    if (closeRequestInProgress.value) return
    if (closeChoiceOpen.value) return

    const behavior = themeStore.closeBehavior
    if (behavior === 'close') {
        await closeWindowImmediately()
        return
    }
    if (behavior === 'lightweight') {
        await enterLightweightModeOnClose()
        return
    }

    closeChoiceOpen.value = true
    closeChoiceRemember.value = false
    closeChoiceModal.value?.show()
}

const handleClose = closeWindowImmediately

const loading = setupLoadingStateProvider()
loading.setEnabled(false)
let initialLoadToken = loading.begin()
let routerToken = null
let suspenseToken = null
let lastDiscordActivity = null
let discordActivityUpdate = Promise.resolve()

let suspensePending = false

const sidebarOverlayScrollbarsOptions = Object.freeze({
    overflow: {
        x: 'hidden',
        y: 'scroll',
    },
})

router.beforeEach(() => {
    suspensePending = false
    if (routerToken) loading.end(routerToken)
    routerToken = loading.begin()
})

function syncDiscordActivity(to: RouteLocationNormalizedLoaded) {
    const activity =
        typeof to.meta.discordActivity === 'string' ? to.meta.discordActivity : 'Idling...'
    if (activity === lastDiscordActivity) return

    lastDiscordActivity = activity
    discordActivityUpdate = discordActivityUpdate
        .then(() => set_discord_activity(activity))
        .catch((error) => {
            if (lastDiscordActivity === activity) lastDiscordActivity = null
            console.error('Failed to update Discord activity', error)
        })
}

router.afterEach((to, from, failure) => {
    if (!failure) void invoke('lightweight_mode_set_route', { route: to.fullPath })
    trackEvent('PageView', {
        path: to.path,
        fromPath: from.path,
        failed: failure,
    })
    if (!failure) {
        if (stateInitialized.value) syncDiscordActivity(to)
    }
    setTimeout(() => {
        if (!suspensePending && stateInitialized.value) {
            if (initialLoadToken) {
                loading.end(initialLoadToken)
                initialLoadToken = null
            }
            if (routerToken) {
                loading.end(routerToken)
                routerToken = null
            }
        }
    }, 100)
})

function onSuspensePending() {
    suspensePending = true
    if (suspenseToken) loading.end(suspenseToken)
    suspenseToken = loading.begin()
}

function onSuspenseResolve() {
    if (suspenseToken) {
        loading.end(suspenseToken)
        suspenseToken = null
    }
    if (routerToken) {
        loading.end(routerToken)
        routerToken = null
    }
}

watch(
    stateInitialized,
    (ready) => {
        if (ready) {
            syncDiscordActivity(router.currentRoute.value)
            if (initialLoadToken) {
                loading.end(initialLoadToken)
                initialLoadToken = null
            }
            if (routerToken) {
                loading.end(routerToken)
                routerToken = null
            }
            void scheduleStartupDialogs()
        }
    },
    { flush: 'post' },
)

watch(offline, (isOffline) => {
    if (isOffline && (route.path.startsWith('/browse') || route.path.startsWith('/project'))) {
        void router.push('/library')
    }
})

watch(
    () => route.path,
    (path) => {
        if (
            path.startsWith('/instance/') &&
            onboardingSettings.value?.onboarded &&
            !onboardingSettings.value?.onboarding_instance_tour_completed &&
            !showOnboarding.value
        ) {
            startOnboarding('instance')
        }
    },
)

const error = useError()
error.setMinecraftLaunchErrorHandler((launchError, context) => {
    if (!minecraftCrashModal.value?.isLaunchFailure(launchError) || !context?.instanceId)
        return false
    void minecraftCrashModal.value.handleLaunchError(launchError, {
        instance_id: context.instanceId,
        instance_name: 'Minecraft',
    })
    return true
})
const errorModal = ref()
const antiPiracyNoticeModal = ref<InstanceType<typeof OfflineAccountRestrictionModal>>()
const minecraftAuthErrorModal = ref()

const contentInstall = createContentInstall({ router, handleError, addNotification })
provideContentInstall(contentInstall)
const {
    instances: contentInstallInstances,
    compatibleLoaders: contentInstallLoaders,
    gameVersions: contentInstallGameVersions,
    loading: contentInstallLoading,
    defaultTab: contentInstallDefaultTab,
    preferredLoader: contentInstallPreferredLoader,
    preferredGameVersion: contentInstallPreferredGameVersion,
    releaseGameVersions: contentInstallReleaseGameVersions,
    projectInfo: contentInstallProjectInfo,
    symlinkTarget: contentInstallSymlinkTarget,
    handleInstallToInstance,
    handleCreateAndInstall,
    handleNavigate: handleContentInstallNavigate,
    handleCancel: handleContentInstallCancel,
    setContentInstallModal,
    setModpackInstallModal: setContentInstallModpackInstallModal,
    handleModpackInstall: handleContentInstallModpackInstall,
    handleModpackInstallCancel: handleContentInstallModpackInstallCancel,
    setCurseForgeManualDownloadsModal: setContentInstallCurseForgeManualDownloadsModal,
    handleCurseForgeManualDownloadsImported: handleContentInstallCurseForgeManualDownloadsImported,
    setIncompatibilityWarningModal: setContentIncompatibilityWarningModal,
    incompatibilityWarningVersions: contentInstallIncompatibilityWarningVersions,
    incompatibilityWarningCurrentGameVersion:
        contentInstallIncompatibilityWarningCurrentGameVersion,
    incompatibilityWarningCurrentLoader: contentInstallIncompatibilityWarningCurrentLoader,
    incompatibilityWarningProjectType: contentInstallIncompatibilityWarningProjectType,
    incompatibilityWarningProjectIconUrl: contentInstallIncompatibilityWarningProjectIconUrl,
    incompatibilityWarningProjectName: contentInstallIncompatibilityWarningProjectName,
    incompatibilityWarningMessage: contentInstallIncompatibilityWarningMessage,
    incompatibilityWarningInstalling: contentInstallIncompatibilityWarningInstalling,
} = contentInstall

const serverInstall = createServerInstall({ router, handleError, popupNotificationManager })
provideServerInstall(serverInstall)
const {
    setAddServerToInstanceModal: setServerAddServerToInstanceModal,
    symlinkTarget: addServerSymlinkTarget,
} = serverInstall

const modInstallModal = ref()
const resourceInstallPreview = ref<InstanceType<typeof ResourceOperationPreviewModal> | null>(null)
const contentInstallPreviewModal = ref<InstanceType<typeof ContentInstallPreviewModal> | null>(null)
const modpackAlreadyInstalledModal = ref()
const contentInstallModpackInstallModal = ref<InstanceType<typeof ModpackInstallModal> | null>(null)
const handleContentInstallModpackDuplicateGoToInstance = (instanceId: string) =>
    router.push(`/instance/${encodeURIComponent(instanceId)}`)
const contentInstallCurseForgeManualDownloadsModal = ref()
const addServerToInstanceModal = ref()
const incompatibilityWarningModal = ref()
const modrinthLoginFlowWaitModal = ref()

// ── Drop import system ──────────────────────────────────────────────────
const dropImport = useDropImport({
    notificationManager,
    popupNotificationManager,
    installModpackFromPath,
    contentInstall,
    fileDrop,
    onSkinsPage,
    onSchematicWorkshopPage,
    onSettingsPage,
    isSchematicFile,
    trackEvent,
    router,
})

// Destructure what we need from dropImport
const {
    // State
    isDragging,
    isProcessing,
    batchActive,
    batchPhase,
    batchItems,
    batchOriginalCount,
    batchScanDone,
    dropClassification,
    dropFileName,
    dropFilePath,
    dropProcessingNotificationId,
    scanningInstances,
    batchGroupKey,
    incompatWarningKey,

    // Modal refs
    serverImportModal,
    confirmDropModal,
    genericInstallModal,
    launcherImportModal,
    symlinkCardsModal,
    dataPackWorldModal,
    compatibleModeConfirmModal,
    incompatibilityWarningModal: dropIncompatibilityWarningModal,

    // Handlers
    handleConfirmDropCancel,
    handleConfirmDropConfirm,
    handleConfirmDropHelp,
    handleCompatibleModeConfirm,
    handleGenericInstall,
    handleGenericInstallCancel,
    handleGenericInstallNavigateCreate,
    handleBatchOrDatapackWorldSelect,
    handleBatchWorldAfterHide,
    onLauncherImportCancelled,
    onImportSelected,
    onSymlinkMethodCancelled,
    onSymlinkMethodConfirmed,
    chooseImportMethod,
    cancelBatchScan,
    handleIncompatibilityWarningUpdate,
    handleIncompatibilityWarningCancel,
    handleDropInstallSearchCompat,

    // Utility
    clearDropProcessingNotification,
    showNestedUnpackPrompt,
    showForceAnalysisPrompt,
    unknownReasonMessage,
} = dropImport

provide('chooseImportMethod', chooseImportMethod)

watch(
    dropIncompatibilityWarningModal,
    (modal) => {
        if (modal) {
            setContentIncompatibilityWarningModal(modal)
        }
    },
    { flush: 'post' },
)

watch(
    incompatibilityWarningModal,
    (modal) => {
        dropIncompatibilityWarningModal.value = modal
    },
    { flush: 'post' },
)

setupAuthProvider(credentials, async (_redirectPath) => {
    if (AxolotlBrandConfig.capabilities.privateModrinthServices) await signIn()
})

async function validateSession(sessionToken) {
    try {
        const response = await tauriFetch(`${getOfficialLabrinthBaseUrl()}/v2/user`, {
            method: 'GET',
            headers: { Authorization: sessionToken },
        })
        if (response.status === 401) return false
        return true
    } catch {
        return true
    }
}

async function fetchCredentials() {
    if (!AxolotlBrandConfig.capabilities.privateModrinthServices) {
        credentials.value = null
        return
    }
    const creds = await getCreds().catch(handleError)
    if (creds && creds.user_id) {
        if (creds.session && !(await validateSession(creds.session))) {
            await logout().catch(handleError)
            credentials.value = null
            return
        }
        creds.user = await get_user(creds.user_id, 'bypass').catch(handleError)
    }
    credentials.value = creds ?? null
}

async function signIn() {
    modrinthLoginFlowWaitModal.value.show()

    try {
        await login()
        await fetchCredentials()
    } catch (error) {
        if (
            typeof error === 'object' &&
            typeof error['message'] === 'string' &&
            error.message.includes('Login canceled')
        ) {
            // Not really an error due to being a result of user interaction, show nothing
        } else {
            handleError(error)
        }
    } finally {
        modrinthLoginFlowWaitModal.value.hide()
    }
}

async function logOut() {
    await logout().catch(handleError)
    await fetchCredentials()
}

onMounted(() => {
    invoke('show_window')

    error.setErrorModal(errorModal.value)
    error.setAntiPiracyNoticeModal(antiPiracyNoticeModal.value)
    error.setMinecraftAuthErrorModal(minecraftAuthErrorModal.value)

    setContentIncompatibilityWarningModal(incompatibilityWarningModal.value)
    dropIncompatibilityWarningModal.value = incompatibilityWarningModal.value
    setContentInstallModal(modInstallModal.value)
    contentSelection.setPreviewModal(contentInstallPreviewModal.value)
    contentSelection.setResourceInstallPreview(resourceInstallPreview.value)
    contentInstall.setResourceInstallPreview(resourceInstallPreview.value)
    setContentInstallModpackInstallModal(contentInstallModpackInstallModal.value!)
    setContentInstallCurseForgeManualDownloadsModal(
        contentInstallCurseForgeManualDownloadsModal.value,
    )
    setModpackAlreadyInstalledModal(modpackAlreadyInstalledModal.value)
    setServerAddServerToInstanceModal(addServerToInstanceModal.value)
    void (async () => {
        try {
            const ready = await invoke<{
                pending_crashes: { instance_id: string; uuid: string }[]
                pending_commands: Parameters<typeof handleCommand>[0][]
            }>('lightweight_mode_frontend_ready', { route: route.fullPath })
            for (const pendingCrash of ready.pending_crashes) {
                const instance = await getInstance(pendingCrash.instance_id).catch(() => null)
                await minecraftCrashModal.value?.handleWarning({
                    message: `Instance ${instance?.name || 'Minecraft'} has crashed`,
                    kind: 'minecraft_crash',
                    instance_id: pendingCrash.instance_id,
                    instance_name: instance?.name || 'Minecraft',
                })
            }
            for (const command of ready.pending_commands) await handleCommand(command)
        } catch (error) {
            handleError(error)
        }
    })()
})

const accounts = ref<InstanceType<typeof AccountsCard> | null>(null)
provide('accountsCard', accounts)

command_listener(handleCommand)

async function handleCommand(e) {
    if (!e) return
    if (e.event === 'OpenRoute') {
        const target = resolveOpenRoute(e.path ?? '', e.query ?? null)
        if (!target) {
            addNotification({
                title: formatMessage(messages.warning),
                text: `Unsupported link: ${e.path ?? ''}`,
                type: 'warning',
            })
            return
        }
        if (
            offline.value &&
            (target.path.startsWith('/browse') || target.path.startsWith('/project'))
        ) {
            await router.push('/library')
            return
        }
        await router.push(target)
        return
    }
    if (e.event === 'OpenSettings') {
        const target = resolveSettingsRoute(e.tab ?? null, e.entry ?? null)
        await router.push({ path: target.path, hash: target.hash })
        if (target.entry) {
            await nextTick()
            const el = document.getElementById(target.entry)
            el?.scrollIntoView({ behavior: 'smooth', block: 'center' })
        }
        return
    }
    // 特权动作：离线也允许，但必须过确认弹窗
    if (e.event === 'UpdateSettings' || e.event === 'StopInstance') {
        await handlePrivilegedCommand(e)
        return
    }
    if (offline.value && e.event !== 'LaunchInstance') {
        await router.push('/library')
        return
    }

    if (e.event === 'RunMRPack') {
        // RunMRPack should directly install a local modpack file given a path;
        // non-mrpack archives (CurseForge/MCBBS/HMCL/MultiMC zips) are format-sniffed by the backend
        if (e.path.endsWith('.mrpack') || e.path.endsWith('.zip')) {
            const location = { type: 'fromFile', path: e.path }
            const preview = await install_get_modpack_preview(location).catch(handleError)
            if (preview?.unknownFile) {
                const splitPath = e.path.split(/[\\/]/)
                const fileName = splitPath ? splitPath[splitPath.length - 1] : e.path
                unknownPackWarningModal.value?.show(
                    () => install_create_modpack_instance(location).then(() => undefined),
                    fileName,
                )
            } else {
                await install_create_modpack_instance(location).catch(handleError)
            }
            trackEvent('InstanceCreate', {
                source: 'CreationModalFileDrop',
            })
        }
    } else if (e.event === 'LaunchInstance') {
        const instance = await getInstance(e.id).catch(() => null)
        const handleLaunchCommandError = async (launchError) => {
            if (isOfflineAccountRestrictedError(launchError)) {
                error.showAntiPiracyNotice()
                return
            }
            const handled =
                (await minecraftCrashModal.value?.handleLaunchError(launchError, {
                    instance_id: e.id,
                    instance_name: instance?.name || 'Minecraft',
                })) ?? false
            if (!handled) handleError(launchError)
        }
        if (e.server) {
            await start_join_server(e.id, e.server).catch(handleLaunchCommandError)
        } else if (e.singleplayer_world) {
            await start_join_singleplayer_world(e.id, e.singleplayer_world).catch(
                handleLaunchCommandError,
            )
        } else {
            await run(e.id).catch(handleLaunchCommandError)
        }
    } else if (e.event === 'InstallVersion') {
        const version = await get_version(e.id, 'must_revalidate').catch(handleError)
        if (version) {
            await contentInstall
                .install(
                    version.project_id,
                    version.id,
                    null,
                    'URLConfirmModal',
                    undefined,
                    undefined,
                    {
                        showProjectInfo: true,
                    },
                )
                .catch(handleError)
        }
    } else {
        await contentInstall
            .install(e.id, null, null, 'URLConfirmModal', undefined, undefined, {
                showProjectInfo: true,
            })
            .catch(handleError)
    }
}

const privilegedMessages = defineMessages({
    applied: {
        id: 'app.privileged-modal.applied',
        defaultMessage: 'Privileged link applied',
    },
    stopped: {
        id: 'app.privileged-modal.stopped',
        defaultMessage: 'Stop request sent',
    },
    noProcesses: {
        id: 'app.privileged-modal.no-processes',
        defaultMessage: 'No running game processes for this instance',
    },
})

// 启用开关时已强制阅读确认，执行不再逐次确认
async function handlePrivilegedCommand(e) {
    if (e.event === 'UpdateSettings') {
        const latest = await getSettings().catch(handleError)
        if (!latest) return
        const before = JSON.parse(JSON.stringify(latest))
        if (!applySettingChanges(latest, e.changes ?? [])) return
        await updateSettings(diffSettings(before, latest)).catch(handleError)
        await applyThemeFromSettings(latest)
        setFollowSystemLocale(!latest.locale || latest.locale === 'system')
        await applyLocalePreference(latest.locale)
        addNotification({
            title: formatMessage(privilegedMessages.applied),
            type: 'success',
        })
        return
    }
    const processes = await get_by_instance_id(e.instance_id).catch(() => [])
    if (!processes.length) {
        addNotification({
            title: formatMessage(privilegedMessages.noProcesses),
            type: 'warning',
        })
        return
    }
    for (const process of processes) await killProcess(process.uuid).catch(handleError)
    addNotification({
        title: formatMessage(privilegedMessages.stopped),
        type: 'success',
    })
}

const updatePopupMessages = defineMessages({
    updateAvailable: {
        id: 'app.update-popup.title',
        defaultMessage: 'Update available',
    },
    downloadComplete: {
        id: 'app.update-popup.download-complete',
        defaultMessage: 'Download complete',
    },
    meteredBody: {
        id: 'app.update-popup.body.metered',
        defaultMessage: `Axolotl Launcher v{version} is available now! Since you're on a metered network, we didn't automatically download it.`,
    },
    downloadedBody: {
        id: 'app.update-popup.body.download-complete',
        defaultMessage: `Axolotl Launcher v{version} has finished downloading. Reload to update now, or automatically when you close Axolotl Launcher.`,
    },
    reload: {
        id: 'app.update-popup.reload',
        defaultMessage: 'Reload to update',
    },
    download: {
        id: 'app.update-popup.download',
        defaultMessage: 'Download ({size})',
    },
    changelog: {
        id: 'app.update-popup.changelog',
        defaultMessage: 'Changelog',
    },
})

function clearDelayedUpdatePopup() {
    if (delayedUpdatePopupTimeout !== null) {
        clearTimeout(delayedUpdatePopupTimeout)
        delayedUpdatePopupTimeout = null
    }
}

function getCurrentUpdatePromptStage() {
    return finishedDownloading.value ? 'downloaded' : 'available'
}

function scheduleDelayedUpdatePopup() {
    clearDelayedUpdatePopup()

    const version = availableUpdate.value?.version
    if (!version) {
        return
    }

    const nextPopupTime = getNextAppUpdatePopupTime(version, getCurrentUpdatePromptStage())
    if (nextPopupTime === null) {
        return
    }

    const delay = nextPopupTime - Date.now()
    if (delay <= 0) {
        showDelayedUpdatePopup()
        return
    }

    delayedUpdatePopupTimeout = setTimeout(showDelayedUpdatePopup, Math.min(delay, 2_147_483_647))
}

function showDelayedUpdatePopup() {
    const update = availableUpdate.value
    if (!update) {
        return
    }

    const stage = getCurrentUpdatePromptStage()
    const nextPopupTime = getNextAppUpdatePopupTime(update.version, stage)
    if (nextPopupTime === null) {
        return
    }

    if (Date.now() < nextPopupTime) {
        scheduleDelayedUpdatePopup()
        return
    }

    if (metered.value && !finishedDownloading.value) {
        addPopupNotification({
            title: formatMessage(updatePopupMessages.updateAvailable),
            text: formatMessage(updatePopupMessages.meteredBody, { version: update.version }),
            type: 'info',
            autoCloseMs: null,
            buttons: [
                {
                    label: formatMessage(updatePopupMessages.download, {
                        size: formatBytes(updateSize.value ?? 0),
                    }),
                    action: () => downloadAvailableAppUpdate(),
                    color: 'brand',
                },
                {
                    label: formatMessage(updatePopupMessages.changelog),
                    action: () => openAppUpdateChangelog(),
                    keepOpen: true,
                },
            ],
        })
    } else if (finishedDownloading.value) {
        addPopupNotification({
            title: formatMessage(updatePopupMessages.downloadComplete),
            text: formatMessage(updatePopupMessages.downloadedBody, {
                version: update.version,
            }),
            type: 'success',
            autoCloseMs: null,
            buttons: [
                {
                    label: formatMessage(updatePopupMessages.reload),
                    action: () => installAvailableAppUpdate(),
                    color: 'brand',
                },
                {
                    label: formatMessage(updatePopupMessages.changelog),
                    action: () => openAppUpdateChangelog(),
                    keepOpen: true,
                },
            ],
        })
    } else {
        scheduleDelayedUpdatePopup()
        return
    }

    markAppUpdatePopupShown(update.version, stage)
}

let lastUpdateChannel = 'release'

async function performUpdateCheck() {
    const channel = await getUpdateChannel()
    const preferences = await getUpdatePreferences()
    updatesPaused.value = preferences.updatesPaused
    if (updatesPaused.value) return 'paused'
    if (channel !== lastUpdateChannel) {
        availableUpdate.value = null
        updateSize.value = null
        appUpdateDownload.progress.value = 0
        finishedDownloading.value = false
        downloading.value = false
        lastUpdateChannel = channel
    }

    const update = await checkAppUpdate(channel)
    if (!update) {
        console.log('No update available')
        return 'up-to-date'
    }

    const publishedAt = Date.parse(update.publishedAt ?? '')
    if (
        channel === 'release' &&
        !preferences.immediateUpdateFetch &&
        !update.forceUpdate &&
        (!Number.isFinite(publishedAt) || Date.now() < publishedAt + 24 * 60 * 60 * 1000)
    ) {
        console.warn(
            Number.isFinite(publishedAt)
                ? `Update ${update.version} is waiting for the release delay.`
                : `Update ${update.version} has no valid published_at timestamp.`,
        )
        return 'up-to-date'
    }

    const isExistingUpdate = update.version === availableUpdate.value?.version

    if (isExistingUpdate) {
        console.log('Update is already known')
        scheduleDelayedUpdatePopup()
        return 'available'
    }

    appUpdateDownload.progress.value = 0
    finishedDownloading.value = false
    downloading.value = false
    updateSize.value = null
    availableUpdate.value = update

    console.log(`Update ${update.version} is available.`)

    metered.value = await isNetworkMetered()

    if (!metered.value) {
        console.log('Starting download of update')
        downloadUpdate(update)
    } else {
        console.log(`Metered connection detected, not auto-downloading update.`)
        markAppUpdateActionable(update.version)
        scheduleDelayedUpdatePopup()
    }

    getUpdateSize(update.rid)
        .then((size) => (updateSize.value = size))
        .catch((error) => console.warn('Failed to fetch update size', error))
    return 'available'
}

async function manualUpdateCheck() {
    if (!(await areUpdatesEnabled())) {
        updatesEnabled.value = false
        return 'disabled'
    }

    updatesEnabled.value = true
    updatesPaused.value = (await getUpdatePreferences()).updatesPaused
    if (updatesPaused.value) return 'paused'
    if (offline.value) {
        return 'offline'
    }

    return await performUpdateCheck()
}

async function downloadAvailableUpdate() {
    return downloadUpdate(availableUpdate.value)
}

async function downloadUpdate(versionToDownload) {
    if (!versionToDownload) {
        handleError(`Failed to download update: no version available`)
        return
    }

    if (downloading.value || appUpdateDownload.progress.value !== 0) {
        console.error(`Update ${versionToDownload.version} already downloading`)
        return
    }

    console.log(`Downloading update ${versionToDownload.version} from Update Server`)
    downloading.value = true

    try {
        enqueueUpdateForInstallation(versionToDownload.rid)
            .then(() => {
                downloading.value = false
                finishedDownloading.value = true
                unlistenUpdateDownload?.().then(() => {
                    unlistenUpdateDownload = null
                })
                console.log('Finished downloading!')
                markAppUpdateActionable(versionToDownload.version, 'downloaded')
                scheduleDelayedUpdatePopup()
            })
            .catch((error) => {
                downloading.value = false
                appUpdateDownload.progress.value = 0
                unlistenUpdateDownload?.().then(() => {
                    unlistenUpdateDownload = null
                })
                handleError(error)
            })
        unlistenUpdateDownload = await subscribeToDownloadProgress(
            appUpdateDownload,
            versionToDownload.version,
        )
    } catch (error) {
        downloading.value = false
        appUpdateDownload.progress.value = 0
        handleError(error)
    }
}

async function installUpdate() {
    restarting.value = true

    try {
        await backupAppDbForUpdate(availableUpdate.value?.version)
        await setRestartAfterPendingUpdate(true)
    } catch (e) {
        restarting.value = false
        handleError(e)
        return
    }
    setTimeout(async () => {
        await handleClose()
    }, 250)
}

setAppUpdateActions({
    check: manualUpdateCheck,
    download: downloadAvailableUpdate,
    install: installUpdate,
    changelog: (version) => {
        if (version && getAnnouncementByVersion(version)) {
            updateAnnouncementModal.value?.show(version)
        } else {
            openUrl(AxolotlBrandConfig.website)
        }
    },
})

async function openModrinthProjectLinkInApp(parsed) {
    const { slug, pathSuffix, url } = parsed
    const loadToken = loading.begin()
    try {
        const { id } = await tauriApiClient.labrinth.projects_v2.check(slug)
        const query = mergeUrlQuery(route.query, url)
        await router.push({
            path: `/project/${id}${pathSuffix}`,
            query,
            hash: url.hash || undefined,
        })
    } catch (err) {
        if (err instanceof ModrinthApiError && err.statusCode === 404) {
            openUrl(url.href)
        } else {
            handleError(err)
        }
    } finally {
        loading.end(loadToken)
    }
}

function handleClick(e) {
    let target = e.target
    while (target != null) {
        if (target.matches('a')) {
            // RouterLinks and same-origin SPA paths must keep default handling /
            // vue-router click; only intercept true external protocol links.
            const href = target.getAttribute('href') ?? ''
            const externalUrl = getExternalLinkUrl(href, target.href)
            const isRouterLink =
                target.classList.contains('router-link-active') ||
                (href.startsWith('/') && !externalUrl)
            if (!isRouterLink && externalUrl && !target.classList.contains('router-link-active')) {
                const parsed = parseModrinthLink(externalUrl)
                if (target.target !== '_blank' && parsed) {
                    void openModrinthProjectLinkInApp(parsed)
                } else {
                    openUrl(externalUrl)
                }
                e.preventDefault()
            }
            break
        }
        target = target.parentElement
    }
}

function handleAuxClick(e) {
    // A shortcut answers to this button, so the click is not a click at all.
    if (findShortcut(e, bindingMatchesMouseEvent)) return

    // disables middle click -> new tab
    if (e.button === 1) {
        e.preventDefault()
        // instead do a left click
        const event = new MouseEvent('click', {
            view: window,
            bubbles: true,
            cancelable: true,
        })
        e.target.dispatchEvent(event)
    }
}

provideAppUpdateDownloadProgress(appUpdateDownload)
</script>

<template>
    <SplashScreen v-if="!stateFailed" ref="splashScreen" data-tauri-drag-region />
    <!--
		A failed initialization never renders the app shell, so this framed window
		ends up with no title bar and no controls at all. This strip keeps the
		window draggable and offers a way to quit that does not depend on the
		parts which failed to load.
	-->
    <div
        v-if="stateFailed && !stateInitialized"
        data-tauri-drag-region
        class="fixed inset-x-0 top-0 z-[300] flex h-9 items-center justify-end px-1"
    >
        <button
            v-tooltip.bottom="formatMessage(messages.quitLauncher)"
            data-tauri-drag-region-exclude
            class="flex size-8 cursor-pointer items-center justify-center rounded-md border-0 bg-transparent text-[var(--color-text-tertiary)] transition-colors hover:bg-surface-4 hover:text-[var(--color-text-primary)]"
            type="button"
            :aria-label="formatMessage(messages.quitLauncher)"
            @click="forceExit"
        >
            <XIcon class="size-4" />
        </button>
    </div>
    <div id="teleports"></div>
    <div
        v-if="
            stateInitialized && themeStore.customBackgroundPath && !themeStore.transparentBackground
        "
        class="launcher-background"
    >
        <div class="launcher-background-image" :style="customBackgroundStyle" />
    </div>
    <div
        v-if="stateInitialized"
        class="app-grid-layout relative"
        :class="{
            'disable-advanced-rendering': !themeStore.advancedRendering,
            'has-custom-background':
                themeStore.customBackgroundPath && !themeStore.transparentBackground,
            'has-transparent-background': themeStore.transparentBackground,
            'is-maximized': isMaximized,
            'has-native-decorations': nativeDecorations,
            'has-css-window-border': cssWindowBorder,
        }"
    >
        <Transition name="fade">
            <div
                v-if="restarting"
                data-tauri-drag-region
                class="inset-0 fixed bg-black/80 backdrop-blur z-[200] flex items-center justify-center"
            >
                <span
                    data-tauri-drag-region
                    class="flex items-center gap-4 text-[var(--color-text-primary)] font-semibold text-xl select-none cursor-default"
                >
                    <RefreshCwIcon data-tauri-drag-region class="animate-spin w-6 h-6" />
                    {{ formatMessage(messages.restarting) }}
                </span>
            </div>
        </Transition>
        <Suspense>
            <AuthGrantFlowWaitModal ref="modrinthLoginFlowWaitModal" @flow-cancel="cancelLogin" />
        </Suspense>
        <InstanceIconPickerModal ref="instanceIconPickerModal" />
        <CreationFlowModal
            ref="installationModal"
            type="instance"
            :available-loaders="[...clientInstallableLoaders]"
            show-snapshot-toggle
            :fetch-existing-instance-names="fetchExistingInstanceNames"
            :search-modpacks="searchModpacks"
            :get-project-versions="getProjectVersions"
            :has-compatible-opti-fabric="hasCompatibleOptiFabric"
            :get-loader-manifest="getLoaderManifest"
            :on-import-file-received="onImportFileReceived"
            @create="handleCreate"
            @browse-modpacks="handleBrowseModpacks"
        />
        <UnknownPackWarningModal ref="unknownPackWarningModal" />
        <div
            class="app-grid-navbar bg-surface-bar flex flex-col p-[0.5rem] pt-0 gap-[0.5rem] w-[--left-bar-width] overflow-hidden"
        >
            <NavRail>
                <NavButton
                    v-if="!themeStore.isNavItemHidden('home')"
                    v-tooltip.right="formatMessage(messages.home)"
                    to="/"
                >
                    <HomeIcon />
                </NavButton>
                <NavButton
                    v-if="!themeStore.isNavItemHidden('screenshots')"
                    v-tooltip.right="formatMessage(messages.screenshots)"
                    data-onboarding-id="nav-screenshots"
                    to="/screenshots"
                    :is-primary="(r) => r.path.startsWith('/screenshots')"
                >
                    <ImagesIcon />
                </NavButton>
                <NavButton
                    v-if="
                        themeStore.featureFlags.worlds_tab && !themeStore.isNavItemHidden('worlds')
                    "
                    v-tooltip.right="formatMessage(messages.worlds)"
                    to="/worlds"
                >
                    <WorldIcon />
                </NavButton>
                <NavButton
                    v-if="!themeStore.isNavItemHidden('discover')"
                    v-tooltip.right="formatMessage(messages.discoverContent)"
                    data-onboarding-id="nav-discover"
                    :to="discoverContentPath"
                    :disabled="offline"
                    :is-primary="() => route.path.startsWith('/browse') && !route.query.i"
                    :is-subpage="(route) => route.path.startsWith('/project') && !route.query.i"
                >
                    <CompassIcon />
                </NavButton>
                <NavButton
                    v-if="!themeStore.isNavItemHidden('skins')"
                    v-tooltip.right="formatMessage(messages.skinSelector)"
                    data-onboarding-id="nav-skins"
                    to="/skins"
                >
                    <ChangeSkinIcon />
                </NavButton>
                <NavButton
                    v-if="!themeStore.isNavItemHidden('multiplayer')"
                    v-tooltip.right="formatMessage(messages.multiplayer)"
                    to="/multiplayer"
                    :is-primary="(r) => r.path.startsWith('/multiplayer')"
                >
                    <UsersIcon />
                </NavButton>
                <NavButton
                    v-if="!themeStore.isNavItemHidden('library')"
                    v-tooltip.right="formatMessage(messages.library)"
                    data-onboarding-id="nav-library"
                    to="/library"
                    :is-primary="(r) => r.matched.some((record) => record.name === 'Library')"
                    :is-subpage="
                        () =>
                            route.path.startsWith('/instance') ||
                            ((route.path.startsWith('/browse') ||
                                route.path.startsWith('/project')) &&
                                route.query.i)
                    "
                >
                    <LibraryIcon />
                </NavButton>
                <NavButton
                    v-if="!themeStore.isNavItemHidden('lab')"
                    v-tooltip.right="formatMessage(messages.lab)"
                    data-onboarding-id="nav-lab"
                    to="/lab"
                    :is-primary="(r) => r.path.startsWith('/lab')"
                >
                    <FlaskConicalIcon />
                </NavButton>
            </NavRail>
            <div class="h-px w-6 mx-auto my-2 bg-surface-5"></div>
            <div class="quick-instance-scroll flex-1 min-h-0 overflow-x-hidden overflow-y-auto">
                <suspense>
                    <QuickInstanceSwitcher />
                </suspense>
            </div>
            <NavButton
                v-tooltip.right="formatMessage(messages.createInstance)"
                data-onboarding-id="create-instance"
                to="/create"
                :disabled="offline"
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
            <OverflowMenu
                v-if="AxolotlBrandConfig.capabilities.privateModrinthServices && credentials?.user"
                v-tooltip.right="formatMessage(messages.modrinthAccount)"
                data-onboarding-id="account-entry"
                class="w-12 h-12 text-[var(--color-text-default)] rounded-full flex items-center justify-center text-2xl transition-all bg-transparent hover:bg-surface-4 hover:text-[var(--color-text-primary)] border-0 cursor-pointer"
                :options="[
                    {
                        id: 'view-profile',
                        action: () =>
                            openUrl('https://modrinth.com/user/' + credentials.user.username),
                    },
                    {
                        id: 'sign-out',
                        action: () => logOut(),
                        color: 'danger',
                    },
                ]"
                placement="right-end"
            >
                <Avatar :src="credentials?.user?.avatar_url" alt="" size="32px" circle />
                <template #view-profile>
                    <UserIcon />
                    <span class="inline-flex items-center gap-1">
                        {{ formatMessage(messages.signedInAs) }}
                        <span
                            class="inline-flex items-center gap-1 text-[var(--color-text-primary)] font-semibold"
                        >
                            <Avatar
                                :src="credentials?.user?.avatar_url"
                                alt=""
                                size="20px"
                                circle
                            />
                            {{ credentials?.user?.username }}
                        </span>
                    </span>
                    <ExternalIcon />
                </template>
                <template #sign-out>
                    <LogOutIcon /> {{ formatMessage(commonMessages.signOutButton) }}
                </template>
            </OverflowMenu>
            <NavButton
                v-else-if="AxolotlBrandConfig.capabilities.privateModrinthServices"
                v-tooltip.right="formatMessage(messages.signInModrinth)"
                data-onboarding-id="account-entry"
                :to="() => signIn()"
            >
                <LogInIcon class="text-brand" />
            </NavButton>
        </div>
        <AppTitlebar
            class="app-grid-statusbar bg-surface-bar h-[--top-bar-height]"
            :is-beta-build="isBetaBuild"
        >
            <div class="flex min-w-0 mr-3">
                <Suspense>
                    <AppActionBar />
                </Suspense>
            </div>
            <WindowControls />
        </AppTitlebar>
    </div>
    <div
        v-if="stateInitialized"
        class="app-contents"
        :class="{
            'sidebar-enabled': sidebarVisible,
            'studio-mode':
                route.name === 'FileStudio' || route.name === 'MultiplayerServerFileStudio',
            'disable-advanced-rendering': !themeStore.advancedRendering,
            'has-custom-background':
                themeStore.customBackgroundPath && !themeStore.transparentBackground,
            'has-transparent-background': themeStore.transparentBackground,
        }"
    >
        <div class="app-viewport flex-grow router-view" tabindex="-1">
            <div
                class="loading-indicator-container h-8 fixed z-50 pointer-events-none"
                :style="{
                    top: 'calc(var(--top-bar-height))',
                    left: 'calc(var(--left-bar-width))',
                    width: 'calc(100% - var(--left-bar-width) - var(--right-bar-width))',
                }"
            >
                <LoadingBar position="absolute" />
            </div>
            <button
                v-if="themeStore.featureFlags.page_path"
                v-tooltip="
                    formatMessage(pagePathCopied ? messages.pagePathCopied : messages.copyPagePath)
                "
                type="button"
                class="absolute bottom-0 left-0 m-2 bg-tooltip-bg text-tooltip-text font-semibold rounded-full px-2 py-1 text-xs z-50 cursor-pointer border-0 text-left"
                @click="copyPagePath"
            >
                {{ route.fullPath }}
            </button>
            <div
                id="background-teleport-target"
                class="absolute h-full -z-10 rounded-tl-[--radius-xl] overflow-hidden"
                :style="{
                    width: 'calc(100% - var(--right-bar-width))',
                }"
            ></div>
            <Admonition
                v-if="authUnreachable"
                type="warning"
                :header="formatMessage(messages.authUnreachableHeader)"
                class="m-6 mb-0"
            >
                {{ formatMessage(messages.authUnreachableBody) }}
            </Admonition>
            <div class="page-transition-grid grid min-h-full">
                <RouterView v-slot="{ Component, route: pageRoute }">
                    <!--
						Enter animation is keyed only on the route (see getPageTransitionKey).
						The layer mounts as soon as the URL changes — not when the async page
						Suspense resolves — so nav switches stay smooth while data loads.
					-->
                    <Transition
                        name="page-slide"
                        :css="themeStore.getFeatureFlag('page_transitions') && !onSettingsPage"
                        appear
                    >
                        <div
                            :key="
                                onSettingsPage ? 'settings-shell' : getPageTransitionKey(pageRoute)
                            "
                            class="page-transition-layer"
                        >
                            <Suspense
                                v-if="Component"
                                @pending="onSuspensePending"
                                @resolve="onSuspenseResolve"
                            >
                                <component :is="Component"></component>
                            </Suspense>
                        </div>
                    </Transition>
                </RouterView>
            </div>
            <ScrollToTopButton v-if="themeStore.showScrollTop" />
        </div>
        <div
            class="app-sidebar mt-px shrink-0 flex flex-col border-0 border-l-[1px] border-[--brand-gradient-border] border-solid"
        >
            <button
                v-if="!forceSidebar && !forceSidebarHidden"
                v-tooltip.left="
                    sidebarToggled
                        ? formatMessage(messages.collapseSidebar)
                        : formatMessage(messages.expandSidebar)
                "
                class="sidebar-toggle-handle"
                :aria-label="
                    sidebarToggled
                        ? formatMessage(messages.collapseSidebar)
                        : formatMessage(messages.expandSidebar)
                "
                type="button"
                @click="toggleSidebar"
            >
                <RightArrowIcon
                    class="w-2.5 h-2.5 -translate-x-[1px] transition-transform duration-300"
                    :class="{ 'rotate-180': !sidebarToggled }"
                />
            </button>
            <div
                v-overlay-scrollbars="sidebarOverlayScrollbarsOptions"
                class="app-sidebar-scrollable relative min-h-0 flex-1"
                data-overlayscrollbars-initialize
            >
                <div id="sidebar-teleport-target" class="sidebar-teleport-content contents"></div>
                <div
                    class="sidebar-default-content hidden"
                    :class="{ 'sidebar-enabled': sidebarVisible }"
                >
                    <div
                        class="p-4 border-0 border-b-[1px] border-[--brand-gradient-border] border-solid"
                    >
                        <h3 class="text-base text-[var(--color-text-default)] font-medium m-0">
                            {{ formatMessage(messages.playingAs) }}
                        </h3>
                        <suspense>
                            <AccountsCard ref="accounts" />
                        </suspense>
                    </div>
                    <div id="sidebar-default-teleport-target"></div>
                </div>
            </div>
        </div>
    </div>
    <I18nDebugPanel />
    <NotificationPanel
        :has-sidebar="sidebarVisible"
        :on-error-action="exportNotificationErrorLogs"
        :error-action-label="formatMessage(messages.exportErrorLogs)"
    />
    <PopupNotificationPanel
        :has-sidebar="sidebarVisible"
        :on-error-action="exportNotificationErrorLogs"
        :error-action-label="formatMessage(messages.exportErrorLogs)"
    />
    <MinecraftCrashModal ref="minecraftCrashModal" />
    <JavaDownloadConfirmationModal ref="javaDownloadConfirmationModal" />
    <PrivacyConsentModal ref="privacyConsentModal" @saved="handlePrivacyConsentSaved" />
    <RemoteAnnouncements
        :ready="
            stateInitialized &&
            !privacyConsentPending &&
            !showOnboarding &&
            !updateAnnouncementShowing
        "
    />
    <RemoteAnnouncements
        ref="remoteAnnouncementPreview"
        preview-only
        :ready="
            stateInitialized &&
            !privacyConsentPending &&
            !showOnboarding &&
            !updateAnnouncementShowing
        "
    />
    <UpdateAnnouncementModal
        ref="updateAnnouncementModal"
        @closed="handleUpdateAnnouncementClosed"
    />
    <NewModal
        ref="closeChoiceModal"
        :header="formatMessage(messages.closeLauncherTitle)"
        :closable="true"
        :close-on-click-outside="true"
        :disable-close="closeRequestInProgress"
        :on-hide="onCloseChoiceModalHide"
        max-width="30rem"
    >
        <div class="grid grid-cols-2 gap-3">
            <Button
                type="colored"
                color="brand"
                :disabled="closeRequestInProgress"
                @click="applyCloseChoice('close', closeChoiceRemember)"
                >{{ formatMessage(messages.closeLauncherDirect) }}
            </Button>
            <Button
                :disabled="closeRequestInProgress"
                @click="applyCloseChoice('lightweight', closeChoiceRemember)"
                >{{ formatMessage(messages.closeLauncherTray) }}
            </Button>
        </div>
        <div class="mt-4">
            <Checkbox
                v-model="closeChoiceRemember"
                :disabled="closeRequestInProgress || stateFailed"
                :label="formatMessage(messages.closeLauncherRemember)"
            />
        </div>
    </NewModal>
    <NewModal
        ref="backupExitModal"
        :header="formatMessage(messages.backupExitTitle)"
        :disable-close="closeRequestInProgress"
        fade="danger"
        max-width="32rem"
    >
        <Admonition type="warning">
            {{ formatMessage(messages.backupExitBody, { count: activeBackupOperations.length }) }}
        </Admonition>
        <Admonition v-if="hasActiveRepositoryMove" type="warning" class="mt-3">
            {{ formatMessage(messages.backupExitRepositoryMoveBody) }}
        </Admonition>
        <template #actions>
            <div class="flex flex-wrap items-center justify-end gap-2">
                <Button
                    type="outlined"
                    :disabled="closeRequestInProgress"
                    @click="returnFromBackupExit"
                    >{{ formatMessage(messages.backupExitReturn) }}
                </Button>
                <Button :disabled="closeRequestInProgress" @click="hideDuringBackups"
                    >{{ formatMessage(messages.backupExitTray) }}
                </Button>
                <Button
                    v-if="activeBackupOperations.every((operation) => operation.cancellable)"
                    :disabled="closeRequestInProgress"
                    @click="cancelBackupsAndExit"
                    >{{ formatMessage(messages.backupExitCancel) }}
                </Button>
                <Button
                    v-if="!hasActiveRepositoryMove"
                    type="colored"
                    color="red"
                    :disabled="closeRequestInProgress"
                    @click="forceBackupExit"
                    >{{ formatMessage(messages.backupExitForce) }}
                </Button>
            </div>
        </template>
    </NewModal>
    <ErrorModal ref="errorModal" />
    <OfflineAccountRestrictionModal ref="antiPiracyNoticeModal" @sign-in="accounts?.login()" />
    <MinecraftAuthErrorModal ref="minecraftAuthErrorModal" />
    <ContentInstallModal
        ref="modInstallModal"
        :instances="contentInstallInstances"
        :compatible-loaders="contentInstallLoaders"
        :game-versions="contentInstallGameVersions"
        :loading="contentInstallLoading"
        :default-tab="contentInstallDefaultTab"
        :preferred-loader="contentInstallPreferredLoader"
        :preferred-game-version="contentInstallPreferredGameVersion"
        :release-game-versions="contentInstallReleaseGameVersions"
        :project-info="contentInstallProjectInfo"
        :symlink-target="contentInstallSymlinkTarget"
        @install="handleInstallToInstance"
        @create-and-install="handleCreateAndInstall"
        @navigate="handleContentInstallNavigate"
        @cancel="handleContentInstallCancel"
    />
    <ContentInstallPreviewModal ref="contentInstallPreviewModal" />
    <ResourceOperationPreviewModal ref="resourceInstallPreview" />
    <ModpackAlreadyInstalledModal
        ref="modpackAlreadyInstalledModal"
        @create-anyway="handleModpackDuplicateCreateAnyway"
        @go-to-instance="handleModpackDuplicateGoToInstance"
    />
    <AddServerToInstanceModal
        ref="addServerToInstanceModal"
        :symlink-target="addServerSymlinkTarget"
    />
    <ContentUpdaterModal
        ref="incompatibilityWarningModal"
        :key="incompatWarningKey"
        mode="incompatibility-warning"
        :versions="contentInstallIncompatibilityWarningVersions"
        :current-game-version="contentInstallIncompatibilityWarningCurrentGameVersion"
        :current-loader="contentInstallIncompatibilityWarningCurrentLoader"
        current-version-id=""
        :is-app="true"
        :project-type="contentInstallIncompatibilityWarningProjectType"
        :project-icon-url="contentInstallIncompatibilityWarningProjectIconUrl"
        :project-name="contentInstallIncompatibilityWarningProjectName"
        :warning="contentInstallIncompatibilityWarningMessage"
        :action-loading="contentInstallIncompatibilityWarningInstalling"
        @update="handleIncompatibilityWarningUpdate"
        @cancel="handleIncompatibilityWarningCancel"
        @search-compat="handleDropInstallSearchCompat"
    />
    <ModpackInstallModal
        ref="contentInstallModpackInstallModal"
        @install="handleContentInstallModpackInstall"
        @cancel="handleContentInstallModpackInstallCancel"
    />
    <CurseForgeManualDownloadsModal
        ref="contentInstallCurseForgeManualDownloadsModal"
        @view-instance="handleContentInstallModpackDuplicateGoToInstance"
        @imported="handleContentInstallCurseForgeManualDownloadsImported"
    />

    <!-- Global drop overlay -->
    <div
        v-if="isDragging && !onSkinsPage && !onSettingsPage"
        class="fixed inset-0 z-[9999] bg-black/40 flex items-center justify-center pointer-events-none"
    >
        <div
            class="drop-overlay-card rounded-2xl border-2 border-dashed border-brand p-8 text-center"
        >
            <p class="text-lg text-[var(--color-text-primary)]">
                {{ formatMessage(messages.dropOverlayTitle) }}
            </p>
            <p class="text-sm text-[var(--color-text-tertiary)] mt-2">
                {{ formatMessage(messages.dropOverlaySubtitle) }}
            </p>
        </div>
    </div>

    <!-- Processing overlay -->
    <div
        v-if="
            (isProcessing || scanningInstances) &&
            !isDragging &&
            !onSkinsPage &&
            !onSettingsPage &&
            !batchActive
        "
        class="fixed inset-0 z-[9999] bg-black/20 flex items-center justify-center"
    >
        <div class="flex flex-col items-center gap-3">
            <SpinnerIcon class="h-10 w-10 animate-spin text-[var(--color-text-primary)]" />
            <span v-if="scanningInstances" class="text-sm text-[var(--color-text-tertiary)]"
                >{{ formatMessage(messages.dropScanning) }}…</span
            >
        </div>
    </div>

    <ImportServerModal ref="serverImportModal" />

    <!-- Drop type confirmation modal -->
    <ConfirmDropTypeModal
        ref="confirmDropModal"
        :key="batchGroupKey"
        :classification="dropClassification"
        :file-name="dropFileName"
        @confirm="handleConfirmDropConfirm"
        @cancel="handleConfirmDropCancel"
        @help="handleConfirmDropHelp"
    />

    <!-- Generic content install modal (instance selection when not in an instance page) -->
    <GenericContentInstallModal
        ref="genericInstallModal"
        @install="handleGenericInstall"
        @cancel="handleGenericInstallCancel"
        @navigate-create="handleGenericInstallNavigateCreate"
    />

    <!-- Data pack world selection modal -->
    <InstanceExportModal
        ref="dataPackWorldModal"
        :show-save-as="false"
        @select="handleBatchOrDatapackWorldSelect"
        @after-hide="handleBatchWorldAfterHide"
    />

    <!-- Launcher import instance selection modal -->
    <LauncherImportModal
        ref="launcherImportModal"
        @confirm="onImportSelected"
        @cancel="onLauncherImportCancelled"
    />

    <!-- Symlink method selection modal -->
    <SymlinkMethodCards
        ref="symlinkCardsModal"
        @confirm="onSymlinkMethodConfirmed"
        @cancel="onSymlinkMethodCancelled"
    />

    <NewModal ref="compatibleModeConfirmModal" max-width="560px" :closable="true">
        <template #title>
            <span class="text-[var(--color-text-primary)]">{{
                formatMessage(messages.dropCompatibleModeTitle)
            }}</span>
        </template>
        <div class="flex flex-col gap-4">
            <span class="text-[var(--color-text-tertiary)] text-sm">{{
                formatMessage(messages.dropCompatibleModeDesc)
            }}</span>
            <div class="grid grid-cols-2 gap-3">
                <BigOptionButton
                    :icon="FolderOpenIcon"
                    :title="formatMessage(messages.dropCompatibleModeImport)"
                    :description="formatMessage(messages.dropCompatibleModeImport)"
                    no-icon-border
                    @click="handleCompatibleModeConfirm('compatible')"
                />
                <BigOptionButton
                    :icon="RotateCounterClockwiseIcon"
                    :title="formatMessage(messages.dropCompatibleModeOldWay)"
                    :description="formatMessage(messages.dropCompatibleModeOldWay)"
                    no-icon-border
                    @click="handleCompatibleModeConfirm('old-way')"
                />
            </div>
        </div>
        <template #actions>
            <div class="flex w-full items-center justify-end">
                <Button
                    class="flex items-center gap-2"
                    @click="handleCompatibleModeConfirm('cancel')"
                    >{{ formatMessage(messages.dropCompatibleModeCancel) }}
                </Button>
            </div>
        </template>
    </NewModal>

    <!-- Batch drag-and-drop import UI -->
    <BatchScanOverlay
        v-if="batchPhase === 'scanning'"
        :items="batchItems"
        :done="batchScanDone"
        :total="batchOriginalCount"
        @cancel="cancelBatchScan"
    />

    <OnboardingOverlay
        :visible="showOnboarding"
        :mode="onboardingMode"
        @complete="finishOnboarding"
        @skip="skipOnboarding"
        @request-close-settings="closeOnboardingSettings"
    />
</template>

<style lang="scss" scoped>
@property --right-bar-width {
    syntax: '<length>';
    inherits: true;
    initial-value: 0px;
}

.drop-overlay-card {
    background-color: color-mix(
        in srgb,
        var(--surface-2) var(--opacity-ratio-keep-90),
        transparent
    );
}

.app-grid-layout,
.app-contents {
    --top-bar-height: 3rem;
    --left-bar-width: 4rem;
    --right-bar-width: 300px;
}

.app-contents.studio-mode {
    grid-template-columns: 1fr 0;

    .app-sidebar {
        display: none;
    }
}

.app-contents.studio-mode {
    top: var(--top-bar-height);
    height: calc(100vh - var(--top-bar-height));
}

.app-grid-layout {
    display: grid;
    grid-template: 'status status' 'nav dummy';
    grid-template-columns: auto 1fr;
    grid-template-rows: auto 1fr;
    position: relative;
    //z-index: 0;
    background-color: var(--surface-3);
    height: 100vh;
}

.quick-instance-scroll {
    -ms-overflow-style: none;
    scrollbar-width: none;

    &::-webkit-scrollbar {
        display: none;
    }
}

.launcher-background {
    position: fixed;
    inset: -3rem;
    z-index: 0;
    pointer-events: none;
    // Opaque floor under the custom image: lowering "background visibility"
    // dims the image against the app surface instead of revealing the desktop.
    // It must read the *opaque* snapshot -- the translucent `--surface-3`
    // is what the components above use to show the image through, and using it
    // here as well would let the desktop through along with the image.
    background-color: var(--surface-3-opaque);
}

.launcher-background-image {
    position: absolute;
    inset: 0;
    pointer-events: none;
    background-position: center;
    background-size: cover;
    background-repeat: no-repeat;
    transition:
        filter 180ms ease,
        opacity 180ms ease;
}

.app-grid-layout.has-custom-background,
.app-grid-layout.has-transparent-background {
    background-color: transparent;
}

.app-grid-layout.has-custom-background,
.app-grid-layout.has-transparent-background:not(.has-native-decorations) {
    &:not(.is-maximized) {
        border-radius: 8px;
        clip-path: inset(0 round 8px);
        overflow: hidden;
    }
}

.app-grid-layout.has-custom-background {
    .app-grid-navbar,
    .app-grid-statusbar {
        // `--surface-3` carries the body-overridden "Component opacity" alpha in
        // this mode (see `global.scss`), so this only paints the chrome.
        background-color: var(--surface-3) !important;

        backdrop-filter: none;
        -webkit-backdrop-filter: none;
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
    padding-right: var(--window-controls-width, 0px);
    position: relative;
    z-index: 200;
}

[data-tauri-drag-region-exclude] {
    -webkit-app-region: no-drag;
}

.app-contents {
    position: absolute;
    z-index: 1;
    left: var(--left-bar-width);
    top: var(--top-bar-height);
    right: 0;
    bottom: 0;
    height: calc(100vh - var(--top-bar-height));
    background-color: var(--color-bg);
    border-top-left-radius: var(--radius-xl);
    overflow: hidden;
    // Keep sticky/fixed chrome and the page-layer stack from spilling or
    // re-anchoring while slide/opacity transitions repaint.
    isolation: isolate;
    --right-bar-width: 0px;

    display: grid;
    grid-template-columns: 1fr var(--right-bar-width);
    // 显式行高：让 .app-viewport 的 height: 100% 有确定参照（隐式 auto 行会使百分比高度失效）
    grid-template-rows: 1fr;

    &.sidebar-enabled {
        --right-bar-width: 300px;
    }

    &.has-custom-background,
    &.has-transparent-background {
        border-top-left-radius: 0;

        &::before {
            border: none;
            box-shadow: none;
        }
    }

    &.has-custom-background {
        // The page already reads `--color-bg` from the base rule above; neither
        // opacity setting applies to it (see `global.scss`).
        .loading-indicator-container {
            border-top-left-radius: 0;
        }
    }
}

@media (prefers-reduced-motion: no-preference) {
    .app-contents {
        transition: --right-bar-width 320ms cubic-bezier(0.22, 1, 0.36, 1);
    }
}

.app-grid-layout.has-transparent-background {
    .app-grid-navbar,
    .app-grid-statusbar {
        // Window chrome sits above the page: the chrome rung of the opacity
        // model, which follows `--opacity-baseline`.
        background-color: color-mix(
            in srgb,
            var(--surface-3-opaque) var(--opacity-ratio-chrome),
            transparent
        ) !important;

        backdrop-filter: none;
        -webkit-backdrop-filter: none;
    }
}

.app-contents.has-transparent-background {
    // Window chrome sits above the page: the chrome rung of the opacity model,
    // which follows `--opacity-baseline`.
    background-color: color-mix(
        in srgb,
        var(--surface-3-opaque) var(--opacity-ratio-chrome),
        transparent
    );

    &::before {
        position: absolute;
        inset: 0;
        z-index: -10;
        border: none;
        box-shadow: none;
        border-top-left-radius: var(--radius-xl);
        // The panel behind the chrome keeps 82% of the baseline: a `calc()` in
        // this slot would invalidate the whole declaration, so it goes through a
        // rung of the opacity model.
        background-color: color-mix(
            in srgb,
            var(--surface-1-opaque) var(--opacity-ratio-keep-82),
            transparent
        );
    }

    :deep(.browse-install-header) {
        // 68% of the baseline, so the header follows the window opacity setting
        // instead of a fixed alpha. A `calc()` inline in this slot would
        // invalidate the mix, hence the rung.
        background-color: color-mix(
            in srgb,
            var(--surface-1-opaque) var(--opacity-ratio-keep-68),
            transparent
        ) !important;

        backdrop-filter: blur(20px) saturate(115%);
        -webkit-backdrop-filter: blur(20px) saturate(115%);
    }
}

.loading-indicator-container {
    border-top-left-radius: var(--radius-xl);
    overflow: hidden;
}

.app-sidebar {
    overflow: visible;
    width: 300px;
    position: relative;
    z-index: 11;
    height: calc(100vh - var(--top-bar-height));
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

    // 只让外侧(左边)圆润,右边与 Sidebar 完全贴合,单一元素完成形状
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

    // Programmatic focus target for route handoff and keyboard shortcuts.
    // Clicking the pane focuses it; Tab/Shift/Esc then paint the UA ring as a
    // full-pane black frame (issue #579).
    &:focus,
    &:focus-visible {
        outline: none;
    }
}

.app-contents::before {
    z-index: 30;
    content: '';
    // Absolute (not fixed) so the inset edge shadow stays glued to the content
    // pane. Fixed coordinates recompute against the viewport and can desync
    // from the nav/content boundary during page-layer compositing.
    position: absolute;
    inset: 0;
    border-top-left-radius: var(--radius-xl);
    box-shadow: 1px 1px 15px rgba(0, 0, 0, 0.1) inset;
    border-color: var(--surface-5);
    border-width: 1px;
    border-style: solid;
    pointer-events: none;
}

// Console fullscreen teleports above the content pane; the decorative edge
// must not draw through that opaque overlay.
body.modrinth-console-fullscreen-active .app-contents::before {
    opacity: 0;
}

// The body class is the console's authoritative fullscreen state. Keep the
// shell layout correct even if its companion event is delayed or missed.
body.modrinth-console-fullscreen-active .app-contents {
    --right-bar-width: 0px;
}

body.modrinth-console-fullscreen-active .app-sidebar {
    display: none;
}

.sidebar-teleport-content:empty + .sidebar-default-content.sidebar-enabled {
    display: contents;
}

@media (prefers-reduced-motion: no-preference) {
    .nav-button-animated-enter-active {
        transition: all 0.5s cubic-bezier(0.15, 1.4, 0.64, 0.96);
    }

    .nav-button-animated-leave-active {
        transition: all 0.25s ease;
    }

    .nav-button-animated-enter-active {
        position: relative;
    }

    .nav-button-animated-enter-active::before {
        content: '';
        inset: 0;
        border-radius: 100vw;
        background-color: var(--color-brand-highlight);
        position: absolute;
        animation: pop 0.5s ease-in forwards;
        opacity: 0;
    }

    @keyframes pop {
        0% {
            scale: 0.5;
        }
        50% {
            opacity: 0.5;
        }
        100% {
            scale: 1.5;
        }
    }

    .nav-button-animated-enter-from {
        scale: 0.5;
        translate: -2rem 0;
        opacity: 0;
    }

    .nav-button-animated-leave-to {
        scale: 0.75;
        opacity: 0;
    }

    .fade-enter-active {
        transition: 0.25s ease-in-out;
    }

    .fade-enter-from {
        opacity: 0;
    }
}
</style>
<style>
.os-theme-dark,
.os-theme-light {
    --os-handle-bg: var(--color-scrollbar) !important;
    --os-handle-bg-hover: var(--color-scrollbar) !important;
    --os-handle-bg-active: var(--color-scrollbar) !important;
}

.mac {
    .app-grid-statusbar {
        padding-left: 5rem;
    }
}

.windows {
    .info-card {
        right: 22rem;
    }

    .profile-card {
        right: 8rem;
    }
}
</style>
