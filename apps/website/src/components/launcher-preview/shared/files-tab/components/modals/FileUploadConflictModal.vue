<template>
    <NewModal ref="modal" :header="formatMessage(messages.header)" :closable="true" no-padding>
        <div class="max-w-[500px]">
            <div class="flex flex-col gap-4 p-4">
                <Admonition type="warning" :header="formatMessage(messages.warningHeader)">
                    <span>
                        <template v-if="hasMany">
                            {{ formatMessage(messages.overwriteManyWarning) }}
                        </template>
                        <template v-else>
                            {{ formatMessage(messages.overwriteWarning, { count: files.length }) }}
                        </template>
                    </span>
                </Admonition>

                <div v-if="files.length" class="flex gap-2">
                    <div class="flex items-center gap-1">
                        <MinusIcon />
                        {{ formatMessage(messages.overwrittenCount, { count: files.length }) }}
                    </div>
                </div>
            </div>

            <div
                v-if="files.length"
                class="flex max-h-[272px] flex-col overflow-y-auto border-b border-l-0 border-r-0 border-t border-solid border-surface-5 bg-surface-2 p-4"
            >
                <div
                    v-for="(file, index) in files"
                    :key="file"
                    class="grid h-10 min-h-10 grid-cols-[auto_minmax(0,1fr)] items-center gap-2"
                >
                    <div class="flex flex-col items-center justify-between">
                        <div class="h-2 w-[1px]"></div>
                        <MinusIcon class="text-red" />
                        <div
                            :class="index === files.length - 1 ? 'bg-transparent' : 'bg-surface-5'"
                            class="relative top-1 h-2 w-[1px]"
                        ></div>
                    </div>
                    <span class="flex min-w-0 overflow-hidden whitespace-nowrap text-sm">
                        <IntlFormatted
                            :message-id="messages.overwriteFileLabel"
                            :values="{ path: file }"
                        >
                            <template #file-path="{ children }">
                                <span
                                    v-tooltip="file"
                                    class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap font-medium text-[var(--color-text-primary)]"
                                >
                                    <component :is="() => children" />
                                </span>
                            </template>
                        </IntlFormatted>
                    </span>
                </div>
            </div>
        </div>

        <template #actions>
            <div class="flex justify-end gap-2 pt-4">
                <Button type="outlined" @click="hide"
                    ><XIcon />
                    {{ formatMessage(commonMessages.cancelButton) }}
                </Button>
                <Button type="colored" color="brand" @click="handleProceed"
                    ><CheckIcon />
                    {{ formatMessage(messages.overwriteButton) }}
                </Button>
            </div>
        </template>
    </NewModal>
</template>

<script setup lang="ts">
import CheckIcon from '@modrinth/assets/icons/check.svg?component'
import MinusIcon from '@modrinth/assets/icons/minus.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Admonition from '@modrinth/ui/src/components/base/Admonition.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import IntlFormatted from '@modrinth/ui/src/components/base/IntlFormatted.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, ref } from 'vue'

import NewModal from '../../../../overlays/PreviewModal.vue'

const { formatMessage } = useVIntl()

const messages = defineMessages({
    header: {
        id: 'files.conflict-modal.header',
        defaultMessage: 'Extract summary',
    },
    warningHeader: {
        id: 'files.conflict-modal.warning-header',
        defaultMessage: 'Files will be overwritten',
    },
    overwriteManyWarning: {
        id: 'files.conflict-modal.overwrite-many-warning',
        defaultMessage:
            'Over 100 files will be overwritten if you proceed with extraction; here are some of them.',
    },
    overwriteWarning: {
        id: 'files.conflict-modal.overwrite-warning',
        defaultMessage:
            'The following {count} files already exist on your server, and will be overwritten if you proceed with extraction.',
    },
    overwrittenCount: {
        id: 'files.conflict-modal.overwritten-count',
        defaultMessage: '{count} overwritten',
    },
    overwriteFileLabel: {
        id: 'files.conflict-modal.overwrite-file-label',
        defaultMessage: 'Will overwrite <file-path>{path}</file-path>',
    },
    overwriteButton: {
        id: 'files.conflict-modal.overwrite-button',
        defaultMessage: 'Overwrite',
    },
})

const path = ref('')
const files = ref<string[]>([])

const emit = defineEmits<{
    proceed: [path: string]
}>()

const modal = ref<InstanceType<typeof NewModal>>()

const hasMany = computed(() => files.value.length > 100)

const show = (zipPath: string, conflictingFiles: string[]) => {
    path.value = zipPath
    files.value = conflictingFiles
    modal.value?.show()
}

const hide = () => {
    modal.value?.hide()
}

const handleProceed = () => {
    hide()
    emit('proceed', path.value)
}

defineExpose({ show })
</script>
