<script setup lang="ts">
import CheckIcon from '@modrinth/assets/icons/check.svg?component'
import PlusIcon from '@modrinth/assets/icons/plus.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Table from '@modrinth/ui/src/components/base/Table.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { ref } from 'vue'

import PreviewModal from '../../../../overlays/PreviewModal.vue'
import { javaDetectionMessages as messages } from '../../messages'
import { useJre } from './jre'
import { type JavaVersion, useSimulatedMachine } from './machine'

/** Port of `apps/app-frontend/src/components/ui/JavaDetectionModal.vue`. */
const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const machine = useSimulatedMachine()
const jre = useJre(machine)

const chosenInstallOptions = ref<JavaVersion[]>([])
const detectJavaModal = ref<InstanceType<typeof PreviewModal> | null>(null)
const currentSelected = ref<{ path: string; version?: string }>({ path: '', version: '' })
const javaInstallColumns = [
    { key: 'version', label: formatMessage(messages.version), width: '9rem' },
    { key: 'path', label: formatMessage(messages.path) },
    { key: 'actions', label: formatMessage(messages.actions), align: 'right', width: '10rem' },
]

defineExpose({
    show: async (
        version: number | null,
        currentSelectedJava: { path: string; version?: string } | null,
    ) => {
        chosenInstallOptions.value =
            (await jre.findFilteredJres(version ?? null).catch(handleError)) ?? []

        currentSelected.value = currentSelectedJava ?? { path: '', version: '' }

        detectJavaModal.value?.show()
    },
})

const emit = defineEmits<{ submit: [java: JavaVersion] }>()

function setJavaInstall(javaInstall: JavaVersion) {
    emit('submit', javaInstall)
    detectJavaModal.value?.hide()
}
</script>

<template>
    <PreviewModal ref="detectJavaModal" :header="formatMessage(messages.selectJavaVersion)">
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
                    <div class="p-4 text-[var(--color-text-tertiary)]">
                        {{ formatMessage(messages.noneFound) }}
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
