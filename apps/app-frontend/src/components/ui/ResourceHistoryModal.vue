<script setup lang="ts">
import { Button, defineMessages, NewModal, useVIntl } from '@modrinth/ui'
import { nextTick, onBeforeUnmount, ref } from 'vue'

import type { InstallJobSnapshot } from '@/helpers/install'
import { resourceOperationError, resourceSourceName } from '@/helpers/resource-operation-logic'
import {
    type ResourceHistoryEntry,
    resourceListHistory,
    resourceRollback,
} from '@/helpers/resources'

const { formatMessage } = useVIntl()
const messages = defineMessages({
    header: { id: 'app.resources.history.header', defaultMessage: 'Installation history' },
    restore: { id: 'app.resources.history.restore', defaultMessage: 'Restore this backup' },
    unavailable: {
        id: 'app.resources.history.unavailable',
        defaultMessage: 'Rollback backup unavailable',
    },
    loading: { id: 'app.resources.history.loading', defaultMessage: 'Checking rollback backups…' },
    empty: {
        id: 'app.resources.history.empty',
        defaultMessage: 'No rollback backups remain.',
    },
    retry: { id: 'app.resources.history.retry', defaultMessage: 'Retry' },
    cancel: { id: 'app.resources.history.cancel', defaultMessage: 'Close' },
})
const modal = ref<InstanceType<typeof NewModal> | null>(null)
const entries = ref<ResourceHistoryEntry[]>([])
const busy = ref(false)
const queueing = ref(false)
const error = ref<string | null>(null)
let generation = 0
let retryLoad: (() => Promise<void>) | null = null
let resolveShow: ((job: InstallJobSnapshot | null) => void) | null = null
function hide() {
    generation++
    resolveShow?.(null)
    resolveShow = null
}
function show(instanceId: string, entryId: string): Promise<InstallJobSnapshot | null> {
    hide()
    const current = generation
    queueing.value = false
    entries.value = []
    error.value = null
    busy.value = true
    modal.value?.show()
    const result = new Promise<InstallJobSnapshot | null>((resolve) => {
        resolveShow = resolve
    })
    retryLoad = async () => {
        busy.value = true
        error.value = null
        try {
            const value = await resourceListHistory(instanceId, entryId)
            if (current === generation)
                entries.value = value.filter(
                    (entry) => !entry.backupRetired && entry.backupRelativePath !== null,
                )
        } catch (failure) {
            if (current === generation) error.value = resourceOperationError(failure, formatMessage)
        } finally {
            if (current === generation) busy.value = false
        }
    }
    void retryLoad()
    return result
}
async function rollback(entry: ResourceHistoryEntry) {
    if (queueing.value) return
    queueing.value = true
    error.value = null
    const current = generation
    try {
        const job = await resourceRollback(entry.instanceId, entry.entryId, entry.id)
        if (current !== generation) return
        const resolve = resolveShow
        resolveShow = null
        queueing.value = false
        await nextTick()
        await modal.value?.hide()
        resolve?.(job)
    } catch (failure) {
        if (current === generation) error.value = resourceOperationError(failure, formatMessage)
    } finally {
        if (current === generation) queueing.value = false
    }
}
onBeforeUnmount(hide)
defineExpose({ show })
</script>

<template>
    <NewModal
        ref="modal"
        :header="formatMessage(messages.header)"
        :on-hide="hide"
        :disable-close="queueing"
        scrollable
        width="min(36rem, 90vw)"
    >
        <div class="flex flex-col gap-3">
            <p v-if="busy" class="m-0">{{ formatMessage(messages.loading) }}</p>
            <p v-if="error" class="m-0" role="alert">{{ error }}</p>
            <Button v-if="error && retryLoad" :disabled="busy || queueing" @click="retryLoad()">{{
                formatMessage(messages.retry)
            }}</Button>
            <p v-if="!busy && !error && !entries.length" class="m-0">
                {{ formatMessage(messages.empty) }}
            </p>
            <div
                v-for="entry in entries"
                :key="entry.id"
                class="flex flex-col gap-2 rounded-lg border border-surface-4 bg-surface-2 p-3"
            >
                <p class="m-0 break-all font-semibold">
                    {{ entry.previous.originalFileName ?? entry.previous.relativePath }}
                </p>
                <p class="m-0 text-sm">
                    {{ resourceSourceName(entry.previous.artifact?.release.project.source ?? '') }}
                    ·
                    {{ new Date(entry.recordedAt).toLocaleString() }}
                </p>
                <Button
                    v-if="entry.backupAvailable"
                    :disabled="queueing"
                    @click="rollback(entry)"
                    >{{ formatMessage(messages.restore) }}</Button
                >
                <p v-else class="m-0 text-sm">{{ formatMessage(messages.unavailable) }}</p>
            </div>
        </div>
        <template #actions
            ><Button :disabled="queueing" @click="modal?.hide()">{{
                formatMessage(messages.cancel)
            }}</Button></template
        >
    </NewModal>
</template>
