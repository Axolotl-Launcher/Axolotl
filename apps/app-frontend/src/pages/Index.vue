<script setup lang="ts">
import {
    CheckIcon,
    GridIcon,
    LayoutTemplateIcon,
    MinimizeIcon,
    MoveIcon,
    PencilIcon,
    PlusIcon,
    RotateCounterClockwiseIcon,
} from '@modrinth/assets'
import {
    defineMessages,
    injectNotificationManager,
    injectPageContext,
    useVIntl,
} from '@modrinth/ui'
import { useQueryClient } from '@tanstack/vue-query'
import { computed, inject, onUnmounted, provide, type Ref, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import {
    createDefaultHomeDashboard,
    createHomeDashboardSaveQueue,
    getHomeGreetingOverride,
    HOME_GREETING_GLOBAL_KEY,
    type HomeDashboardConfig,
    normalizeHomeDashboard,
    pruneHomeGreetingOverrides,
    setHomeGreetingOverride,
} from '@/components/home/home-dashboard'
import {
    HOME_GREETING_EDITOR_KEY,
    type HomeGreetingEditorContext,
} from '@/components/home/home-greeting-editor'
import { getActivePlayerAccount } from '@/components/home/home-utils'
import HomeDailyChallenge from '@/components/home/HomeDailyChallenge.vue'
import HomeDashboard from '@/components/home/HomeDashboard.vue'
import HomeGreetingAccountModal from '@/components/home/HomeGreetingAccountModal.vue'
import HomeInstancePickerModal from '@/components/home/HomeInstancePickerModal.vue'
import HomeMinecraftNews from '@/components/home/HomeMinecraftNews.vue'
import HomeMinimal from '@/components/home/HomeMinimal.vue'
import HomePlayInsights from '@/components/home/HomePlayInsights.vue'
import { useNetworkStatus } from '@/composables/useNetworkStatus'
import { get_default_user, users } from '@/helpers/auth'
import { DIRECT_LINKS_CHANGED_EVENT } from '@/helpers/direct-link-sync'
import { instance_groups_listener, instance_listener } from '@/helpers/events'
import { list } from '@/helpers/instance'
import {
    appSettingsKeys,
    appSettingsQueryOptions,
    update as updateSettings,
} from '@/helpers/settings'
import type { GameInstance } from '@/helpers/types'
import { useBreadcrumbs } from '@/store/breadcrumbs'
import { useTheming } from '@/store/state'
import type { FeatureFlag, HomeLayout } from '@/store/theme'

const { handleError } = injectNotificationManager()
const route = useRoute()
const router = useRouter()
const breadcrumbs = useBreadcrumbs()
const { formatMessage } = useVIntl()
const { offline } = useNetworkStatus()
const themeStore = useTheming()
const queryClient = useQueryClient()
const pageContext = injectPageContext()

const messages = defineMessages({
    home: { id: 'app.home.breadcrumb', defaultMessage: 'Home' },
    switchToMinimal: {
        id: 'app.home.layout.switch-to-minimal',
        defaultMessage: 'Switch to Minimal Home',
    },
    switchToInformation: {
        id: 'app.home.layout.switch-to-information',
        defaultMessage: 'Switch to Information Home',
    },
    homeLayoutToggle: {
        id: 'app.home.layout.toggle',
        defaultMessage: 'Minimal Home',
    },
    switchToGridWidgetLayout: {
        id: 'app.home.widgets.layout.switch-to-grid',
        defaultMessage: 'Switch to grid widget layout',
    },
    switchToFreeWidgetLayout: {
        id: 'app.home.widgets.layout.switch-to-free',
        defaultMessage: 'Switch to free widget layout',
    },
    widgetLayoutToggle: {
        id: 'app.home.widgets.layout.toggle',
        defaultMessage: 'Widget layout mode',
    },
    resetWidgets: {
        id: 'app.home.widgets.reset-confirm',
        defaultMessage: 'Restore the default widget layout?',
    },
    customizeWidgets: {
        id: 'app.home.widgets.customize',
        defaultMessage: 'Customize widgets',
    },
    doneEditing: { id: 'app.home.widgets.done', defaultMessage: 'Finish editing' },
    addWidget: { id: 'app.home.widgets.add', defaultMessage: 'Add widget' },
    resetWidgetLayout: {
        id: 'app.home.widgets.reset',
        defaultMessage: 'Restore default widgets',
    },
})

const recentProjectsInHomeFlag: FeatureFlag = 'worlds_in_home'

breadcrumbs.setRootContext({ name: formatMessage(messages.home), link: route.path })

const instances = ref<GameInstance[]>([])
const playerName = ref<string | null>(null)
const playerAccountId = ref<string | null>(null)
const dashboardConfig = ref<HomeDashboardConfig | null>(null)
const dashboard = ref<InstanceType<typeof HomeDashboard>>()
const dashboardEditing = ref(false)
const instancePicker = ref<InstanceType<typeof HomeInstancePickerModal>>()
const greetingAccountModal = ref<InstanceType<typeof HomeGreetingAccountModal>>()
const greetingEditorOpen = ref(false)
const isMinimal = computed(() => themeStore.homeLayout === 'minimal')
const isFreeWidgetLayout = computed(() => dashboardConfig.value?.layout === 'free')
const switchingLayout = ref(false)
const dashboardSaveQueue = createHomeDashboardSaveQueue(
    async (config) => {
        const settings = await updateSettings({ home_widgets: config })
        queryClient.setQueryData(appSettingsKeys.all, settings)
    },
    (config) => {
        dashboardConfig.value = config
    },
    handleError,
)
const floatingControlsStyle = computed(() => ({
    bottom: themeStore.getFeatureFlag('page_path') ? '3.5rem' : '1rem',
    right: `calc(${pageContext.floatingActionBarOffsets?.right.value ?? '0px'} + 1rem)`,
}))

const animateSidebarShow = ref(false)
const sidebarTimer = setTimeout(() => {
    animateSidebarShow.value = true
}, 200)

async function clearMissingMinimalInstance() {
    const selectedId = themeStore.minimalHomeInstanceId
    if (!selectedId || instances.value.some((instance) => instance.id === selectedId)) return

    themeStore.minimalHomeInstanceId = null
    try {
        const settings = await updateSettings({ minimal_home_instance_id: null })
        queryClient.setQueryData(appSettingsKeys.all, settings)
    } catch (error) {
        handleError(error)
    }
}

let instanceRequest = 0
async function fetchInstances() {
    const request = ++instanceRequest
    try {
        const result = await list()
        if (disposed || request !== instanceRequest) return false
        instances.value = result
        void clearMissingMinimalInstance()
        return true
    } catch (error) {
        handleError(error)
        return false
    }
}

async function fetchPlayerName() {
    const [selectedUser, accounts] = await Promise.all([
        get_default_user(offline.value).catch(() => undefined),
        users(offline.value).catch(() => []),
    ])
    if (disposed) return
    const account = getActivePlayerAccount(selectedUser, accounts)

    playerName.value = account?.name ?? null
    playerAccountId.value = account?.id ?? null
    pruneGreetingOverrides(accounts)
}

/**
 * A greeting override is bound to an account id, so removing an account has to
 * take its text with it -- otherwise a later account that reuses the id would
 * inherit text it never wrote.
 */
function pruneGreetingOverrides(accounts: readonly { account_id?: string }[]) {
    const config = dashboardConfig.value
    if (!config) return

    const accountIds = accounts
        .map((account) => account.account_id)
        .filter((accountId): accountId is string => !!accountId)
    const pruned = pruneHomeGreetingOverrides(config, accountIds)
    if (pruned !== config) updateDashboardConfig(pruned)
}

async function loadDashboardConfig() {
    try {
        const settings = await queryClient.fetchQuery({
            ...appSettingsQueryOptions(),
            staleTime: 30_000,
        })
        const normalized = normalizeHomeDashboard(settings.home_widgets)
        if (normalized) {
            dashboardConfig.value = normalized
            return
        }

        const config = createDefaultHomeDashboard(
            themeStore.getFeatureFlag(recentProjectsInHomeFlag),
        )
        dashboardConfig.value = config
        void dashboardSaveQueue.enqueue(config, config)
    } catch (error) {
        dashboardConfig.value = createDefaultHomeDashboard(
            themeStore.getFeatureFlag(recentProjectsInHomeFlag),
        )
        handleError(error)
    }
}

function updateDashboardConfig(config: HomeDashboardConfig) {
    const previous = dashboardConfig.value ?? config
    dashboardConfig.value = config
    void dashboardSaveQueue.enqueue(config, previous)
}

const greetingWidget = computed(
    () => dashboardConfig.value?.widgets.find((widget) => widget.kind === 'greeting') ?? null,
)
/** Text is bound to the active account, or to the global key before anyone signs in. */
const greetingEditorAccountId = computed(() => playerAccountId.value ?? HOME_GREETING_GLOBAL_KEY)
const greetingOverride = computed(() =>
    getHomeGreetingOverride(greetingWidget.value?.options, greetingEditorAccountId.value),
)

provide<HomeGreetingEditorContext>(HOME_GREETING_EDITOR_KEY, {
    accountId: greetingEditorAccountId,
    accountName: computed(() => playerName.value),
    override: greetingOverride,
    canEdit: computed(() => greetingWidget.value !== null),
    canEditNickname: computed(() => playerAccountId.value !== null),
    editing: greetingEditorOpen,
    open: () => {
        greetingEditorOpen.value = true
        greetingAccountModal.value?.show(greetingOverride.value)
    },
})

function saveGreetingOverride(override: { nickname: string; title: string }) {
    const config = dashboardConfig.value
    const widget = greetingWidget.value
    if (!config || !widget) return

    updateDashboardConfig(
        setHomeGreetingOverride(config, widget.id, greetingEditorAccountId.value, override),
    )
}

/**
 * The greeting name is read once per page, so it has to follow the account the
 * sidebar switches to. AccountsCard owns that state and bumps a revision when
 * it changes; this is the same subscription the skins page uses.
 */
const accountsCard = inject<Ref<{ accountChangeRevision?: number } | null> | null>(
    'accountsCard',
    null,
)
const accountChangeRevision = computed(() => accountsCard?.value?.accountChangeRevision)

watch(accountChangeRevision, (revision, previousRevision) => {
    if (revision === undefined || previousRevision === undefined) return
    void fetchPlayerName()
})

function resetDashboardConfig() {
    if (!window.confirm(formatMessage(messages.resetWidgets))) return
    updateDashboardConfig(createDefaultHomeDashboard())
}

async function selectMinimalInstance(instance: GameInstance) {
    try {
        await updateSettings({ minimal_home_instance_id: instance.id })
        themeStore.minimalHomeInstanceId = instance.id
    } catch (error) {
        handleError(error)
    }
}

function createInstance() {
    void router.push('/create')
}

async function toggleHomeLayout() {
    if (switchingLayout.value) return

    const previousLayout = themeStore.homeLayout
    const nextLayout: HomeLayout = previousLayout === 'minimal' ? 'standard' : 'minimal'
    const previousEditing = dashboardEditing.value
    switchingLayout.value = true
    themeStore.homeLayout = nextLayout
    if (nextLayout === 'minimal') dashboardEditing.value = false

    try {
        await updateSettings({ home_layout: nextLayout })
    } catch (error) {
        themeStore.homeLayout = previousLayout
        dashboardEditing.value = previousEditing
        handleError(error)
    } finally {
        switchingLayout.value = false
    }
}

function toggleDashboardEditing() {
    dashboardEditing.value = !dashboardEditing.value
}

function toggleWidgetLayout() {
    dashboard.value?.setLayout(isFreeWidgetLayout.value ? 'grid' : 'free')
}

function openWidgetPicker() {
    dashboard.value?.openWidgetPicker()
}

let disposed = false
const unlisteners: Array<() => void> = []
function keepListener(request: Promise<() => void>) {
    void request
        .then((unlisten) => {
            if (disposed) unlisten()
            else unlisteners.push(unlisten)
        })
        .catch(handleError)
}
window.addEventListener(DIRECT_LINKS_CHANGED_EVENT, fetchInstances)
keepListener(
    instance_listener(() => {
        if (!disposed) void fetchInstances()
    }),
)
keepListener(
    instance_groups_listener(() => {
        if (!disposed) void fetchInstances()
    }),
)
onUnmounted(() => {
    disposed = true
    clearTimeout(sidebarTimer)
    unlisteners.forEach((unlisten) => unlisten())
    window.removeEventListener(DIRECT_LINKS_CHANGED_EVENT, fetchInstances)
})

const [instancesLoaded] = await Promise.all([fetchInstances(), loadDashboardConfig()])
if (!instancesLoaded || instances.value.length > 0) void fetchPlayerName()
</script>

<template>
    <HomeInstancePickerModal
        ref="instancePicker"
        :instances="instances"
        :selected-instance-id="themeStore.minimalHomeInstanceId"
        @select="selectMinimalInstance"
    />
    <HomeGreetingAccountModal
        ref="greetingAccountModal"
        :account-name="playerName"
        :can-edit-nickname="playerAccountId !== null"
        @save="saveGreetingOverride"
        @closed="greetingEditorOpen = false"
    />
    <div class="min-h-full">
        <HomeDashboard
            v-if="!isMinimal && dashboardConfig"
            ref="dashboard"
            :config="dashboardConfig"
            :instances="instances"
            :player-name="playerName"
            :editing="dashboardEditing"
            @change="updateDashboardConfig"
        />

        <HomeMinimal
            v-else
            :instances="instances"
            :player-name="playerName"
            :selected-instance-id="themeStore.minimalHomeInstanceId"
            @choose="instancePicker?.show()"
            @create="createInstance"
        />
    </div>
    <div class="home-floating-controls" :style="floatingControlsStyle">
        <template v-if="!isMinimal">
            <button
                v-if="dashboardEditing"
                v-tooltip="formatMessage(messages.addWidget)"
                type="button"
                class="home-floating-action"
                :aria-label="formatMessage(messages.addWidget)"
                @click="openWidgetPicker"
            >
                <PlusIcon />
            </button>
            <button
                v-if="dashboardEditing"
                v-tooltip="formatMessage(messages.resetWidgetLayout)"
                type="button"
                class="home-floating-action"
                :aria-label="formatMessage(messages.resetWidgetLayout)"
                @click="resetDashboardConfig"
            >
                <RotateCounterClockwiseIcon />
            </button>
            <button
                v-tooltip="
                    formatMessage(
                        dashboardEditing ? messages.doneEditing : messages.customizeWidgets,
                    )
                "
                data-onboarding-id="home-widget-customize"
                type="button"
                class="home-floating-action"
                :class="{ 'is-active': dashboardEditing }"
                :aria-label="
                    formatMessage(
                        dashboardEditing ? messages.doneEditing : messages.customizeWidgets,
                    )
                "
                :aria-pressed="dashboardEditing"
                @click="toggleDashboardEditing"
            >
                <CheckIcon v-if="dashboardEditing" />
                <PencilIcon v-else />
            </button>
            <button
                v-if="dashboardEditing"
                v-tooltip="
                    formatMessage(
                        isFreeWidgetLayout
                            ? messages.switchToGridWidgetLayout
                            : messages.switchToFreeWidgetLayout,
                    )
                "
                type="button"
                role="switch"
                class="home-layout-switch home-widget-layout-switch"
                :class="{ 'is-free': isFreeWidgetLayout }"
                :aria-checked="isFreeWidgetLayout"
                :aria-label="formatMessage(messages.widgetLayoutToggle)"
                @click="toggleWidgetLayout"
            >
                <span class="home-layout-switch-option home-widget-layout-grid" aria-hidden="true">
                    <GridIcon />
                </span>
                <span class="home-layout-switch-thumb" aria-hidden="true" />
                <span class="home-layout-switch-option home-widget-layout-free" aria-hidden="true">
                    <MoveIcon />
                </span>
            </button>
            <span class="home-floating-divider" aria-hidden="true" />
        </template>
        <button
            v-tooltip="
                formatMessage(isMinimal ? messages.switchToInformation : messages.switchToMinimal)
            "
            data-onboarding-id="home-layout-switch"
            type="button"
            role="switch"
            class="home-layout-switch"
            :class="{ 'is-minimal': isMinimal }"
            :disabled="switchingLayout"
            :aria-checked="isMinimal"
            :aria-label="formatMessage(messages.homeLayoutToggle)"
            @click="toggleHomeLayout"
        >
            <span
                class="home-layout-switch-option home-layout-switch-information"
                aria-hidden="true"
            >
                <LayoutTemplateIcon />
            </span>
            <span class="home-layout-switch-thumb" aria-hidden="true" />
            <span class="home-layout-switch-option home-layout-switch-minimal" aria-hidden="true">
                <MinimizeIcon />
            </span>
        </button>
    </div>
    <Teleport v-if="!isMinimal" to="#sidebar-default-teleport-target">
        <div
            class="flex min-w-0 flex-col slide-enter-active"
            :class="{ 'slide-enter-from': !animateSidebarShow }"
        >
            <HomePlayInsights />
            <HomeDailyChallenge />
            <HomeMinecraftNews />
        </div>
    </Teleport>
</template>

<style scoped>
.home-floating-controls {
    position: fixed;
    z-index: 40;
    display: flex;
    height: 2.5rem;
    align-items: center;
    gap: 0.125rem;
    padding: 0.25rem;
    box-sizing: border-box;
    border: 1px solid var(--color-divider);
    border-radius: 9999px;
    background: var(--surface-3);
    box-shadow:
        var(--shadow-button),
        0 0.25rem 0.75rem rgb(0 0 0 / 20%);
    isolation: isolate;
}

.home-floating-action {
    display: flex;
    width: 2rem;
    height: 2rem;
    align-items: center;
    justify-content: center;
    padding: 0;
    border: 0;
    border-radius: 9999px;
    background: transparent;
    color: var(--color-text-tertiary);
    cursor: pointer;
    transition:
        background-color 120ms ease,
        color 120ms ease,
        filter 150ms ease,
        transform 150ms ease;
}

.home-floating-action:hover {
    background: var(--surface-4);
    color: var(--color-text-primary);
}

.home-floating-action:active {
    transform: scale(0.96);
}

.home-floating-action:focus-visible,
.home-layout-switch:focus-visible {
    outline: none;
    box-shadow: 0 0 0 4px var(--color-brand-shadow);
}

.home-floating-action.is-active {
    background: var(--color-brand);
    color: var(--color-accent-contrast);
}

.home-floating-action :deep(svg) {
    width: 1rem;
    height: 1rem;
}

.home-floating-divider {
    width: 1px;
    height: 1.25rem;
    margin: 0 0.125rem;
    background: var(--color-divider);
}

.home-layout-switch {
    position: relative;
    display: grid;
    grid-template-columns: repeat(2, 2rem);
    align-items: center;
    /* width: 4.25rem; */ /* closes #210 */
    height: 2rem;
    margin: 0;
    padding: 0;
    border: 0;
    border-radius: 9999px;
    background: var(--surface-4);
    cursor: pointer;
    isolation: isolate;
    transition:
        filter 150ms ease,
        transform 150ms ease;
}

.home-layout-switch:hover:not(:disabled) {
    filter: brightness(var(--hover-brightness));
}

.home-layout-switch:active:not(:disabled) {
    transform: scale(0.97);
}

.home-layout-switch:disabled {
    cursor: not-allowed;
    opacity: 0.6;
}

.home-layout-switch-thumb {
    position: absolute;
    top: 0.125rem;
    left: 0.125rem;
    z-index: 0;
    width: 1.75rem;
    height: 1.75rem;
    border-radius: 9999px;
    background: var(--color-brand);
    transition: transform 180ms ease;
}

.home-layout-switch.is-minimal .home-layout-switch-thumb {
    transform: translateX(2rem);
}

.home-widget-layout-switch.is-free .home-layout-switch-thumb {
    transform: translateX(2rem);
}

.home-layout-switch-option {
    position: relative;
    z-index: 1;
    display: flex;
    width: 2rem;
    height: 1.75rem;
    align-items: center;
    justify-content: center;
    color: var(--color-text-tertiary);
    transition: color 180ms ease;
}

.home-layout-switch-option :deep(svg) {
    width: 1rem;
    height: 1rem;
}

.home-layout-switch:not(.is-minimal) .home-layout-switch-information,
.home-layout-switch.is-minimal .home-layout-switch-minimal,
.home-widget-layout-switch:not(.is-free) .home-widget-layout-grid,
.home-widget-layout-switch.is-free .home-widget-layout-free {
    color: var(--color-accent-contrast);
}
</style>
