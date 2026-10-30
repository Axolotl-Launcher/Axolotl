<template>
    <NewModal ref="detectJavaModal" :header="formatMessage(messages.selectJavaVersion)">
        <div class="flex flex-col gap-4">
            <Table :columns="javaInstallColumns" :data="chosenInstallOptions" row-key="path">
                <template #cell-version="{ value }">
                    <span class="font-semibold text-[var(--color-text-default)]">{{ value }}</span>
                </template>
                <template #cell-path="{ value }">
                    <span v-tooltip="value" class="block truncate font-mono text-xs">{{
                        value
                    }}</span>
                </template>
                <template #cell-actions="{ row }">
                    <div class="flex items-center justify-end">
                        <Button
                            v-if="currentSelected.path === row.path"
                            class="!shadow-none"
                            disabled
                            ><CheckIcon /> {{ formatMessage(commonMessages.selectedLabel) }}
                        </Button>
                        <Button v-else class="!shadow-none" @click="setJavaInstall(row)"
                            ><PlusIcon /> {{ formatMessage(messages.select) }}
                        </Button>
                    </div>
                </template>
                <template #empty-state>
                    <div
                        v-if="loading"
                        class="flex items-center gap-2 p-4 text-[var(--color-text-tertiary)]"
                        role="status"
                    >
                        <SpinnerIcon class="size-4 animate-spin" />
                        {{ formatMessage(messages.searching) }}
                    </div>
                    <div v-else class="p-4 text-[var(--color-text-tertiary)]">
                        {{ formatMessage(messages.noneFound) }}
                    </div>
                </template>
            </Table>
            <div class="flex justify-end">
                <Button
                    type="outlined"
                    class="!shadow-none !border-surface-4 !border"
                    @click="$refs.detectJavaModal.hide()"
                    ><XIcon />
                    {{ formatMessage(commonMessages.cancelButton) }}
                </Button>
            </div>
        </div>
    </NewModal>
</template>
<script setup>
import { CheckIcon, PlusIcon, SpinnerIcon, XIcon } from '@modrinth/assets'
import {
    Button,
    commonMessages,
    defineMessages,
    injectNotificationManager,
    NewModal,
    Table,
    useVIntl,
} from '@modrinth/ui'
import { onUnmounted, ref } from 'vue'

import { createAnalysisTask } from '@/helpers/analysis-task'
import { trackEvent } from '@/helpers/analytics'
import { java_discovery_listener } from '@/helpers/events'
import { find_filtered_jres } from '@/helpers/jre.js'

const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const messages = defineMessages({
    selectJavaVersion: {
        id: 'app.java.select-version',
        defaultMessage: 'Select Java version',
    },
    select: { id: 'app.java.select', defaultMessage: 'Select' },
    noneFound: {
        id: 'app.java.none-found',
        defaultMessage: 'No Java installations found!',
    },
    searching: {
        id: 'app.java.searching',
        defaultMessage: 'Looking for Java installations…',
    },
    version: { id: 'app.java.table.version', defaultMessage: 'Version' },
    path: { id: 'app.java.table.path', defaultMessage: 'Path' },
    actions: { id: 'app.java.table.actions', defaultMessage: 'Actions' },
})

const chosenInstallOptions = ref([])
const detectJavaModal = ref(null)
const currentSelected = ref({})
const javaInstallColumns = [
    { key: 'version', label: formatMessage(messages.version), width: '9rem' },
    { key: 'path', label: formatMessage(messages.path) },
    { key: 'actions', label: formatMessage(messages.actions), align: 'right', width: '10rem' },
]

const lastRequestedVersion = ref(null)
let unlistenJavaDiscovery = null
let listening = null
let disposed = false
// Each opening owns its lookup; a slower lookup for an earlier version is abandoned.
const lookup = createAnalysisTask({
    label: 'java-detection',
    run: ({ version }) => find_filtered_jres(version, false, false),
})
const loading = lookup.running

defineExpose({
    show: async (version, currentSelectedJava) => {
        lastRequestedVersion.value = version ?? null
        currentSelected.value = currentSelectedJava || { path: '', version: '' }
        chosenInstallOptions.value = []
        detectJavaModal.value.show()
        listening ??= java_discovery_listener(refreshInstallOptions).then((unlisten) => {
            if (disposed) unlisten()
            else unlistenJavaDiscovery = unlisten
        })
        await loadInstallOptions(false)
    },
})

let refreshPending = false

/** Discovery refreshes keep the rows already shown and report no errors of their own. */
async function loadInstallOptions(quiet) {
    const input = { version: lastRequestedVersion.value }
    const found = await lookup.start(input)
    if (found) chosenInstallOptions.value = found
    else if (!quiet && lookup.input.value === input && lookup.status.value === 'error')
        handleError(lookup.error.value)
    if (refreshPending && lookup.input.value === input) {
        refreshPending = false
        await loadInstallOptions(true)
    }
}

/**
 * A discovery event during a lookup the user started waits for that lookup
 * instead of replacing it, so its result and error are not lost.
 */
function refreshInstallOptions() {
    if (lookup.running.value) {
        refreshPending = true
        return
    }
    return loadInstallOptions(true)
}

onUnmounted(() => {
    disposed = true
    lookup.dispose()
    if (unlistenJavaDiscovery) {
        unlistenJavaDiscovery()
        unlistenJavaDiscovery = null
    }
})

const emit = defineEmits(['submit'])

function setJavaInstall(javaInstall) {
    emit('submit', javaInstall)
    detectJavaModal.value.hide()
    trackEvent('JavaAutoDetect', {
        path: javaInstall.path,
        version: javaInstall.version,
    })
}
</script>
