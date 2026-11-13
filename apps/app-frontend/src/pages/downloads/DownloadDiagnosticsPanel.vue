<script setup lang="ts">
import { InfoIcon } from '@modrinth/assets'
import { Button, Combobox, Table, type TableColumn, useFormatBytes, useVIntl } from '@modrinth/ui'
import { computed, ref, toRef } from 'vue'

import {
    isSlowTransfer,
    orderDiagnosticFiles,
    orderDiagnosticTransfers,
    useDownloadDiagnostics,
} from './download-diagnostics'
import {
    diagnosticMessages as messages,
    diagnosticMetricHints,
} from './download-diagnostics-messages'

const props = defineProps<{ jobId: string }>()
const { formatMessage } = useVIntl()
const formatBytes = useFormatBytes()
const { snapshot, selectedFileId, unavailable } = useDownloadDiagnostics(toRef(props, 'jobId'))
const sortColumn = ref<string>()
const sortDirection = ref<'asc' | 'desc'>('asc')
const fileOptions = computed(() =>
    orderDiagnosticFiles(snapshot.value?.files ?? []).map((file) => ({
        value: file.id,
        label: `${file.name} · #${file.id} · ${formatMessage(messages[file.status])}`,
    })),
)
const details = computed(() =>
    snapshot.value?.selected_file?.summary.id === selectedFileId.value
        ? snapshot.value.selected_file
        : undefined,
)
const selectedSummary = computed(() =>
    snapshot.value?.files.find((file) => file.id === selectedFileId.value),
)
const transfers = computed(() =>
    orderDiagnosticTransfers(details.value?.transfers ?? [], sortColumn.value, sortDirection.value),
)
const columns = computed<TableColumn[]>(() => [
    { key: 'remote_addr', label: formatMessage(messages.address), width: '30%' },
    { key: 'protocol', label: formatMessage(messages.protocol), width: '20%' },
    {
        key: 'speed_bytes_per_second',
        label: formatMessage(messages.speed),
        width: '19%',
        align: 'right',
        enableSorting: true,
    },
    {
        key: 'elapsed_ms',
        label: formatMessage(messages.duration),
        width: '15%',
        align: 'right',
        enableSorting: true,
        defaultSortDirection: 'desc',
    },
    { key: 'state', label: formatMessage(messages.state), width: '16%' },
])
function duration(milliseconds: number | null | undefined) {
    return milliseconds == null
        ? '—'
        : formatMessage(messages.seconds, { seconds: (milliseconds / 1000).toFixed(1) })
}
const metrics = computed(() => {
    const file = selectedSummary.value
    if (!file) return []
    const entries = [
        { key: 'elapsed', value: duration(file.elapsed_ms) },
        { key: 'networkElapsed', value: duration(file.network_elapsed_ms) },
        { key: 'received', value: formatBytes(file.received_bytes) },
        { key: 'activeRequests', value: file.active_requests },
        { key: 'effectiveRequests', value: file.effective_requests },
        { key: 'physicalConnections', value: file.physical_connections },
        { key: 'effectivePhysical', value: file.effective_physical_connections },
        { key: 'configuredLimit', value: file.configured_limit },
        { key: 'effectiveCap', value: file.effective_cap },
        { key: 'target', value: file.target_connections },
        { key: 'held', value: file.held_permits },
        { key: 'pool', value: snapshot.value?.pool_available },
    ] as const
    return entries.map(({ key, value }) => ({
        label: messages[key],
        hint: diagnosticMetricHints[key],
        value,
    }))
})
</script>

<template>
    <section class="mt-6 flex min-w-0 flex-col gap-3" data-download-diagnostics>
        <h3 class="m-0 text-base font-semibold">{{ formatMessage(messages.title) }}</h3>
        <p v-if="unavailable" class="m-0 text-sm text-[var(--color-red)]" role="status">
            {{ formatMessage(messages.unavailable) }}
        </p>
        <p v-else-if="!snapshot" class="m-0 text-sm text-[var(--color-text-tertiary)]">
            {{ formatMessage(messages.loading) }}
        </p>
        <p v-else-if="!snapshot.enabled" class="m-0 text-sm text-[var(--color-text-tertiary)]">
            {{ formatMessage(messages.disabled) }}
        </p>
        <template v-else-if="fileOptions.length">
            <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.fileSelectionGuide) }}
            </p>
            <div class="flex min-w-0 flex-wrap items-center gap-3">
                <label :for="`diagnostic-file-${jobId}`" class="font-semibold">{{
                    formatMessage(messages.file)
                }}</label>
                <div class="min-w-0 flex-1">
                    <Combobox
                        :id="`diagnostic-file-${jobId}`"
                        v-model="selectedFileId"
                        :options="fileOptions"
                        searchable
                        sync-with-selection
                    />
                </div>
            </div>
            <dl class="m-0 grid grid-cols-2 gap-3 text-sm lg:grid-cols-4">
                <div v-for="metric in metrics" :key="metric.label.id" class="min-w-0">
                    <dt class="flex items-center gap-1 text-[var(--color-text-tertiary)]">
                        <span>{{ formatMessage(metric.label) }}</span>
                        <span
                            v-tooltip="formatMessage(metric.hint)"
                            tabindex="0"
                            :aria-label="formatMessage(metric.hint)"
                            class="inline-flex shrink-0 cursor-help rounded-sm focus-visible:outline focus-visible:outline-2 focus-visible:outline-brand"
                        >
                            <InfoIcon class="size-3.5" aria-hidden="true" />
                        </span>
                    </dt>
                    <dd class="m-0 font-semibold tabular-nums">{{ metric.value ?? '—' }}</dd>
                </div>
            </dl>
            <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.note) }}
            </p>
            <Button
                type="quiet"
                class="self-start"
                :disabled="!sortColumn"
                @click="sortColumn = undefined"
                >{{ formatMessage(messages.priority) }}</Button
            >
            <Table
                v-model:sort-column="sortColumn"
                v-model:sort-direction="sortDirection"
                :columns="columns"
                :data="transfers"
                row-key="id"
                table-min-width="44rem"
                virtualized
                class="max-h-80 overflow-y-auto"
            >
                <template #cell-remote_addr="{ row }">
                    <div class="flex min-w-0 flex-col gap-1 py-2">
                        <span class="break-all font-mono text-xs">{{
                            row.remote_addr ?? '—'
                        }}</span>
                        <span v-if="row.proxied" class="text-xs">{{
                            formatMessage(messages.proxy)
                        }}</span>
                    </div>
                </template>
                <template #cell-protocol="{ row }">
                    <div class="flex flex-col gap-1 py-2">
                        <span>{{
                            row.protocol && row.protocol !== 'pending' ? row.protocol : '—'
                        }}</span>
                        <span v-if="row.physical_id != null" class="text-xs">{{
                            formatMessage(messages.socket, { id: row.physical_id })
                        }}</span>
                    </div>
                </template>
                <template #cell-speed_bytes_per_second="{ row }">
                    <span
                        class="tabular-nums"
                        :class="{ 'text-[var(--color-red)]': isSlowTransfer(row) }"
                        >{{
                            row.speed_bytes_per_second == null
                                ? '—'
                                : formatMessage(messages.rate, {
                                      speed: formatBytes(row.speed_bytes_per_second),
                                  })
                        }}</span
                    >
                </template>
                <template #cell-elapsed_ms="{ row }"
                    ><span class="tabular-nums">{{ duration(row.elapsed_ms) }}</span></template
                >
                <template #cell-state="{ row }">
                    <div
                        class="flex flex-col gap-1 py-2"
                        :class="{ 'text-[var(--color-red)]': isSlowTransfer(row) }"
                    >
                        <span>{{ formatMessage(messages[row.state]) }}</span>
                        <span v-if="isSlowTransfer(row)" class="text-xs">{{
                            formatMessage(messages.slow)
                        }}</span>
                    </div>
                </template>
                <template #empty-state
                    ><p class="m-0 p-4 text-sm text-[var(--color-text-tertiary)]">
                        {{ formatMessage(details ? messages.noRequests : messages.loading) }}
                    </p></template
                >
            </Table>
        </template>
        <p v-else class="m-0 text-sm text-[var(--color-text-tertiary)]">
            {{ formatMessage(messages.waiting) }}
        </p>
        <p
            v-if="snapshot && (snapshot.dropped_files || snapshot.dropped_transfers)"
            class="m-0 text-sm text-[var(--color-text-tertiary)]"
        >
            {{
                formatMessage(messages.truncated, {
                    files: snapshot.dropped_files,
                    transfers: snapshot.dropped_transfers,
                })
            }}
        </p>
    </section>
</template>
