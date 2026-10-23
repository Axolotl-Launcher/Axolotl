<script setup lang="ts">
import CheckIcon from '@modrinth/assets/icons/check.svg?component'
import GridIcon from '@modrinth/assets/icons/grid.svg?component'
import LayoutTemplateIcon from '@modrinth/assets/icons/layout-template.svg?component'
import MinimizeIcon from '@modrinth/assets/icons/minimize.svg?component'
import MoveIcon from '@modrinth/assets/icons/move.svg?component'
import PencilIcon from '@modrinth/assets/icons/pencil.svg?component'
import PlusIcon from '@modrinth/assets/icons/plus.svg?component'
import RotateCounterClockwiseIcon from '@modrinth/assets/icons/rotate-counter-clockwise.svg?component'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, onUnmounted, provide, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { usePreviewShell } from '../../lib/shell'
import { usePreview } from '../../lib/state'
import type { GameInstance } from '../../lib/types'
import HomeDailyChallenge from './components/HomeDailyChallenge.vue'
import HomeDashboard from './components/HomeDashboard.vue'
import HomeGreetingAccountModal from './components/HomeGreetingAccountModal.vue'
import HomeInstancePickerModal from './components/HomeInstancePickerModal.vue'
import HomeMinecraftNews from './components/HomeMinecraftNews.vue'
import HomeMinimal from './components/HomeMinimal.vue'
import HomePlayInsights from './components/HomePlayInsights.vue'
import HomeSystemDialog from './components/HomeSystemDialog.vue'
import {
    createDefaultHomeDashboard,
    getHomeGreetingOverride,
    HOME_GREETING_GLOBAL_KEY,
    type HomeDashboardConfig,
    pruneHomeGreetingOverrides,
    setHomeGreetingOverride,
} from './home-dashboard'
import { HOME_GREETING_EDITOR_KEY, type HomeGreetingEditorContext } from './home-greeting-editor'
import { homeMessages } from './messages'
import { provideHomeRuntime } from './runtime'
import { useHomeState } from './state'

const { formatMessage } = useVIntl()
const route = useRoute()
const router = useRouter()
const preview = usePreview()
const shell = usePreviewShell()
const state = useHomeState()
const runtime = provideHomeRuntime(state)
const messages = homeMessages

preview.breadcrumbs.setRootContext({ name: formatMessage(messages.home), link: route.path })

const instances = computed<GameInstance[]>(() => preview.instances.instances.value)
const playerName = runtime.playerName
const playerAccountId = runtime.playerAccountId
const dashboardConfig = state.dashboard
const dashboardEditing = state.dashboardEditing
const dashboard = ref<InstanceType<typeof HomeDashboard>>()
const instancePicker = ref<InstanceType<typeof HomeInstancePickerModal>>()
const greetingAccountModal = ref<InstanceType<typeof HomeGreetingAccountModal>>()
const greetingEditorOpen = ref(false)
const isMinimal = computed(() => preview.settings.home_layout === 'minimal')
const isFreeWidgetLayout = computed(() => dashboardConfig.value.layout === 'free')
const switchingLayout = ref(false)
const floatingControlsStyle = computed(() => ({
    bottom: preview.settings.feature_flags.page_path ? '3.5rem' : '1rem',
    right: `calc(${shell.sidebarVisible.value ? '300px' : '0px'} + 1rem)`,
}))

const animateSidebarShow = ref(false)
const cancelSidebarTimer = preview.clock.after(200, () => {
    animateSidebarShow.value = true
})
onUnmounted(cancelSidebarTimer)

function updateDashboardConfig(config: HomeDashboardConfig) {
    dashboardConfig.value = config
}

const greetingWidget = computed(
    () => dashboardConfig.value.widgets.find((widget) => widget.kind === 'greeting') ?? null,
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
    if (!widget) return

    updateDashboardConfig(
        setHomeGreetingOverride(config, widget.id, greetingEditorAccountId.value, override),
    )
}

/**
 * A greeting override is bound to an account id, so removing an account has to
 * take its text with it -- otherwise a later account that reuses the id would
 * inherit text it never wrote.
 */
watch(
    () => preview.accounts.accounts.value.map((account) => account.account_id),
    (accountIds) => {
        const config = dashboardConfig.value
        const pruned = pruneHomeGreetingOverrides(config, accountIds)
        if (pruned !== config) updateDashboardConfig(pruned)
    },
    { immediate: true },
)

async function resetDashboardConfig() {
    if (!(await runtime.confirm(formatMessage(messages.widgetsResetConfirm)))) return
    updateDashboardConfig(createDefaultHomeDashboard())
}

function selectMinimalInstance(instance: GameInstance) {
    state.minimalInstanceId.value = instance.id
}

function createInstance() {
    void router.push('/create')
}

function toggleHomeLayout() {
    if (switchingLayout.value) return

    const nextLayout = preview.settings.home_layout === 'minimal' ? 'standard' : 'minimal'
    switchingLayout.value = true
    preview.settings.home_layout = nextLayout
    if (nextLayout === 'minimal') dashboardEditing.value = false
    // `update(settings)` round-trips to the backend before the switch re-enables.
    preview.clock.after(120, () => {
        switchingLayout.value = false
    })
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
</script>

<template>
    <HomeInstancePickerModal
        ref="instancePicker"
        :instances="instances"
        :selected-instance-id="state.minimalInstanceId.value"
        @select="selectMinimalInstance"
    />
    <HomeGreetingAccountModal
        ref="greetingAccountModal"
        :account-name="playerName"
        :can-edit-nickname="playerAccountId !== null"
        @save="saveGreetingOverride"
        @closed="greetingEditorOpen = false"
    />
    <HomeSystemDialog />
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
            :selected-instance-id="state.minimalInstanceId.value"
            @choose="instancePicker?.show()"
            @create="createInstance"
        />
    </div>
    <div class="home-floating-controls" :style="floatingControlsStyle">
        <template v-if="!isMinimal">
            <button
                v-if="dashboardEditing"
                v-tooltip="formatMessage(messages.widgetsAdd)"
                type="button"
                class="home-floating-action"
                :aria-label="formatMessage(messages.widgetsAdd)"
                @click="openWidgetPicker"
            >
                <PlusIcon />
            </button>
            <button
                v-if="dashboardEditing"
                v-tooltip="formatMessage(messages.widgetsReset)"
                type="button"
                class="home-floating-action"
                :aria-label="formatMessage(messages.widgetsReset)"
                @click="resetDashboardConfig"
            >
                <RotateCounterClockwiseIcon />
            </button>
            <button
                v-tooltip="
                    formatMessage(
                        dashboardEditing ? messages.widgetsDone : messages.widgetsCustomize,
                    )
                "
                data-onboarding-id="home-widget-customize"
                type="button"
                class="home-floating-action"
                :class="{ 'is-active': dashboardEditing }"
                :aria-label="
                    formatMessage(
                        dashboardEditing ? messages.widgetsDone : messages.widgetsCustomize,
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
                            ? messages.widgetsLayoutSwitchToGrid
                            : messages.widgetsLayoutSwitchToFree,
                    )
                "
                type="button"
                role="switch"
                class="home-layout-switch home-widget-layout-switch"
                :class="{ 'is-free': isFreeWidgetLayout }"
                :aria-checked="isFreeWidgetLayout"
                :aria-label="formatMessage(messages.widgetsLayoutToggle)"
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
                formatMessage(
                    isMinimal ? messages.layoutSwitchToInformation : messages.layoutSwitchToMinimal,
                )
            "
            data-onboarding-id="home-layout-switch"
            type="button"
            role="switch"
            class="home-layout-switch"
            :class="{ 'is-minimal': isMinimal }"
            :disabled="switchingLayout"
            :aria-checked="isMinimal"
            :aria-label="formatMessage(messages.layoutToggle)"
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
    <Teleport v-if="!isMinimal" defer to="#sidebar-default-teleport-target">
        <div
            class="slide-enter-active flex min-w-0 flex-col"
            :class="{ 'slide-enter-from': !animateSidebarShow }"
        >
            <HomePlayInsights />
            <HomeDailyChallenge />
            <HomeMinecraftNews />
        </div>
    </Teleport>
</template>

<style scoped>
/* The app's global `.slide-enter-*` classes (assets/stylesheets/global.scss). */
.slide-enter-active {
    transition:
        transform 0.2s ease,
        opacity 0.2s ease;
}

.slide-enter-from {
    transform: translateY(30px);
    opacity: 0;
}

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
