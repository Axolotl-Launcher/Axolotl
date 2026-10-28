<script setup lang="ts">
import { FileArchiveIcon, PlayIcon, SpinnerIcon } from '@modrinth/assets'
import { Admonition, Button, injectNotificationManager, useVIntl } from '@modrinth/ui'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { storeToRefs } from 'pinia'
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'

import ModTranslationFilePicker from '@/components/lab/mod-translation/ModTranslationFilePicker.vue'
import ModTranslationJobList from '@/components/lab/mod-translation/ModTranslationJobList.vue'
import ModTranslationSettingsPanel from '@/components/lab/mod-translation/ModTranslationSettingsPanel.vue'
import { createAnalysisTask } from '@/helpers/analysis-task'
import { analyzeMod } from '@/lab/mod-translation/backend'
import { modTranslationMessages as messages } from '@/lab/mod-translation/i18n'
import type { ModTranslationJob } from '@/lab/mod-translation/types.ts'
import { useModTranslationStore } from '@/store/modTranslation'

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()

const store = useModTranslationStore()
const { inputPath, analysis, providerId, modelId, options, jobs } = storeToRefs(store)
// The file being analyzed is fixed when analysis starts. Picking another file
// or leaving the page abandons that analysis, including its error.
const modAnalysis = createAnalysisTask({
    label: 'mod-translation-analysis',
    run: ({ path }: { path: string }, { signal, stopping }) => analyzeMod(path, signal, stopping),
})
const analyzing = modAnalysis.running
const starting = ref(false)

const analyzeStartedAt = ref<number | null>(null)
const analyzeNow = ref(Date.now())
let analyzeTimer: ReturnType<typeof setInterval> | undefined

watch(analyzing, (value) => {
    if (value) {
        analyzeStartedAt.value = Date.now()
        analyzeNow.value = Date.now()
        analyzeTimer = setInterval(() => {
            analyzeNow.value = Date.now()
        }, 1000)
    } else if (analyzeTimer) {
        clearInterval(analyzeTimer)
        analyzeTimer = undefined
        analyzeStartedAt.value = null
    }
})

onBeforeUnmount(() => {
    modAnalysis.dispose()
    if (analyzeTimer) clearInterval(analyzeTimer)
})

const analyzeElapsed = computed(() => {
    if (analyzeStartedAt.value === null) return 0
    return Math.max(0, Math.floor((analyzeNow.value - analyzeStartedAt.value) / 1000))
})

const canStart = computed(
    () =>
        !!inputPath.value &&
        !!providerId.value &&
        !!modelId.value &&
        !starting.value &&
        !analyzing.value,
)
const startHint = computed(() => {
    if (!inputPath.value) return formatMessage(messages.startHint)
    if (!providerId.value || !modelId.value) return formatMessage(messages.aiNotConfigured)
    return formatMessage(messages.outputPath, { path: defaultOutputPath(inputPath.value) })
})

const summaryStats = computed(() => {
    if (!analysis.value) return []
    return [
        {
            label: formatMessage(messages.loader),
            value: analysis.value.loader,
        },
        {
            label: formatMessage(messages.languageEntries),
            value: String(analysis.value.languageEntries),
        },
        {
            label: formatMessage(messages.languageCharacters),
            value: String(analysis.value.languageCharacters),
        },
        {
            label: formatMessage(messages.classCandidates),
            value: String(analysis.value.classCandidates.length),
        },
        {
            label: formatMessage(messages.estimatedQuote),
            value: formatMessage(messages.estimatedTokens, {
                tokens: (analysis.value.quote?.estimatedTokens ?? 0).toLocaleString(),
            }),
            detail: formatMessage(messages.estimatedTokensDetail, {
                calls: analysis.value.quote?.estimatedCalls ?? 0,
                input: (analysis.value.quote?.estimatedInputTokens ?? 0).toLocaleString(),
                output: (analysis.value.quote?.estimatedOutputTokens ?? 0).toLocaleString(),
            }),
            full: true,
        },
    ]
})

function defaultOutputPath(path: string): string {
    return path.replace(/\.jar$/i, '-zh_cn.jar')
}

async function runAnalyze() {
    const path = inputPath.value
    if (!path) return
    store.setAnalysis(null)
    const input = { path }
    const result = await modAnalysis.start(input)
    if (result) store.setAnalysis(result, path)
    else if (modAnalysis.input.value === input && modAnalysis.status.value === 'error')
        handleError(new Error(errorMessage(modAnalysis.error.value)))
}

watch(inputPath, (path) => {
    if (modAnalysis.running.value) modAnalysis.reset()
    if (store.analysisInputPath !== path) store.setAnalysis(null)
})

async function startTranslation() {
    if (!canStart.value) return
    starting.value = true
    try {
        await store.startTranslation()
    } catch (error) {
        handleError(new Error(errorMessage(error)))
    } finally {
        starting.value = false
    }
}

async function cancelJob(taskId: string) {
    try {
        await store.cancelJob(taskId)
    } catch (error) {
        handleError(new Error(errorMessage(error)))
    }
}

async function removeJob(taskId: string) {
    try {
        await store.removeJob(taskId)
    } catch (error) {
        handleError(new Error(errorMessage(error)))
    }
}

function openOutput(job: ModTranslationJob) {
    void revealItemInDir(job.outputPath).catch((error) => {
        handleError(new Error(String(error)))
    })
}

function errorMessage(error: unknown): string {
    if (error instanceof Error) return error.message
    if (typeof error === 'string') return error
    if (
        typeof error === 'object' &&
        error !== null &&
        'message' in error &&
        typeof error.message === 'string'
    ) {
        return error.message
    }
    return String(error)
}

onMounted(() => {
    void store.init().catch((error) => {
        handleError(new Error(errorMessage(error)))
    })
})
</script>

<template>
    <main class="mod-translation-page flex min-h-0 flex-col gap-4 p-6">
        <header class="flex min-w-0 items-start justify-between gap-4">
            <div class="min-w-0">
                <h1 class="m-0 text-2xl font-bold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.title) }}
                </h1>
                <p class="m-0 mt-1 text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.description) }}
                </p>
            </div>
            <div
                v-if="store.activeJobs.length"
                class="background-badge"
                :title="formatMessage(messages.backgroundRunning)"
            >
                <span class="live-dot" />
                <span>{{ formatMessage(messages.backgroundRunning) }}</span>
            </div>
        </header>

        <div class="mod-translation-grid">
            <!-- 左栏：输入与设置 -->
            <div class="flex min-w-0 flex-col gap-4">
                <section class="panel">
                    <h2 class="panel-title">{{ formatMessage(messages.inputSection) }}</h2>
                    <ModTranslationFilePicker v-model:path="inputPath" />
                    <div class="flex items-center gap-2">
                        <Button
                            type="colored"
                            color="brand"
                            :disabled="!inputPath || analyzing"
                            @click="runAnalyze"
                            ><SpinnerIcon v-if="analyzing" class="animate-spin" />
                            <FileArchiveIcon v-else />
                            {{ formatMessage(analyzing ? messages.analyzing : messages.analyze) }}
                        </Button>
                        <span v-if="!inputPath" class="panel-hint">
                            {{ formatMessage(messages.selectFile) }}…
                        </span>
                    </div>
                    <div v-if="analyzing" class="analyze-status flex items-center gap-2">
                        <span class="analyze-timer">
                            {{
                                formatMessage(messages.analyzingElapsed, {
                                    seconds: analyzeElapsed,
                                })
                            }}
                        </span>
                        <span class="panel-hint">{{ formatMessage(messages.analyzingHint) }}</span>
                    </div>
                </section>

                <section v-if="analysis" class="panel">
                    <h2 class="panel-title">{{ formatMessage(messages.analysis) }}</h2>
                    <div class="stats-grid">
                        <div
                            v-for="item in summaryStats"
                            :key="item.label"
                            class="stat"
                            :class="{ 'stat-full': item.full }"
                        >
                            <span class="stat-label">{{ item.label }}</span>
                            <span class="stat-value">{{ item.value }}</span>
                            <span v-if="item.detail" class="stat-detail">{{ item.detail }}</span>
                        </div>
                    </div>
                    <Admonition v-if="analysis.signed" type="warning">
                        {{ formatMessage(messages.signedMod) }}
                    </Admonition>
                    <Admonition v-for="warning in analysis.warnings" :key="warning" type="warning">
                        {{ warning }}
                    </Admonition>
                    <div v-if="analysis.languageSources.length" class="flex flex-col gap-1.5">
                        <h3 class="m-0 text-sm font-semibold text-[var(--color-text-primary)]">
                            {{ formatMessage(messages.languageSources) }}
                        </h3>
                        <div class="source-list">
                            <div
                                v-for="source in analysis.languageSources"
                                :key="source.sourcePath"
                                class="source-row"
                            >
                                <span class="source-path" :title="source.sourcePath">
                                    {{ source.sourcePath }}
                                </span>
                                <span class="source-count"
                                    >{{ source.required }} / {{ source.entries }}</span
                                >
                            </div>
                        </div>
                    </div>
                </section>

                <section class="panel">
                    <h2 class="panel-title">{{ formatMessage(messages.aiSection) }}</h2>
                    <ModTranslationSettingsPanel
                        v-model="options"
                        v-model:provider-id="providerId"
                        v-model:model-id="modelId"
                    />
                    <div class="flex flex-col gap-1.5">
                        <Button
                            type="colored"
                            color="brand"
                            class="start-button"
                            :disabled="!canStart"
                            @click="startTranslation"
                            ><PlayIcon />{{ formatMessage(messages.start) }}
                        </Button>
                        <span class="panel-hint">{{ startHint }}</span>
                    </div>
                </section>
            </div>

            <!-- 右栏：任务 -->
            <div class="flex min-w-0 flex-col">
                <section class="panel jobs-panel flex-1">
                    <h2 class="panel-title">{{ formatMessage(messages.jobsSection) }}</h2>
                    <ModTranslationJobList
                        :jobs="jobs"
                        @cancel="cancelJob"
                        @remove="removeJob"
                        @open-output="openOutput"
                    />
                </section>
            </div>
        </div>
    </main>
</template>

<style scoped>
.mod-translation-grid {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 1rem;
    align-items: start;
}

@media (min-width: 1100px) {
    .mod-translation-grid {
        grid-template-columns: minmax(0, 5fr) minmax(0, 7fr);
    }
}

.panel {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    border: 1px solid var(--surface-5);
    border-radius: var(--radius-md);
    background: var(--surface-2);
    padding: 1rem;
}

.panel-title {
    margin: 0;
    font-size: 0.72rem;
    font-weight: 800;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--color-text-tertiary);
}

.panel-hint {
    font-size: 0.72rem;
    color: var(--color-text-tertiary);
}

.analyze-timer {
    font-size: 0.72rem;
    font-variant-numeric: tabular-nums;
    font-weight: 700;
    color: var(--color-brand);
}

.start-button {
    display: inline-flex;
    width: 100%;
    align-items: center;
    justify-content: center;
    gap: 0.4rem;
    white-space: nowrap;
}

.background-badge {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    border: 1px solid var(--surface-5);
    border-radius: 999px;
    background: var(--surface-2);
    padding: 0.3rem 0.65rem;
    font-size: 0.7rem;
    font-weight: 700;
    color: var(--color-text-tertiary);
    white-space: nowrap;
}

.live-dot {
    width: 0.5rem;
    height: 0.5rem;
    border-radius: 999px;
    background: var(--color-brand);
    animation: page-pulse 1.6s ease-out infinite;
}

@keyframes page-pulse {
    0% {
        box-shadow: 0 0 0 0 color-mix(in srgb, var(--color-brand) 50%, transparent);
    }

    70%,
    100% {
        box-shadow: 0 0 0 0.5rem transparent;
    }
}

.stats-grid {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 0.5rem;
}

@media (max-width: 640px) {
    .stats-grid {
        grid-template-columns: repeat(2, minmax(0, 1fr));
    }
}

.stat {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    border: 1px solid var(--surface-5);
    border-radius: var(--radius-sm);
    background: var(--surface-3);
    padding: 0.5rem 0.65rem;
}

.stat-label {
    color: var(--color-text-tertiary);
    font-size: 0.65rem;
    font-weight: 700;
}

.stat-value {
    overflow: hidden;
    color: var(--color-text-primary);
    font-size: 0.9rem;
    font-weight: 800;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.stat-detail {
    color: var(--color-text-tertiary);
    font-size: 0.62rem;
    font-variant-numeric: tabular-nums;
}

.stat-full {
    display: flex;
    grid-column: 1 / -1;
    flex-direction: row;
    align-items: baseline;
    gap: 0.75rem;
}

.stat-full .stat-label {
    flex: 0 0 auto;
}

.stat-full .stat-value {
    flex: 0 0 auto;
    font-size: 1rem;
}

.stat-full .stat-detail {
    margin-left: auto;
}

.source-list {
    display: flex;
    max-height: 14rem;
    flex-direction: column;
    gap: 0.3rem;
    overflow-y: auto;
    padding-right: 0.2rem;
}

.source-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    border: 1px solid var(--surface-5);
    border-radius: var(--radius-sm);
    background: var(--surface-3);
    padding: 0.35rem 0.55rem;
}

.source-path {
    min-width: 0;
    overflow: hidden;
    flex: 1;
    font-size: 0.75rem;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.source-count {
    flex: 0 0 auto;
    font-size: 0.7rem;
    font-variant-numeric: tabular-nums;
    color: var(--color-text-tertiary);
}
</style>
