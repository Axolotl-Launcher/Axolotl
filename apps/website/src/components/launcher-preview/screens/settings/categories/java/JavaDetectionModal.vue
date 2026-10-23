<script setup lang="ts">
import CheckIcon from '@modrinth/assets/icons/check.svg?component'
import PlusIcon from '@modrinth/assets/icons/plus.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Table from '@modrinth/ui/src/components/base/Table.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { ref, watch } from 'vue'

import PreviewModal from '../../../../overlays/PreviewModal.vue'
import { messages } from '../../messages'
import { type JavaVersion, useJre } from './jre'

/**
 * The "Detect" dialog of a Java selector (apps/app-frontend/src/components/ui/
 * JavaDetectionModal.vue): the known installations of one major version, served from
 * the discovery cache, refreshed when a background rescan finds a change.
 */
const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const jre = useJre()

const chosenInstallOptions = ref<JavaVersion[]>([])
const detectJavaModal = ref<InstanceType<typeof PreviewModal> | null>(null)
const currentSelected = ref<Partial<JavaVersion>>({})
const javaInstallColumns = [
    { key: 'version', label: formatMessage(messages.appJavaTableVersion), width: '9rem' },
    { key: 'path', label: formatMessage(messages.appJavaTablePath) },
    {
        key: 'actions',
        label: formatMessage(messages.appJavaTableActions),
        align: 'right' as const,
        width: '10rem',
    },
]

const lastRequestedVersion = ref<number | null>(null)
let listening = false

async function show(version: number | null, currentSelectedJava?: Partial<JavaVersion> | null) {
    lastRequestedVersion.value = version ?? null
    chosenInstallOptions.value =
        (await jre.findFilteredJres(version, false, false).catch(handleError)) ?? []

    currentSelected.value = currentSelectedJava ?? { path: '', version: '' }
    if (!currentSelected.value) {
        currentSelected.value = { path: '', version: '' }
    }

    // `java_discovery_listener`: subscribed on first open, for the component's life.
    listening = true

    detectJavaModal.value?.show()
}

defineExpose({ show })

async function refreshInstallOptions() {
    const updated = await jre
        .findFilteredJres(lastRequestedVersion.value, false, false)
        .catch(() => null)
    if (updated) {
        chosenInstallOptions.value = updated
    }
}

watch(
    () => jre.data.discoveryEvents,
    () => {
        if (listening) void refreshInstallOptions()
    },
)

const emit = defineEmits<{ submit: [JavaVersion] }>()

function setJavaInstall(javaInstall: JavaVersion) {
    emit('submit', javaInstall)
    detectJavaModal.value?.hide()
}
</script>

<template>
    <PreviewModal ref="detectJavaModal" :header="formatMessage(messages.appJavaSelectVersion)">
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
                        <Button
                            v-else
                            class="!shadow-none"
                            @click="setJavaInstall(row as JavaVersion)"
                            ><PlusIcon /> {{ formatMessage(messages.appJavaSelect) }}
                        </Button>
                    </div>
                </template>
                <template #empty-state>
                    <div class="p-4 text-[var(--color-text-tertiary)]">
                        {{ formatMessage(messages.appJavaNoneFound) }}
                    </div>
                </template>
            </Table>
            <div class="flex justify-end">
                <Button
                    type="outlined"
                    class="!border !border-surface-4 !shadow-none"
                    @click="detectJavaModal?.hide()"
                    ><XIcon />
                    {{ formatMessage(commonMessages.cancelButton) }}
                </Button>
            </div>
        </div>
    </PreviewModal>
</template>
