<script setup lang="ts">
import DownloadIcon from '@modrinth/assets/icons/download.svg?component'
import FolderSearchIcon from '@modrinth/assets/icons/folder-search.svg?component'
import ListIcon from '@modrinth/assets/icons/list.svg?component'
import ScanEyeIcon from '@modrinth/assets/icons/scan-eye.svg?component'
import SearchIcon from '@modrinth/assets/icons/search.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Checkbox from '@modrinth/ui/src/components/base/Checkbox.vue'
import Slider from '@modrinth/ui/src/components/base/Slider.vue'
import Toggle from '@modrinth/ui/src/components/base/Toggle.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { ref, watch } from 'vue'

import SettingsSection from '../components/SettingsSection.vue'
import { messages } from '../messages'
import { PREVIEW_PLATFORM, useSettingsPage } from '../state'
import DownloadJavaModal from './java/DownloadJavaModal.vue'
import { collectGcContext, type GcContext, getJavaArgumentPresets } from './java/gc'
import InstalledJavaModal from './java/InstalledJavaModal.vue'
import JavaArgumentsInput from './java/JavaArgumentsInput.vue'
import JavaSelector from './java/JavaSelector.vue'
import { type JavaInstallJob, type JavaVersion, useJre } from './java/jre'
import MacOpenPanel from './java/MacOpenPanel.vue'
import MemoryAllocationDisplay from './java/MemoryAllocationDisplay.vue'
import useMemorySlider from './java/useMemorySlider'

/**
 * Java & performance (apps/app-frontend/src/components/ui/settings/JavaSettings.vue):
 * the default Java for each supported major version, scans, manual add, downloads and
 * the installed list, then the memory allocation and the Java arguments. On macOS the
 * high-performance GPU switch and memory optimization do not apply, so the page hides
 * them as the app does. Java work runs against the simulated backend (`./java/jre.ts`).
 */
const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const { settings, simulate } = useSettingsPage()
const jre = useJre()

const supportedJavaVersions = [25, 21, 17, 8]
const javaDefaults = ref<Record<number, Partial<JavaVersion> | null | undefined>>({})
const scanning = ref(false)
const scanMode = ref('')
const showDeepScanConfirm = ref(false)
const downloadJavaModal = ref<InstanceType<typeof DownloadJavaModal> | null>(null)
const installedJavaModal = ref<InstanceType<typeof InstalledJavaModal> | null>(null)
const openPanel = ref<InstanceType<typeof MacOpenPanel> | null>(null)
const defaultSaveQueues = new Map<number, Promise<void>>()

const currentPlatform: string = PREVIEW_PLATFORM
const supportsHighPerformanceMode = ['windows', 'linux'].includes(currentPlatform)
const supportsMemoryOptimization = currentPlatform === 'windows'
// `get()`: the settings row.
await simulate()
const autoHighPerformanceMode = ref(settings.auto_set_java_high_performance_mode ?? false)

const javaArgs = ref((settings.extra_launch_args ?? []).join(' '))

const memory = ref(
    settings.memory
        ? { optimize_before_launch: false, ...settings.memory }
        : { maximum: 2048, automatic: true, optimize_before_launch: false },
)

let shouldApplyDefaultAuto = (settings.extra_launch_args?.length ?? 0) === 0

const memorySlider = useMemorySlider()
const maxMemory = memorySlider?.maxMemory ?? 4096
const snapPoints = memorySlider?.snapPoints ?? []

const gcContext = ref<GcContext | undefined>(undefined)

async function updateGcContext() {
    gcContext.value = await collectGcContext(memory.value.maximum, null, null, 0)

    if (shouldApplyDefaultAuto) {
        const autoPreset = getJavaArgumentPresets(gcContext.value).find(
            (preset) => preset.id === 'gc-auto',
        )

        if (autoPreset) {
            javaArgs.value = autoPreset.resolveArgs
                ? autoPreset.resolveArgs(gcContext.value)
                : autoPreset.args
        }

        shouldApplyDefaultAuto = false
    }
}

await updateGcContext()

watch(() => memory.value.maximum, updateGcContext)

watch(
    [autoHighPerformanceMode, memory, javaArgs],
    () => {
        // `saveDraft`: the edited fields go to the settings row.
        settings.auto_set_java_high_performance_mode = autoHighPerformanceMode.value
        settings.memory = { ...memory.value }
        settings.extra_launch_args = javaArgs.value.trim().split(/\s+/).filter(Boolean)
    },
    { deep: true },
)

async function reloadDefaults() {
    const defaults = await jre.getJavaDefaultVersions().catch(handleError)
    if (!defaults) return

    javaDefaults.value = Object.fromEntries(
        defaults.map((javaVersion) => [javaVersion.parsed_version, javaVersion]),
    )
}

await reloadDefaults()

async function persistDefault(majorVersion: number, javaVersion?: Partial<JavaVersion> | null) {
    const path = javaVersion?.path?.trim()
    if (!path) {
        const removed = await jre
            .removeJavaDefaultVersion(majorVersion)
            .then(() => true)
            .catch((error) => {
                handleError(error)
                return false
            })
        if (removed) {
            javaDefaults.value[majorVersion] = undefined
        } else {
            await reloadDefaults()
        }
        return
    }

    const validated = await jre.setJavaDefaultVersion(majorVersion, path).catch((error) => {
        handleError(error)
        return null
    })
    if (validated) {
        javaDefaults.value[majorVersion] = validated
    } else {
        await reloadDefaults()
    }
}

function saveDefault(majorVersion: number, javaVersion?: Partial<JavaVersion> | null) {
    const previous = defaultSaveQueues.get(majorVersion) ?? Promise.resolve()
    const operation = previous.then(() => persistDefault(majorVersion, javaVersion))
    defaultSaveQueues.set(majorVersion, operation)

    return operation.finally(() => {
        if (defaultSaveQueues.get(majorVersion) === operation) {
            defaultSaveQueues.delete(majorVersion)
        }
    })
}

async function runScan(exhaustive: boolean) {
    if (exhaustive) {
        showDeepScanConfirm.value = true
        return
    }

    scanning.value = true
    scanMode.value = 'quick'
    try {
        await jre.findFilteredJres(null, false, true, false).catch(handleError)
    } finally {
        scanning.value = false
        scanMode.value = ''
    }
}

async function confirmDeepScan() {
    showDeepScanConfirm.value = false
    scanning.value = true
    scanMode.value = 'deep'
    try {
        await jre.findFilteredJres(null, true, true, true).catch(handleError)
    } finally {
        scanning.value = false
        scanMode.value = ''
    }
}

async function handleManualAdd() {
    const result = await openPanel.value?.pick()
    if (!result) return

    const filePath = result
    const javaInfo = await jre.getJre(filePath).catch(handleError)
    if (!javaInfo) return

    await jre.setJavaVersion(javaInfo).catch(handleError)
}

async function onJavaDownloaded(job: JavaInstallJob) {
    if (job?.job_id) {
        await jre.waitForInstallJob(job.job_id).catch(handleError)
    }
    await reloadDefaults()
}
</script>

<template>
    <DownloadJavaModal ref="downloadJavaModal" @downloaded="onJavaDownloaded" />
    <InstalledJavaModal ref="installedJavaModal" @changed="reloadDefaults" />
    <MacOpenPanel ref="openPanel" />

    <div class="settings-page flex flex-col gap-6">
        <SettingsSection
            v-for="(javaVersion, index) in supportedJavaVersions"
            :key="`java-${javaVersion}`"
        >
            <template #header>
                <h2
                    :id="index === 0 ? 'settings-target-java-installations' : undefined"
                    :tabindex="index === 0 ? -1 : undefined"
                    class="m-0 text-lg font-semibold text-[var(--color-text-primary)]"
                >
                    {{ formatMessage(messages.appSettingsJavaLocation, { version: javaVersion }) }}
                </h2>
            </template>
            <div class="p-4">
                <JavaSelector
                    :id="`java-selector-${javaVersion}`"
                    v-model="javaDefaults[javaVersion]"
                    :version="javaVersion"
                    @commit="saveDefault(javaVersion, $event)"
                />
            </div>
        </SettingsSection>

        <div class="flex flex-wrap gap-2 border-0 border-t border-solid border-button-border pt-5">
            <Button
                type="base"
                native-type="button"
                class="!shadow-none"
                :disabled="scanning"
                @click="runScan(false)"
            >
                <SearchIcon aria-hidden="true" />
                {{
                    scanning && scanMode === 'quick'
                        ? formatMessage(messages.appSettingsJavaScanning)
                        : formatMessage(messages.appSettingsJavaFindJava)
                }}
            </Button>
            <Button
                type="base"
                native-type="button"
                class="!shadow-none"
                :disabled="scanning"
                @click="runScan(true)"
            >
                <ScanEyeIcon aria-hidden="true" />
                {{
                    scanning && scanMode === 'deep'
                        ? formatMessage(messages.appSettingsJavaScanning)
                        : formatMessage(messages.appSettingsJavaDeepScan)
                }}
            </Button>
            <Button
                type="base"
                native-type="button"
                class="!shadow-none"
                :disabled="scanning"
                @click="handleManualAdd"
            >
                <FolderSearchIcon aria-hidden="true" />
                {{ formatMessage(messages.appSettingsJavaManualAdd) }}
            </Button>
            <Button
                type="base"
                native-type="button"
                class="!shadow-none"
                :disabled="scanning"
                @click="downloadJavaModal?.show()"
            >
                <DownloadIcon aria-hidden="true" />
                {{ formatMessage(messages.appSettingsJavaDownloadJava) }}
            </Button>
            <Button
                type="base"
                native-type="button"
                class="!shadow-none"
                @click="installedJavaModal?.show()"
            >
                <ListIcon aria-hidden="true" />
                {{ formatMessage(messages.appSettingsJavaViewInstalled) }}
            </Button>
        </div>

        <div
            v-if="showDeepScanConfirm"
            class="deep-scan-warning flex flex-col gap-2 rounded-lg border border-warning p-3 text-sm"
        >
            <span>{{ formatMessage(messages.appSettingsJavaDeepScanConfirm) }}</span>
            <div class="flex flex-wrap gap-2">
                <Button type="colored" color="red" native-type="button" @click="confirmDeepScan">
                    {{ formatMessage(messages.appSettingsJavaScanAnyway) }}
                </Button>
                <Button type="outlined" native-type="button" @click="showDeepScanConfirm = false">
                    {{ formatMessage(messages.appSettingsJavaCancel) }}
                </Button>
            </div>
        </div>

        <div
            v-if="supportsHighPerformanceMode"
            class="border-0 border-t border-solid border-button-border pt-5"
        >
            <div class="flex items-center justify-between gap-4">
                <div class="flex min-w-0 flex-col gap-1">
                    <span class="text-sm font-semibold text-[var(--color-text-primary)]">
                        {{ formatMessage(messages.appSettingsJavaAutoHighPerformanceMode) }}
                    </span>
                    <span class="text-xs text-[var(--color-text-tertiary)]">
                        {{
                            formatMessage(
                                messages.appSettingsJavaAutoHighPerformanceModeDescription,
                            )
                        }}
                    </span>
                </div>
                <Toggle id="auto-java-high-performance-mode" v-model="autoHighPerformanceMode" />
            </div>
        </div>

        <div class="flex flex-col gap-6 border-0 border-t border-solid border-button-border pt-5">
            <div class="flex flex-col gap-2.5">
                <h2
                    id="settings-target-java-memory"
                    tabindex="-1"
                    class="m-0 text-lg font-semibold text-[var(--color-text-primary)]"
                >
                    {{ formatMessage(messages.appSettingsDefaultsMemory) }}
                </h2>

                <Checkbox
                    v-model="memory.automatic"
                    :label="formatMessage(messages.appSettingsDefaultsAutomaticMemory)"
                />
                <div v-if="supportsMemoryOptimization" class="flex flex-col gap-1">
                    <Checkbox
                        v-model="memory.optimize_before_launch"
                        :label="
                            formatMessage(messages.appSettingsDefaultsOptimizeMemoryBeforeLaunch)
                        "
                    />
                    <p class="m-0 text-xs leading-tight text-[var(--color-text-tertiary)]">
                        {{
                            formatMessage(
                                messages.appSettingsDefaultsOptimizeMemoryBeforeLaunchDescription,
                            )
                        }}
                    </p>
                </div>

                <Slider
                    id="max-memory"
                    v-model="memory.maximum"
                    :disabled="memory.automatic"
                    :min="512"
                    :max="maxMemory"
                    :step="64"
                    :snap-points="snapPoints"
                    :snap-range="512"
                    unit="MB"
                />

                <p class="m-0 mt-1 leading-tight">
                    {{
                        formatMessage(
                            memory.automatic
                                ? messages.appSettingsDefaultsAutomaticMemoryDescription
                                : messages.appSettingsDefaultsMemoryDescription,
                        )
                    }}
                </p>

                <MemoryAllocationDisplay :memory="memory" show-optimize-button />
            </div>

            <div class="flex flex-col gap-2.5">
                <h2
                    id="settings-target-java-arguments"
                    tabindex="-1"
                    class="m-0 text-lg font-semibold text-[var(--color-text-primary)]"
                >
                    {{ formatMessage(messages.appSettingsDefaultsJavaArguments) }}
                </h2>

                <JavaArgumentsInput
                    id="java-args"
                    v-model="javaArgs"
                    :gc-context="gcContext"
                    :placeholder="
                        formatMessage(messages.appSettingsDefaultsJavaArgumentsPlaceholder)
                    "
                />
            </div>
        </div>
    </div>
</template>

<style scoped>
/* The action row at the end of the page is not a card; it only carries a
   separator above it. Cards come from `SettingsSection` now, so there is no
   positional rule turning every top-level child into one. */
.settings-page > div {
    border-top: 1px solid var(--surface-4);
    padding-top: var(--gap-lg);
}

/* `warning` is not one of the tint families (there is no `--color-warning-bg`
   in the app palette), so the box uses a mix of the app's warning tone
   (orange, see Admonition / AppActionBar) at the rung the fixed /10 used. */
.deep-scan-warning {
    background-color: color-mix(
        in srgb,
        var(--color-orange) var(--opacity-ratio-keep-10),
        transparent
    );
}
</style>
