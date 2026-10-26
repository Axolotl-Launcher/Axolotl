<template>
    <!--
        Fullscreen must teleport to body: .app-contents uses isolation:isolate
        and z-index:1, so an in-tree fixed overlay cannot paint above its
        border/vignette pseudo-elements.
    -->
    <Teleport to="body" :disabled="!isFullscreen">
        <div
            class="flex min-h-0 flex-1 flex-col gap-4"
            :class="
                isFullscreen
                    ? // Below statusbar (200) so the title-bar drag strip stays live.
                      `fixed inset-0 z-[150] bg-surface-1 p-6 py-8 ${isApp ? 'pt-12' : ''}`
                    : ''
            "
        >
            <div
                v-if="isFullscreen && isApp"
                data-tauri-drag-region
                class="pointer-events-auto absolute inset-x-0 top-0 h-12"
                aria-hidden="true"
            />
            <div
                v-if="
                    (ctx.localCrashAnalysis?.value?.findings.length ||
                        ctx.localCrashAnalysis?.value?.mod_changes.length) &&
                    !isFullscreen
                "
                class="flex max-h-[50%] shrink-0 flex-col gap-2 overflow-y-auto"
            >
                <CollapsibleAdmonition
                    class="shrink-0"
                    type="critical"
                    :header="localCrashHeader"
                    :items="localCrashItems"
                />
                <div class="flex shrink-0 justify-end">
                    <Button
                        type="outlined"
                        :disabled="exportingCrashContext"
                        @click="handleExportCrashContext"
                        ><DownloadIcon />
                        {{ formatMessage(consoleMessages.exportCrashContext) }}
                    </Button>
                </div>
            </div>
            <CollapsibleAdmonition
                v-if="ctx.crashAnalysis?.value && !isFullscreen"
                type="critical"
                :header="crashHeader"
                :items="crashItems"
                dismissible
                @dismiss="ctx.onDismissCrash?.()"
            />

            <div class="flex items-center gap-2">
                <StyledInput
                    v-model="searchQuery"
                    :icon="SearchIcon"
                    :placeholder="formatMessage(consoleMessages.searchLogs)"
                    wrapper-class="flex-1"
                    input-class="!h-10"
                    clearable
                />
                <div v-if="ctx.logSources?.value && ctx.activeLogSourceIndex" class="w-[220px]">
                    <Combobox
                        :model-value="ctx.activeLogSourceIndex.value"
                        :options="logSourceOptions"
                        @update:model-value="(v) => (ctx.activeLogSourceIndex!.value = v)"
                    />
                </div>
            </div>

            <div class="flex items-center gap-2">
                <ConsoleFilterPills v-model="activeFilters" @toggle="handleFilterToggle" />
                <div class="ml-auto flex items-center gap-2">
                    <Button
                        :type="wrapLines ? 'chip' : 'quiet'"
                        :color="wrapLines ? 'brand' : undefined"
                        :aria-pressed="wrapLines"
                        :title="formatMessage(consoleMessages.toggleWrap)"
                        @click="wrapLines = !wrapLines"
                    >
                        <WrapTextIcon />
                        {{ formatMessage(consoleMessages.wrapLabel) }}
                    </Button>
                    <div class="w-28">
                        <Combobox
                            :model-value="logFontSize"
                            :options="fontSizeOptions"
                            @update:model-value="(v) => (logFontSize = v)"
                        />
                    </div>
                    <ConsoleActionButtons
                        :show-clear="isLiveSource"
                        :has-logs="hasLogs"
                        :share-disabled="resolvedShareDisabled"
                        :sharing="isSharing"
                        :fullscreen="isFullscreen"
                        :clear-disabled="resolvedClearDisabled"
                        :clear-disabled-tooltip="resolvedClearDisabledTooltip"
                        :show-delete="showDelete"
                        :delete-disabled="resolvedDeleteDisabled"
                        :delete-disabled-tooltip="resolvedDeleteDisabledTooltip"
                        @clear="handleClear"
                        @share="handleShare"
                        @toggle-fullscreen="toggleFullscreen"
                        @delete="handleDelete"
                    />
                </div>
            </div>

            <div class="relative min-h-0 flex-1 overflow-hidden rounded-[var(--radius-xl)]">
                <LogViewport
                    ref="viewportRef"
                    class="h-full"
                    :lines="filteredLines"
                    :search-query="searchQuery"
                    :wrap="wrapLines"
                    :font-size="logFontSize"
                    :empty-state-type="ctx.emptyStateType"
                />
                <Transition name="terminal-loading-fade">
                    <div
                        v-if="resolvedLoading"
                        class="console-loading-scrim pointer-events-none absolute inset-0 z-20 flex items-center justify-center px-8"
                        aria-hidden="true"
                    >
                        <LoadingIndicator />
                    </div>
                </Transition>
            </div>

            <StyledInput
                v-if="showCommandInput"
                v-model="commandInput"
                v-tooltip="commandDisabled ? commandDisabledTooltip : undefined"
                :icon="TerminalSquareIcon"
                :placeholder="commandPlaceholder"
                :disabled="commandDisabled"
                wrapper-class="w-full"
                input-class="!h-9"
                autocomplete="off"
                :spellcheck="false"
                @keydown.enter="submitCommand"
            />
        </div>
    </Teleport>
    <ShareModal
        ref="shareModal"
        :header="formatMessage(consoleMessages.shareLogs)"
        link
        :social-buttons="false"
    />
    <NewModal
        ref="deleteModal"
        :header="formatMessage(consoleMessages.deleteLogFile)"
        :fade="'danger'"
        max-width="500px"
    >
        <div class="flex flex-col gap-6">
            <Admonition type="critical" :header="formatMessage(consoleMessages.deleteIrreversible)">
                {{ formatMessage(consoleMessages.deleteConfirmation) }}
            </Admonition>
        </div>
        <template #actions>
            <div class="flex justify-end gap-2">
                <Button type="outlined" @click="deleteModal?.hide()"
                    ><XIcon />
                    {{ formatMessage(commonMessages.cancelButton) }}
                </Button>
                <Button type="colored" color="red" :disabled="isDeleting" @click="confirmDelete"
                    ><TrashIcon />
                    {{ formatMessage(commonMessages.deleteLabel) }}
                </Button>
            </div>
        </template>
    </NewModal>
</template>

<script setup lang="ts">
import {
    DownloadIcon,
    SearchIcon,
    TerminalSquareIcon,
    TrashIcon,
    WrapTextIcon,
    XIcon,
} from '@modrinth/assets'
import { computed, isRef, nextTick, onBeforeUnmount, ref } from 'vue'

import Admonition from '#ui/components/base/Admonition.vue'
import Button from '#ui/components/base/buttons/Button.vue'
import type { CollapsibleAdmonitionItem } from '#ui/components/base/CollapsibleAdmonition.vue'
import CollapsibleAdmonition from '#ui/components/base/CollapsibleAdmonition.vue'
import type { ComboboxOption } from '#ui/components/base/Combobox.vue'
import Combobox from '#ui/components/base/Combobox.vue'
import LoadingIndicator from '#ui/components/base/LoadingIndicator.vue'
import StyledInput from '#ui/components/base/StyledInput.vue'
import NewModal from '#ui/components/modal/NewModal.vue'
import ShareModal from '#ui/components/modal/ShareModal.vue'
import { useBodyScrollLock } from '#ui/composables/body-scroll-lock'
import { useVIntl } from '#ui/composables/i18n'
import { injectModrinthClient } from '#ui/providers'
import { injectModalBehavior } from '#ui/providers/modal-behavior'
import { injectPageContext } from '#ui/providers/page-context'
import { injectNotificationManager } from '#ui/providers/web-notifications.ts'
import { commonMessages } from '#ui/utils/common-messages'
import { shareLogs } from '#ui/utils/log-share'

import ConsoleActionButtons from './components/ConsoleActionButtons.vue'
import ConsoleFilterPills from './components/ConsoleFilterPills.vue'
import LogViewport from './components/LogViewport.vue'
import { useConsoleFilters } from './composables'
import { consoleMessages, localFindingMessages } from './messages'
import { injectConsoleManager } from './providers'
import type { LogLevel, LogLine } from './types'

const ctx = injectConsoleManager()
const client = injectModrinthClient()
const modalBehavior = injectModalBehavior()
const pageContext = injectPageContext(null)
const { addNotification } = injectNotificationManager()
const { formatMessage } = useVIntl()

const localFindingCopy = {
    jvm_arguments: {
        title: localFindingMessages.jvmArgumentsTitle,
        action: localFindingMessages.jvmArgumentsAction,
    },
    out_of_memory: {
        title: localFindingMessages.outOfMemoryTitle,
        action: localFindingMessages.outOfMemoryAction,
    },
    opengl_unsupported: {
        title: localFindingMessages.openglUnsupportedTitle,
        action: localFindingMessages.openglUnsupportedAction,
    },
    pixel_format: {
        title: localFindingMessages.pixelFormatTitle,
        action: localFindingMessages.pixelFormatAction,
    },
    openj9: {
        title: localFindingMessages.openj9Title,
        action: localFindingMessages.openj9Action,
    },
    java_too_new: {
        title: localFindingMessages.javaTooNewTitle,
        action: localFindingMessages.javaTooNewAction,
    },
    java_incompatible: {
        title: localFindingMessages.javaIncompatibleTitle,
        action: localFindingMessages.javaIncompatibleAction,
    },
    jdk_runtime: {
        title: localFindingMessages.jdkRuntimeTitle,
        action: localFindingMessages.jdkRuntimeAction,
    },
    java_32bit: {
        title: localFindingMessages.java32BitTitle,
        action: localFindingMessages.java32BitAction,
    },
    java_11_required: {
        title: localFindingMessages.java11RequiredTitle,
        action: localFindingMessages.java11RequiredAction,
    },
    forge_incomplete: {
        title: localFindingMessages.forgeIncompleteTitle,
        action: localFindingMessages.forgeIncompleteAction,
    },
    duplicate_mod: {
        title: localFindingMessages.duplicateModTitle,
        action: localFindingMessages.duplicateModAction,
    },
    incompatible_mods: {
        title: localFindingMessages.incompatibleModsTitle,
        action: localFindingMessages.incompatibleModsAction,
    },
    missing_dependency: {
        title: localFindingMessages.missingDependencyTitle,
        action: localFindingMessages.missingDependencyAction,
    },
    disk_space: {
        title: localFindingMessages.diskSpaceTitle,
        action: localFindingMessages.diskSpaceAction,
    },
    file_in_use: {
        title: localFindingMessages.fileInUseTitle,
        action: localFindingMessages.fileInUseAction,
    },
    connector_incompatible_fabric_mods: {
        title: localFindingMessages.connectorIncompatibleFabricModsTitle,
        action: localFindingMessages.connectorIncompatibleFabricModsAction,
    },
    missing_embeddium: {
        title: localFindingMessages.missingEmbeddiumTitle,
        action: localFindingMessages.missingEmbeddiumAction,
    },
    missing_indium: {
        title: localFindingMessages.missingIndiumTitle,
        action: localFindingMessages.missingIndiumAction,
    },
    mod_id_limit: {
        title: localFindingMessages.modIdLimitTitle,
        action: localFindingMessages.modIdLimitAction,
    },
    forge_error: {
        title: localFindingMessages.forgeErrorTitle,
        action: localFindingMessages.forgeErrorAction,
    },
    mod_loader_error: {
        title: localFindingMessages.modLoaderErrorTitle,
        action: localFindingMessages.modLoaderErrorAction,
    },
    mod_loader_failure: {
        title: localFindingMessages.modLoaderFailureTitle,
        action: localFindingMessages.modLoaderFailureAction,
    },
    stack_analysis: {
        title: localFindingMessages.stackAnalysisTitle,
        action: localFindingMessages.stackAnalysisAction,
    },
    short_output: {
        title: localFindingMessages.shortOutputTitle,
        action: localFindingMessages.shortOutputAction,
    },
    extracted_mod: {
        title: localFindingMessages.extractedModTitle,
        action: localFindingMessages.extractedModAction,
    },
    mixin_bootstrap: {
        title: localFindingMessages.mixinBootstrapTitle,
        action: localFindingMessages.mixinBootstrapAction,
    },
    mixin_failure: {
        title: localFindingMessages.mixinFailureTitle,
        action: localFindingMessages.mixinFailureAction,
    },
    fabric_solution: {
        title: localFindingMessages.fabricSolutionTitle,
        action: localFindingMessages.fabricSolutionAction,
    },
    mod_config: {
        title: localFindingMessages.modConfigTitle,
        action: localFindingMessages.modConfigAction,
    },
    optifine_incompatible: {
        title: localFindingMessages.optifineIncompatibleTitle,
        action: localFindingMessages.optifineIncompatibleAction,
    },
    resource_pack: {
        title: localFindingMessages.resourcePackTitle,
        action: localFindingMessages.resourcePackAction,
    },
    large_resource_pack: {
        title: localFindingMessages.largeResourcePackTitle,
        action: localFindingMessages.largeResourcePackAction,
    },
    shaders_optifine: {
        title: localFindingMessages.shadersOptifineTitle,
        action: localFindingMessages.shadersOptifineAction,
    },
    multiple_forge_versions: {
        title: localFindingMessages.multipleForgeVersionsTitle,
        action: localFindingMessages.multipleForgeVersionsAction,
    },
    forge_java_incompatible: {
        title: localFindingMessages.forgeJavaIncompatibleTitle,
        action: localFindingMessages.forgeJavaIncompatibleAction,
    },
    content_verification: {
        title: localFindingMessages.contentVerificationTitle,
        action: localFindingMessages.contentVerificationAction,
    },
    optifine_world: {
        title: localFindingMessages.optifineWorldTitle,
        action: localFindingMessages.optifineWorldAction,
    },
    nightconfig_bug: {
        title: localFindingMessages.nightconfigBugTitle,
        action: localFindingMessages.nightconfigBugAction,
    },
    mod_filename: {
        title: localFindingMessages.modFilenameTitle,
        action: localFindingMessages.modFilenameAction,
    },
    definite_mod: {
        title: localFindingMessages.definiteModTitle,
        action: localFindingMessages.definiteModAction,
    },
    definite_mod_fabric: {
        title: localFindingMessages.definiteModFabricTitle,
        action: localFindingMessages.definiteModFabricAction,
    },
    intel_driver: {
        title: localFindingMessages.intelDriverTitle,
        action: localFindingMessages.intelDriverAction,
    },
    amd_driver: {
        title: localFindingMessages.amdDriverTitle,
        action: localFindingMessages.amdDriverAction,
    },
    nvidia_driver: {
        title: localFindingMessages.nvidiaDriverTitle,
        action: localFindingMessages.nvidiaDriverAction,
    },
    manual_debug_crash: {
        title: localFindingMessages.manualDebugCrashTitle,
        action: localFindingMessages.manualDebugCrashAction,
    },
    suspected_mod: {
        title: localFindingMessages.suspectedModTitle,
        action: localFindingMessages.suspectedModAction,
    },
    mod_initialization: {
        title: localFindingMessages.modInitializationTitle,
        action: localFindingMessages.modInitializationAction,
    },
    specific_block: {
        title: localFindingMessages.specificBlockTitle,
        action: localFindingMessages.specificBlockAction,
    },
    specific_entity: {
        title: localFindingMessages.specificEntityTitle,
        action: localFindingMessages.specificEntityAction,
    },
    hs_err_al_lib_alc_cleanup: {
        title: consoleMessages.knownSignatureTitle,
        action: consoleMessages.knownSignatureAction,
    },
    hs_err_glfw_driver: {
        title: consoleMessages.knownSignatureTitle,
        action: consoleMessages.knownSignatureAction,
    },
    hs_err_intel_driver: {
        title: consoleMessages.knownSignatureTitle,
        action: consoleMessages.knownSignatureAction,
    },
    hs_err_java_too_high: {
        title: consoleMessages.knownSignatureTitle,
        action: consoleMessages.knownSignatureAction,
    },
    hs_err_jvm: {
        title: consoleMessages.knownSignatureTitle,
        action: consoleMessages.knownSignatureAction,
    },
    hs_err_openal: {
        title: consoleMessages.knownSignatureTitle,
        action: consoleMessages.knownSignatureAction,
    },
    hs_err_macos_shader: {
        title: consoleMessages.knownSignatureTitle,
        action: consoleMessages.knownSignatureAction,
    },
    hs_err_apple_jdk: {
        title: consoleMessages.knownSignatureTitle,
        action: consoleMessages.knownSignatureAction,
    },
    hs_err_gpu_driver: {
        title: consoleMessages.knownSignatureTitle,
        action: consoleMessages.knownSignatureAction,
    },
    create_addons: {
        title: consoleMessages.knownSignatureTitle,
        action: consoleMessages.knownSignatureAction,
    },
    ctov_missing_lithostitched: {
        title: consoleMessages.knownSignatureTitle,
        action: consoleMessages.knownSignatureAction,
    },
    curseforge_corrupted: {
        title: consoleMessages.knownSignatureTitle,
        action: consoleMessages.knownSignatureAction,
    },
    epic_fight_addons: {
        title: consoleMessages.knownSignatureTitle,
        action: consoleMessages.knownSignatureAction,
    },
    feature_order_cycle: {
        title: consoleMessages.knownSignatureTitle,
        action: consoleMessages.knownSignatureAction,
    },
    ferrite_core_neighbor_table: {
        title: consoleMessages.knownSignatureTitle,
        action: consoleMessages.knownSignatureAction,
    },
    geckolib_oculus_compat: {
        title: consoleMessages.knownSignatureTitle,
        action: consoleMessages.knownSignatureAction,
    },
    groovy_mod_loader_ipv6: {
        title: consoleMessages.knownSignatureTitle,
        action: consoleMessages.knownSignatureAction,
    },
    kubejs_datapack: {
        title: consoleMessages.knownSignatureTitle,
        action: consoleMessages.knownSignatureAction,
    },
    language_provider_mismatch: {
        title: consoleMessages.knownSignatureTitle,
        action: consoleMessages.knownSignatureAction,
    },
    legacy_too_many_ids: {
        title: consoleMessages.knownSignatureTitle,
        action: consoleMessages.knownSignatureAction,
    },
    module_resolution: {
        title: consoleMessages.knownSignatureTitle,
        action: consoleMessages.knownSignatureAction,
    },
    modernfix_watchdog: {
        title: consoleMessages.knownSignatureTitle,
        action: consoleMessages.knownSignatureAction,
    },
    neoforge_1_20_1: {
        title: consoleMessages.knownSignatureTitle,
        action: consoleMessages.knownSignatureAction,
    },
    resource_location: {
        title: consoleMessages.knownSignatureTitle,
        action: consoleMessages.knownSignatureAction,
    },
    rubidium_deprecated: {
        title: consoleMessages.knownSignatureTitle,
        action: consoleMessages.knownSignatureAction,
    },
    server_config_corrupted: {
        title: consoleMessages.knownSignatureTitle,
        action: consoleMessages.knownSignatureAction,
    },
    version_1_21: {
        title: consoleMessages.knownSignatureTitle,
        action: consoleMessages.knownSignatureAction,
    },
    used_by_another_process: {
        title: consoleMessages.knownSignatureTitle,
        action: consoleMessages.knownSignatureAction,
    },
    windows_closed_process: {
        title: consoleMessages.knownSignatureTitle,
        action: consoleMessages.knownSignatureAction,
    },
} as const

const localCrashHeader = computed(() => {
    const analysis = ctx.localCrashAnalysis?.value
    const findings = analysis?.findings.length ?? 0
    const sources = analysis?.sources.length ?? 0
    return formatMessage(consoleMessages.localCrashHeader, { findings, sources })
})

const localCrashItems = computed<CollapsibleAdmonitionItem[]>(() => {
    const analysis = ctx.localCrashAnalysis?.value
    if (!analysis) return []
    const items = analysis.findings.map((finding) => {
        const copy = localFindingCopy[finding.id as keyof typeof localFindingCopy]
        const title = copy
            ? formatMessage(copy.title)
            : formatMessage(consoleMessages.fallbackFindingTitle, { finding: finding.id })
        const action = copy
            ? formatMessage(copy.action)
            : formatMessage(consoleMessages.fallbackFindingAction)
        const evidence = finding.evidence.map(
            (item) => `${item.filename}:${item.line} - ${item.text}`,
        )
        const mods = analysis.mods.map((mod) => {
            const identity = mod.name || mod.id || mod.file_name
            const modId = mod.id && mod.id !== identity ? ` (${mod.id})` : ''
            return formatMessage(consoleMessages.matchedMod, {
                identity,
                modId,
                fileName: mod.file_name,
            })
        })
        return {
            title,
            descriptions: [action, ...mods, ...evidence],
        }
    })
    if (analysis.mod_changes.length > 0) {
        const counts = analysis.mod_change_counts
        const changeKindMessages = {
            added: consoleMessages.modChangeAdded,
            removed: consoleMessages.modChangeRemoved,
            modified: consoleMessages.modChangeModified,
        } as const
        items.push({
            title: formatMessage(consoleMessages.modChangesTitle),
            descriptions: [
                formatMessage(consoleMessages.modChangesSummary, counts),
                ...analysis.mod_changes.map((change) =>
                    formatMessage(consoleMessages.modChange, {
                        kind: formatMessage(changeKindMessages[change.kind]),
                        filename: change.filename,
                    }),
                ),
            ],
        })
    }
    if (analysis.windows_events.length > 0) {
        items.push({
            title: formatMessage(consoleMessages.windowsEventsTitle),
            descriptions: analysis.windows_events.map(
                (event) => `Event ${event.event_id} · ${event.provider}: ${event.message}`,
            ),
        })
    }
    return items
})

const crashHeader = computed(() => {
    const analysis = ctx.crashAnalysis?.value
    const findings = analysis?.findings.length ?? 0
    return formatMessage(consoleMessages.crashHeader, { findings })
})

const crashItems = computed<CollapsibleAdmonitionItem[]>(() => {
    const analysis = ctx.crashAnalysis?.value
    if (!analysis) return []
    return analysis.findings.map((finding) => {
        const copy = localFindingCopy[finding.id as keyof typeof localFindingCopy]
        const title = copy
            ? formatMessage(copy.title)
            : formatMessage(consoleMessages.fallbackFindingTitle, { finding: finding.id })
        const action = copy
            ? formatMessage(copy.action)
            : formatMessage(consoleMessages.fallbackFindingAction)
        const evidence = finding.evidence.map(
            (item) => `${item.filename}:${item.line} - ${item.text}`,
        )
        const mods = analysis.mods.map((mod) => {
            const identity = mod.name || mod.id || mod.file_name
            const modId = mod.id && mod.id !== identity ? ` (${mod.id})` : ''
            return formatMessage(consoleMessages.matchedMod, {
                identity,
                modId,
                fileName: mod.file_name,
            })
        })
        return {
            title,
            descriptions: [action, ...mods, ...evidence],
        }
    })
})

const viewportRef = ref<InstanceType<typeof LogViewport> | null>(null)
const shareModal = ref<InstanceType<typeof ShareModal> | null>(null)
const deleteModal = ref<InstanceType<typeof NewModal> | null>(null)
const isDeleting = ref(false)
const exportingCrashContext = ref(false)
const searchQuery = ref('')
const wrapLines = ref(false)
const logFontSize = ref(12)

const FONT_SIZES = [8, 10, 12, 14, 16, 18, 20, 24] as const
const fontSizeOptions = computed<ComboboxOption<number>[]>(() =>
    FONT_SIZES.map((size) => ({ value: size, label: `${size}px` })),
)

const isFullscreen = ref(false)
const { lock: lockBodyScroll, unlock: unlockBodyScroll } = useBodyScrollLock()
const fullscreenBodyClass = 'modrinth-console-fullscreen-active'
const fullscreenIntercomPadding = 20
const fullscreenIntercomPaddingRequestId = Symbol('console-fullscreen')
const isApp =
    typeof window !== 'undefined' && !!(window as Record<string, unknown>).__TAURI_INTERNALS__
const isSharing = ref(false)
const { activeFilters, toggleFilter, buildFilterPredicate } = useConsoleFilters()
const hasLogs = computed(() => ctx.logLines.value.length > 0)
const isLiveSource = computed(() => {
    const sources = ctx.logSources?.value
    const index = ctx.activeLogSourceIndex?.value
    if (!sources || index === undefined) return true
    return sources[index]?.live ?? true
})
const logSourceOptions = computed(() =>
    (ctx.logSources?.value ?? []).map((s, i) => ({ value: i, label: s.name })),
)

async function handleExportCrashContext() {
    if (!ctx.onExportCrashContext || exportingCrashContext.value) return
    exportingCrashContext.value = true
    try {
        await ctx.onExportCrashContext()
    } finally {
        exportingCrashContext.value = false
    }
}

function buildCombinedPredicate(): ((line: LogLine) => boolean) | null {
    const levelPred = buildFilterPredicate()
    const query = searchQuery.value.trim().toLowerCase()
    if (!levelPred && !query) return null
    return (line: LogLine) => {
        if (levelPred && !levelPred(line)) return false
        if (query && !line.text.toLowerCase().includes(query)) return false
        return true
    }
}

const filteredLines = computed(() => {
    const predicate = buildCombinedPredicate()
    const src = ctx.logLines.value
    if (!predicate) {
        return src.map((line, i) => ({ line, originalIndex: i }))
    }
    const out: Array<{ line: LogLine; originalIndex: number }> = []
    for (let i = 0; i < src.length; i++) {
        if (predicate(src[i]!)) out.push({ line: src[i]!, originalIndex: i })
    }
    return out
})

onBeforeUnmount(() => {
    if (isFullscreen.value) {
        unlockBodyScroll()
        document.body.classList.remove(fullscreenBodyClass)
        window.dispatchEvent(new CustomEvent('modrinth-console-fullscreen', { detail: false }))
        pageContext?.intercomBubble?.requestHorizontalPadding?.(
            fullscreenIntercomPaddingRequestId,
            null,
        )
        modalBehavior?.onHide?.()
    }
})

// needs historical log start/end flags on ws to be properly useful
const resolvedLoading = computed(() => {
    const v = ctx.loading
    if (!v) return false
    return v.value
})

const resolvedShareDisabled = computed(() => {
    const v = ctx.shareDisabled
    if (!v) return false
    return isRef(v) ? v.value : v
})

const commandInput = ref('')

const showCommandInput = computed(() => {
    if (!ctx.sendCommand) return false
    return unwrapMaybeRef(ctx.showCommandInput) ?? false
})

const commandDisabled = computed(() => unwrapMaybeRef(ctx.disableCommandInput) ?? false)

const commandDisabledTooltip = computed(() => ctx.disableCommandInputTooltip?.value)

const commandPlaceholder = computed(() => {
    if (!commandDisabled.value) return formatMessage(consoleMessages.commandPlaceholder)
    return formatMessage(
        ctx.emptyStateType === 'server'
            ? consoleMessages.serverNotRunning
            : consoleMessages.commandInputDisabled,
    )
})

function submitCommand() {
    const command = commandInput.value.trim()
    if (!command || commandDisabled.value || !ctx.sendCommand) return
    ctx.sendCommand(command)
    commandInput.value = ''
    // The user just interacted with the console: pin the view to the bottom
    // so the command echo and its response are visible immediately.
    viewportRef.value?.scrollToBottom()
}

// Re-pins the viewport to the bottom (and re-enables bottom-following).
// Exposed so hosts can react to external events such as a server starting.
function scrollToBottom() {
    viewportRef.value?.scrollToBottom()
}

defineExpose({
    scrollToBottom,
})

const showDelete = computed(() => !isLiveSource.value && ctx.onDelete != null)

const resolvedDeleteDisabled = computed(() => {
    const v = ctx.deleteDisabled
    if (!v) return false
    return isRef(v) ? v.value : v
})

function unwrapMaybeRef<T>(value: T | { value: T } | undefined): T | undefined {
    if (value === undefined) return undefined
    return isRef(value) ? value.value : value
}

const resolvedDeleteDisabledTooltip = computed(() =>
    resolvedDeleteDisabled.value ? unwrapMaybeRef(ctx.deleteDisabledTooltip) : undefined,
)

const resolvedClearDisabled = computed(() => {
    const v = ctx.clearDisabled
    if (!v) return false
    return isRef(v) ? v.value : v
})

const resolvedClearDisabledTooltip = computed(() =>
    resolvedClearDisabled.value ? unwrapMaybeRef(ctx.clearDisabledTooltip) : undefined,
)

function handleFilterToggle(value: LogLevel) {
    toggleFilter(value)
}

async function toggleFullscreen() {
    const viewportState = viewportRef.value?.captureViewState()
    isFullscreen.value = !isFullscreen.value
    if (isFullscreen.value) {
        lockBodyScroll()
        document.body.classList.add(fullscreenBodyClass)
        // Let the shell collapse the account sidebar so the collapse control
        // stays reachable and the expanded pane uses the full width (#584).
        window.dispatchEvent(new CustomEvent('modrinth-console-fullscreen', { detail: true }))
        pageContext?.intercomBubble?.requestHorizontalPadding?.(
            fullscreenIntercomPaddingRequestId,
            fullscreenIntercomPadding,
        )
        modalBehavior?.onShow?.()
    } else {
        unlockBodyScroll()
        document.body.classList.remove(fullscreenBodyClass)
        window.dispatchEvent(new CustomEvent('modrinth-console-fullscreen', { detail: false }))
        pageContext?.intercomBubble?.requestHorizontalPadding?.(
            fullscreenIntercomPaddingRequestId,
            null,
        )
        modalBehavior?.onHide?.()
    }

    await nextTick()
    requestAnimationFrame(() => {
        if (viewportState) viewportRef.value?.restoreViewState(viewportState)
    })
}

function handleClear() {
    if (resolvedClearDisabled.value) return
    ctx.onClear?.()
}

function handleDelete() {
    deleteModal.value?.show()
}

async function confirmDelete() {
    if (!ctx.onDelete) return
    isDeleting.value = true
    try {
        await ctx.onDelete()
        deleteModal.value?.hide()
    } catch (err) {
        console.error('Failed to delete log file:', err)
        addNotification({
            type: 'error',
            title: formatMessage(consoleMessages.deleteFailedTitle),
            text: typeof err === 'string' ? err : formatMessage(consoleMessages.unknownError),
        })
    } finally {
        isDeleting.value = false
    }
}

async function handleShare() {
    const predicate = buildCombinedPredicate()
    const lines = predicate ? ctx.logLines.value.filter(predicate) : ctx.logLines.value
    const content = lines.map((l) => l.text).join('\n')

    isSharing.value = true
    try {
        const result = await shareLogs(client, content)
        if (result.truncated) {
            addNotification({
                type: 'warning',
                title: formatMessage(consoleMessages.shareTruncatedWarning),
            })
        }
        if (result.url) {
            shareModal.value?.show(result.url)
        }
    } catch (err) {
        console.error('Failed to share logs:', err)
        addNotification({
            type: 'error',
            title: formatMessage(consoleMessages.shareFailedTitle),
            text: typeof err === 'string' ? err : formatMessage(consoleMessages.unknownError),
        })
    } finally {
        isSharing.value = false
    }
}
</script>

<style>
.terminal-loading-fade-enter-active,
.terminal-loading-fade-leave-active {
    transition: opacity 250ms ease-in-out;
}

.terminal-loading-fade-enter-from,
.terminal-loading-fade-leave-to {
    opacity: 0;
}

.modrinth-console-fullscreen-active .intercom-lightweight-app,
.modrinth-console-fullscreen-active .intercom-lightweight-app-launcher,
.modrinth-console-fullscreen-active .intercom-lightweight-app-messenger,
.modrinth-console-fullscreen-active .intercom-launcher-frame,
.modrinth-console-fullscreen-active .intercom-messenger-frame,
.modrinth-console-fullscreen-active #intercom-container,
.modrinth-console-fullscreen-active #intercom-frame,
.modrinth-console-fullscreen-active iframe[name='intercom-launcher-frame'],
.modrinth-console-fullscreen-active iframe[name='intercom-messenger-frame'] {
    z-index: 14 !important;
}

.modrinth-console-fullscreen-active .loading-indicator-container,
.modrinth-console-fullscreen-active .app-contents::before {
    z-index: 14 !important;
}

.modrinth-console-fullscreen-active .app-grid-navbar,
.modrinth-console-fullscreen-active .app-grid-statusbar {
    z-index: 0 !important;
}
</style>

<style scoped>
.console-loading-scrim {
    background-color: color-mix(
        in srgb,
        var(--surface-3) var(--opacity-ratio-keep-80),
        transparent
    );
}
</style>
