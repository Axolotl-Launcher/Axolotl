<script setup lang="ts">
import PlusIcon from '@modrinth/assets/icons/plus.svg?component'
import TrashIcon from '@modrinth/assets/icons/trash.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { ref } from 'vue'

import PreviewModal from '../../../overlays/PreviewModal.vue'

/** Port of `SelectedProjectsLeaveModal` (packages/ui browse-tab components). */
const { formatMessage } = useVIntl()

const text = defineMessages({
    header: {
        id: 'browse.selected-projects-leave-modal.header',
        defaultMessage: 'Selected projects not installed yet',
    },
    admonitionBody: {
        id: 'browse.selected-projects-leave-modal.admonition-body',
        defaultMessage:
            'You have selected {count, plural, one {# project} other {# projects}} to install. Install them now or go back without installing them.',
    },
    discardButton: {
        id: 'browse.selected-projects-leave-modal.discard',
        defaultMessage: 'Discard',
    },
})

defineProps<{ count: number; installing?: boolean }>()

type SelectedProjectsLeaveResult = 'cancel' | 'discard' | 'install'

const modal = ref<InstanceType<typeof PreviewModal>>()
let resolvePromise: ((value: SelectedProjectsLeaveResult) => void) | null = null

function prompt(): Promise<SelectedProjectsLeaveResult> {
    return new Promise((resolvePrompt) => {
        resolvePromise = resolvePrompt
        modal.value?.show()
    })
}

function resolve(result: SelectedProjectsLeaveResult) {
    modal.value?.hide()
    resolvePromise?.(result)
    resolvePromise = null
}

defineExpose({ prompt })
</script>

<template>
    <PreviewModal ref="modal" fade="warning" :header="formatMessage(text.header)" max-width="560px">
        <div class="flex flex-col gap-6">
            {{ formatMessage(text.admonitionBody, { count }) }}
        </div>

        <template #actions>
            <div class="flex flex-wrap justify-end gap-2">
                <Button type="outlined" @click="resolve('cancel')"
                    ><XIcon />
                    {{ formatMessage(commonMessages.cancelButton) }}
                </Button>
                <Button
                    type="colored"
                    color="red"
                    :disabled="installing"
                    @click="resolve('discard')"
                    ><TrashIcon />
                    {{ formatMessage(text.discardButton) }}
                </Button>
                <Button
                    type="colored"
                    color="green"
                    :disabled="installing"
                    @click="resolve('install')"
                    ><PlusIcon />
                    {{ formatMessage(commonMessages.installButton) }}
                </Button>
            </div>
        </template>
    </PreviewModal>
</template>
