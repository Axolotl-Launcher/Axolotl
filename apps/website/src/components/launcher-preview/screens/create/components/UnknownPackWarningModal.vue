<script setup lang="ts">
import CircleArrowRightIcon from '@modrinth/assets/icons/circle-arrow-right.svg?component'
import SpinnerIcon from '@modrinth/assets/icons/spinner.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Admonition from '@modrinth/ui/src/components/base/Admonition.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Checkbox from '@modrinth/ui/src/components/base/Checkbox.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { ref, useTemplateRef } from 'vue'

import PreviewModal from '../../../overlays/PreviewModal.vue'
import { unknownPackMessages as messages } from '../messages'

/**
 * Port of `components/ui/install_flow/UnknownPackWarningModal.vue`: shown before a
 * modpack archive that is not on Modrinth is installed. "Don't show this warning
 * again" sets the `skip_unknown_pack_warning` feature flag for the session.
 */
const props = defineProps<{ skipWarning: boolean }>()
const emit = defineEmits<{ 'update:skipWarning': [value: boolean]; closed: [] }>()

const { formatMessage } = useVIntl()
const modal = useTemplateRef<InstanceType<typeof PreviewModal>>('modal')
const dontShowAgain = ref(false)
const onProceed = ref<(() => Promise<void>) | undefined>()
const isProceeding = ref(false)
const fileName = ref('')

function show(createInstance: () => Promise<void>, selectedFileName = '') {
    onProceed.value = createInstance
    fileName.value = selectedFileName
    dontShowAgain.value = false
    if (props.skipWarning) {
        void createInstance()
        return
    }
    modal.value?.show()
}

function reset() {
    onProceed.value = undefined
    fileName.value = ''
    emit('closed')
}

function cancel() {
    modal.value?.hide()
}

function proceed() {
    if (!onProceed.value) return
    if (dontShowAgain.value) emit('update:skipWarning', true)
    const createInstance = onProceed.value
    modal.value?.hide()
    void createInstance()
}

defineExpose({ show })
</script>

<template>
    <PreviewModal ref="modal" :header="formatMessage(messages.header)" :on-hide="reset">
        <div class="flex max-w-[31rem] flex-col gap-6">
            <Admonition
                type="warning"
                :header="formatMessage(messages.warningTitle)"
                :body="formatMessage(messages.warningBody)"
            />
            <div
                v-if="fileName"
                class="overflow-x-auto whitespace-nowrap text-sm text-[var(--color-text-tertiary)]"
            >
                {{ fileName }}
            </div>
            <div>
                <p class="mt-0 leading-tight">
                    {{ formatMessage(messages.body) }}
                </p>
                <p class="mb-0 font-semibold leading-tight text-orange">
                    {{ formatMessage(messages.malwareStatement) }}
                </p>
            </div>
            <Checkbox v-model="dontShowAgain" :label="formatMessage(messages.dontShowAgain)" />
            <div class="flex justify-end gap-2">
                <Button type="outlined" @click="cancel"
                    ><XIcon />
                    {{ formatMessage(commonMessages.cancelButton) }}
                </Button>
                <Button type="colored" color="orange" :disabled="isProceeding" @click="proceed"
                    ><SpinnerIcon v-if="isProceeding" class="animate-spin" />
                    <CircleArrowRightIcon v-else />
                    {{ formatMessage(messages.installAnyway) }}
                </Button>
            </div>
        </div>
    </PreviewModal>
</template>
