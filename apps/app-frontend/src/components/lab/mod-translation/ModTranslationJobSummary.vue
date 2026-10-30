<script setup lang="ts">
import { CheckCircleIcon, FileArchiveIcon, FolderOpenIcon, XIcon } from '@modrinth/assets'
import { Button, defineMessages, useVIntl } from '@modrinth/ui'
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'

import type { ModTranslationJob } from '@/lab/mod-translation/types.ts'

const props = defineProps<{ job: ModTranslationJob }>()
const emit = defineEmits<{
    cancel: [taskId: string]
    remove: [taskId: string]
    openOutput: [job: ModTranslationJob]
}>()
const { formatMessage } = useVIntl()
const cancelling = ref(false)
const now = ref(Date.now())
let timer: ReturnType<typeof setInterval> | undefined

const messages = defineMessages({
    cancel: { id: 'app.lab.mod-translation.cancel', defaultMessage: 'Cancel' },
    cancelling: { id: 'app.lab.mod-translation.cancelling', defaultMessage: 'Cancelling…' },
    openOutput: { id: 'app.lab.mod-translation.open-output', defaultMessage: 'Open output folder' },
    removeTask: { id: 'app.lab.mod-translation.remove-task', defaultMessage: 'Remove task' },
    done: { id: 'app.lab.mod-translation.done', defaultMessage: 'Done' },
    failed: { id: 'app.lab.mod-translation.failed', defaultMessage: 'Failed' },
})

const fileName = computed(() => props.job.inputPath.split(/[\\/]/).pop() || props.job.inputPath)
const elapsed = computed(() => {
    const startedAt = Date.parse(props.job.startedAt)
    if (!Number.isFinite(startedAt)) return '00:00'
    const endedAt = props.job.status === 'running' ? now.value : Date.parse(props.job.updatedAt)
    return formatDuration(Math.floor((Math.max(startedAt, endedAt) - startedAt) / 1000))
})
const badgeClass = computed(() => {
    if (props.job.status === 'completed') return 'file-badge--ok'
    if (props.job.status === 'failed') return 'file-badge--fail'
    return `file-badge--${props.job.level}`
})

onMounted(() => {
    timer = setInterval(() => {
        now.value = Date.now()
    }, 1000)
})

onBeforeUnmount(() => {
    if (timer) clearInterval(timer)
})

watch(
    () => props.job.status,
    (status) => {
        if (status !== 'running') cancelling.value = false
    },
)

function cancel() {
    if (cancelling.value) return
    cancelling.value = true
    emit('cancel', props.job.taskId)
}

function formatDuration(seconds: number): string {
    const safe = Math.max(0, Math.round(seconds))
    const hours = Math.floor(safe / 3600)
    const minutes = Math.floor((safe % 3600) / 60)
    const rest = safe % 60
    const mm = String(minutes).padStart(2, '0')
    const ss = String(rest).padStart(2, '0')
    return hours > 0 ? `${hours}:${mm}:${ss}` : `${mm}:${ss}`
}
</script>

<template>
    <header class="job-header">
        <span class="file-badge" :class="badgeClass"><FileArchiveIcon /></span>
        <div class="job-title">
            <strong :title="job.inputPath">{{ fileName }}</strong>
            <span>{{ elapsed }}</span>
        </div>
        <span v-if="job.status === 'running'" class="job-percent">{{ job.percent }}%</span>
        <span
            v-else
            class="job-status inline-flex items-center gap-[0.3rem] text-[0.72rem] font-extrabold text-green"
            :class="{ 'text-red': job.status === 'failed' }"
        >
            <CheckCircleIcon v-if="job.status === 'completed'" />
            <XIcon v-else />
            {{ formatMessage(job.status === 'completed' ? messages.done : messages.failed) }}
        </span>
        <div class="actions">
            <Button
                v-if="job.status === 'running'"
                type="outlined"
                color="red"
                size="2xs"
                :disabled="cancelling"
                @click="cancel"
                >{{ formatMessage(cancelling ? messages.cancelling : messages.cancel) }}
            </Button>
            <Button
                v-else-if="job.status === 'completed'"
                type="colored"
                color="brand"
                size="2xs"
                @click="emit('openOutput', job)"
                ><FolderOpenIcon />{{ formatMessage(messages.openOutput) }}
            </Button>
            <Button
                v-if="job.status !== 'running'"
                type="outlined"
                size="2xs"
                :aria-label="formatMessage(messages.removeTask)"
                @click="emit('remove', job.taskId)"
                ><XIcon
            /></Button>
        </div>
    </header>
</template>

<style scoped>
.job-header {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 0.65rem;
}

.file-badge {
    display: inline-flex;
    width: 2.1rem;
    height: 2.1rem;
    flex: 0 0 auto;
    align-items: center;
    justify-content: center;
    border-radius: var(--radius-md);
    background: var(--color-brand-highlight);
    color: var(--color-brand);
    transition:
        background 0.3s ease,
        color 0.3s ease;
}

.file-badge :deep(svg) {
    width: 1.05rem;
    height: 1.05rem;
}

.file-badge--warn {
    background: color-mix(in srgb, var(--color-orange) 16%, transparent);
    color: var(--color-orange);
}

.file-badge--error,
.file-badge--fail {
    background: color-mix(in srgb, var(--color-red) 16%, transparent);
    color: var(--color-red);
}

.file-badge--ok {
    background: color-mix(in srgb, var(--color-green) 16%, transparent);
    color: var(--color-green);
}

.job-title {
    display: flex;
    min-width: 0;
    flex: 1;
    flex-direction: column;
    gap: 0.1rem;
}

.job-title strong {
    overflow: hidden;
    font-size: 0.9rem;
    font-weight: 800;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--color-text-primary);
}

.job-title span {
    font-family: var(--mono-font, monospace);
    font-size: 0.66rem;
    font-variant-numeric: tabular-nums;
    color: var(--color-text-tertiary);
}

.job-percent {
    font-size: 1rem;
    font-weight: 800;
    font-variant-numeric: tabular-nums;
    color: var(--color-brand);
}

.job-status :deep(svg),
.actions :deep(svg) {
    width: 0.9rem;
    height: 0.9rem;
}

.actions {
    display: flex;
    flex: 0 0 auto;
    align-items: center;
    gap: 0.35rem;
}

@media (max-width: 44rem) {
    .actions :deep(button) {
        font-size: 0;
    }

    .actions :deep(button svg) {
        font-size: initial;
    }
}
</style>
