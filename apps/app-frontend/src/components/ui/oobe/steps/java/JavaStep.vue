<script setup lang="ts">
import {
    CheckIcon,
    CoffeeIcon,
    DownloadIcon,
    FolderSearchIcon,
    PlusIcon,
    ScanEyeIcon,
    SpinnerIcon,
} from '@modrinth/assets'
import {
    Button,
    commonMessages,
    defineMessages,
    injectNotificationManager,
    StyledInput,
    useVIntl,
} from '@modrinth/ui'
import { open } from '@tauri-apps/plugin-dialog'
import { computed, onMounted, onUnmounted, reactive, ref, watch } from 'vue'

import DownloadJavaModal from '@/components/ui/settings/DownloadJavaModal.vue'
import { trackEvent } from '@/helpers/analytics'
import { errorText } from '@/helpers/error-text'
import { java_discovery_listener } from '@/helpers/events'
import { wait_for_install_job } from '@/helpers/install'
import { confirmJavaSelection } from '@/helpers/java-selection'
import {
    find_filtered_jres,
    get_java_default_versions,
    get_jre,
    set_java_default_version,
    set_java_version,
} from '@/helpers/jre'

import { useOobeStep } from '../../context'
import OobeChoice from '../../parts/OobeChoice.vue'

type JavaVersion = {
    parsed_version: number
    version: string
    architecture: string
    path: string
    distribution?: string | null
}
type Download = { id: string; state: 'downloading' | 'done' | 'failed'; message?: string }

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const step = useOobeStep()

const messages = defineMessages({
    foundTitle: { id: 'app.oobe.java.found.title', defaultMessage: 'Found on this device' },
    foundDescription: {
        id: 'app.oobe.java.found.description',
        defaultMessage: 'Pick the one to use for each Java version.',
    },
    scanning: { id: 'app.java.searching', defaultMessage: 'Looking for Java installations…' },
    noneFound: {
        id: 'app.oobe.java.none-found',
        defaultMessage: 'No Java installations were found on this device.',
    },
    versionLabel: {
        id: 'app.settings.java.download.version-label',
        defaultMessage: 'Java {version}',
    },
    deepScan: { id: 'app.settings.java.deep-scan', defaultMessage: 'Deep Scan' },
    deepScanConfirm: {
        id: 'app.settings.java.deep-scan-confirm',
        defaultMessage: 'This will scan ALL directories on ALL drives. May take several minutes.',
    },
    scanAnyway: { id: 'app.settings.java.scan-anyway', defaultMessage: 'Scan Anyway' },
    addTitle: { id: 'app.oobe.java.add.title', defaultMessage: 'Add a Java you already have' },
    addDescription: {
        id: 'app.oobe.java.add.description',
        defaultMessage: 'Enter the path to a java executable, or browse for it.',
    },
    pathPlaceholder: { id: 'app.oobe.java.add.placeholder', defaultMessage: '/path/to/java' },
    browse: { id: 'app.java.browse', defaultMessage: 'Browse' },
    add: { id: 'app.oobe.java.add.action', defaultMessage: 'Add' },
    invalid: {
        id: 'app.oobe.java.add.invalid',
        defaultMessage: 'No Java runtime could be started from this path.',
    },
    added: {
        id: 'app.oobe.java.add.added',
        defaultMessage: 'Added Java {version}. It is now used for Java {major}.',
    },
    downloadTitle: { id: 'app.oobe.java.download.title', defaultMessage: 'Download Java' },
    downloadDescription: {
        id: 'app.oobe.java.download.description',
        defaultMessage:
            'Choose a distribution and a version. You can keep going while it downloads.',
    },
    downloadJava: { id: 'app.settings.java.download-java', defaultMessage: 'Download Java' },
    downloading: { id: 'app.oobe.java.download.downloading', defaultMessage: 'Downloading Java…' },
    downloaded: { id: 'app.oobe.java.download.done', defaultMessage: 'Java downloaded.' },
})

const javas = ref<JavaVersion[]>([])
/** The path each Java major version uses by default. */
const defaults = reactive(new Map<number, string>())
const scanning = ref(true)
const deepScanning = ref(false)
const confirmingDeepScan = ref(false)
const choosing = ref<string | null>(null)
const changed = ref(false)

const manualPath = ref('')
const adding = ref(false)
const addResult = ref<{ ok: boolean; message: string } | null>(null)

const downloadModal = ref<{ show(): void } | null>(null)
const downloads = ref<Download[]>([])

const groups = computed(() => {
    const byMajor = new Map<number, JavaVersion[]>()
    for (const java of javas.value) {
        byMajor.set(java.parsed_version, [...(byMajor.get(java.parsed_version) ?? []), java])
    }
    return [...byMajor.entries()]
        .sort(([a], [b]) => b - a)
        .map(([major, entries]) => ({
            major,
            entries: entries.sort((a, b) => a.path.localeCompare(b.path)),
        }))
})

watch(changed, (value) => step.setReady(value), { immediate: true })

let unlisten: (() => void) | null = null
let disposed = false

onMounted(async () => {
    java_discovery_listener(() => refresh())
        .then((stop: () => void) => {
            if (disposed) stop()
            else unlisten = stop
        })
        .catch((error: unknown) => console.warn('Failed to watch Java discovery', error))
    await Promise.all([loadDefaults(), scan(false)])
})

onUnmounted(() => {
    disposed = true
    unlisten?.()
})

function markChanged() {
    changed.value = true
    step.markChanged()
}

async function loadDefaults() {
    const found = (await get_java_default_versions().catch(handleError)) as
        JavaVersion[] | undefined
    if (!found) return
    defaults.clear()
    for (const java of found) defaults.set(java.parsed_version, java.path)
}

async function refresh() {
    const found = (await find_filtered_jres(null, false, false, false).catch(() => null)) as
        JavaVersion[] | null
    if (found) javas.value = found
}

async function scan(exhaustive: boolean) {
    if (exhaustive) {
        confirmingDeepScan.value = false
        deepScanning.value = true
        trackEvent('JavaDeepScan', { source: 'oobe' })
    } else {
        trackEvent('JavaQuickScan', { source: 'oobe' })
    }
    scanning.value = true
    try {
        const found = (await find_filtered_jres(null, exhaustive, exhaustive, exhaustive)) as
            JavaVersion[] | null
        if (found) javas.value = found
    } catch (error) {
        handleError(error as Error)
    } finally {
        scanning.value = false
        deepScanning.value = false
    }
}

function detailOf(java: JavaVersion) {
    return [java.version, java.distribution, java.architecture].filter(Boolean).join(' · ')
}

/** Makes `java` the default for its major version, as Settings does. */
async function choose(java: JavaVersion) {
    if (choosing.value) return
    choosing.value = java.path
    step.setBusy(true)
    try {
        const selected = await confirmJavaSelection(java, formatMessage)
        if (!selected || defaults.get(selected.parsed_version) === selected.path) return
        const saved = (await set_java_default_version(selected.parsed_version, selected.path)) as
            JavaVersion | undefined
        defaults.set(selected.parsed_version, saved?.path ?? selected.path)
        markChanged()
    } catch (error) {
        handleError(error as Error)
    } finally {
        choosing.value = null
        step.setBusy(false)
    }
}

async function browse() {
    const choice = await open({ multiple: false })
    if (typeof choice !== 'string') return
    manualPath.value = choice
    await addManual()
}

async function addManual() {
    const path = manualPath.value.trim()
    if (!path || adding.value) return
    adding.value = true
    addResult.value = null
    step.setBusy(true)
    try {
        const java = (await get_jre(path).catch(() => null)) as JavaVersion | null
        if (!java) {
            addResult.value = { ok: false, message: formatMessage(messages.invalid) }
            return
        }
        const selected = await confirmJavaSelection(java, formatMessage)
        if (!selected) return
        await set_java_version(selected)
        const saved = ((await set_java_default_version(selected.parsed_version, selected.path)) ??
            selected) as JavaVersion
        defaults.set(saved.parsed_version, saved.path)
        if (!javas.value.some((entry) => entry.path === saved.path)) {
            javas.value = [...javas.value, saved]
        }
        trackEvent('JavaManualSelect', { path: saved.path, source: 'oobe' })
        markChanged()
        manualPath.value = ''
        addResult.value = {
            ok: true,
            message: formatMessage(messages.added, {
                version: saved.version,
                major: saved.parsed_version,
            }),
        }
    } catch (error) {
        addResult.value = {
            ok: false,
            message: errorText(error),
        }
    } finally {
        adding.value = false
        step.setBusy(false)
    }
}

async function onDownloaded(job: { job_id?: string } | null | undefined) {
    if (!job?.job_id) return
    const download = reactive<Download>({ id: job.job_id, state: 'downloading' })
    downloads.value = [...downloads.value, download]
    markChanged()
    try {
        await wait_for_install_job(job.job_id)
        download.state = 'done'
        await Promise.all([loadDefaults(), refresh()])
    } catch (error) {
        download.state = 'failed'
        download.message = errorText(error)
    }
}
</script>

<template>
    <DownloadJavaModal ref="downloadModal" @downloaded="onDownloaded" />

    <div class="flex flex-col gap-6">
        <section class="flex flex-col gap-3">
            <div class="flex items-start gap-3">
                <div class="flex min-w-0 flex-1 flex-col gap-1">
                    <h2 class="m-0 text-base font-semibold text-[var(--color-text-primary)]">
                        {{ formatMessage(messages.foundTitle) }}
                    </h2>
                    <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                        {{ formatMessage(messages.foundDescription) }}
                    </p>
                </div>
                <Button
                    type="quiet"
                    class="shrink-0"
                    :disabled="scanning"
                    @click="confirmingDeepScan = true"
                >
                    <SpinnerIcon v-if="deepScanning" class="animate-spin" />
                    <ScanEyeIcon v-else />
                    {{ formatMessage(messages.deepScan) }}
                </Button>
            </div>

            <div
                v-if="confirmingDeepScan"
                class="oobe-java-warning flex flex-col gap-3 rounded-[var(--radius-lg)] px-4 py-3 text-sm text-[var(--color-text-primary)]"
            >
                <span>{{ formatMessage(messages.deepScanConfirm) }}</span>
                <div class="flex flex-wrap gap-2">
                    <Button type="colored" color="red" @click="scan(true)">
                        {{ formatMessage(messages.scanAnyway) }}
                    </Button>
                    <Button type="outlined" @click="confirmingDeepScan = false">
                        {{ formatMessage(commonMessages.cancelButton) }}
                    </Button>
                </div>
            </div>

            <div
                v-if="scanning && groups.length === 0"
                class="flex flex-col gap-2"
                aria-busy="true"
            >
                <div
                    v-for="index in 2"
                    :key="index"
                    class="flex items-center gap-3 rounded-[var(--radius-lg)] bg-surface-3 px-4 py-3"
                >
                    <span class="oobe-skeleton size-9 rounded-[var(--radius-sm)]" />
                    <span class="flex flex-1 flex-col gap-2">
                        <span class="oobe-skeleton h-3 w-32 rounded-full" />
                        <span class="oobe-skeleton h-2.5 w-52 rounded-full" />
                    </span>
                </div>
                <p class="m-0 flex items-center gap-2 text-sm text-[var(--color-text-tertiary)]">
                    <SpinnerIcon class="size-4 animate-spin" />
                    {{ formatMessage(messages.scanning) }}
                </p>
            </div>

            <p
                v-else-if="groups.length === 0"
                class="m-0 rounded-[var(--radius-lg)] bg-surface-3 px-4 py-3 text-sm text-[var(--color-text-tertiary)]"
            >
                {{ formatMessage(messages.noneFound) }}
            </p>

            <template v-else>
                <div v-for="group in groups" :key="group.major" class="flex flex-col gap-2">
                    <span
                        :id="`oobe-java-${group.major}`"
                        class="text-sm font-semibold text-[var(--color-text-tertiary)]"
                    >
                        {{ formatMessage(messages.versionLabel, { version: group.major }) }}
                    </span>
                    <div
                        role="radiogroup"
                        :aria-labelledby="`oobe-java-${group.major}`"
                        class="flex flex-col gap-2"
                    >
                        <OobeChoice
                            v-for="java in group.entries"
                            :key="java.path"
                            role="radio"
                            :selected="defaults.get(group.major) === java.path"
                            :disabled="choosing !== null && choosing !== java.path"
                            @select="choose(java)"
                        >
                            <template #leading>
                                <span
                                    class="flex size-9 shrink-0 items-center justify-center rounded-[var(--radius-sm)] bg-surface-4"
                                >
                                    <CoffeeIcon class="size-5 text-[var(--color-text-tertiary)]" />
                                </span>
                            </template>
                            <span
                                class="truncate text-sm font-medium text-[var(--color-text-primary)]"
                            >
                                {{ detailOf(java) }}
                            </span>
                            <span
                                class="truncate text-xs text-[var(--color-text-tertiary)]"
                                :title="java.path"
                                >{{ java.path }}</span
                            >
                            <template #trailing>
                                <SpinnerIcon
                                    v-if="choosing === java.path"
                                    class="size-4 shrink-0 animate-spin"
                                />
                                <CheckIcon
                                    v-else-if="defaults.get(group.major) === java.path"
                                    class="size-5 shrink-0 text-brand"
                                />
                            </template>
                        </OobeChoice>
                    </div>
                </div>
                <p
                    v-if="scanning"
                    class="m-0 flex items-center gap-2 text-sm text-[var(--color-text-tertiary)]"
                >
                    <SpinnerIcon class="size-4 animate-spin" />
                    {{ formatMessage(messages.scanning) }}
                </p>
            </template>
        </section>

        <section class="flex flex-col gap-3">
            <div class="flex flex-col gap-1">
                <h2 class="m-0 text-base font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.addTitle) }}
                </h2>
                <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.addDescription) }}
                </p>
            </div>
            <form class="flex items-center gap-2" @submit.prevent="addManual">
                <StyledInput
                    v-model="manualPath"
                    autocomplete="off"
                    :placeholder="formatMessage(messages.pathPlaceholder)"
                    :disabled="adding"
                    wrapper-class="min-w-0 flex-1"
                />
                <Button :disabled="adding" @click="browse">
                    <FolderSearchIcon />
                    {{ formatMessage(messages.browse) }}
                </Button>
                <Button native-type="submit" :disabled="adding || !manualPath.trim()">
                    <SpinnerIcon v-if="adding" class="animate-spin" />
                    <PlusIcon v-else />
                    {{ formatMessage(messages.add) }}
                </Button>
            </form>
            <p
                v-if="addResult"
                class="m-0 flex items-center gap-1.5 text-sm"
                :class="
                    addResult.ok ? 'text-[var(--color-text-tertiary)]' : 'text-[var(--color-red)]'
                "
                role="status"
            >
                <CheckIcon v-if="addResult.ok" class="size-4 shrink-0 text-brand" />
                {{ addResult.message }}
            </p>
        </section>

        <section class="flex flex-col gap-3">
            <div class="flex flex-col gap-1">
                <h2 class="m-0 text-base font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.downloadTitle) }}
                </h2>
                <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.downloadDescription) }}
                </p>
            </div>
            <div
                v-for="download in downloads"
                :key="download.id"
                class="flex items-center gap-3 rounded-[var(--radius-lg)] bg-surface-3 px-4 py-3"
                role="status"
            >
                <DownloadIcon class="size-5 shrink-0 text-[var(--color-text-tertiary)]" />
                <span
                    class="min-w-0 flex-1 text-sm"
                    :class="
                        download.state === 'failed'
                            ? 'text-[var(--color-red)]'
                            : 'text-[var(--color-text-primary)]'
                    "
                >
                    {{
                        download.state === 'failed'
                            ? download.message
                            : formatMessage(
                                  download.state === 'done'
                                      ? messages.downloaded
                                      : messages.downloading,
                              )
                    }}
                </span>
                <SpinnerIcon
                    v-if="download.state === 'downloading'"
                    class="size-4 shrink-0 animate-spin"
                />
                <CheckIcon
                    v-else-if="download.state === 'done'"
                    class="size-5 shrink-0 text-brand"
                />
            </div>
            <div>
                <Button @click="downloadModal?.show()">
                    <DownloadIcon />
                    {{ formatMessage(messages.downloadJava) }}
                </Button>
            </div>
        </section>
    </div>
</template>

<style scoped lang="scss">
/* The app's warning tone, as the deep-scan notice in Settings uses it. */
.oobe-java-warning {
    background-color: color-mix(
        in srgb,
        var(--color-orange) var(--opacity-ratio-keep-10),
        transparent
    );
    border: 1px solid color-mix(in srgb, var(--color-orange) 40%, transparent);
}

.oobe-skeleton {
    background: linear-gradient(
        90deg,
        var(--surface-4) 0%,
        var(--surface-5) 50%,
        var(--surface-4) 100%
    );
    background-size: 200% 100%;
    animation: oobe-skeleton 1.4s ease-in-out infinite;
}

@keyframes oobe-skeleton {
    from {
        background-position: 100% 0;
    }
    to {
        background-position: -100% 0;
    }
}

@media (prefers-reduced-motion: reduce) {
    .oobe-skeleton {
        animation: none;
    }
}
</style>
