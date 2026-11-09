<template>
    <NewModal ref="modal" :header="formatMessage(messages.header)" :closable="!loading" no-padding>
        <div class="flex max-w-[500px] flex-col gap-6 p-6">
            <Admonition
                :type="variant === 'loader-change' ? 'critical' : 'warning'"
                :header="
                    variant === 'loader-change'
                        ? formatMessage(messages.loaderChangeTitle)
                        : formatMessage(messages.gameVersionWarningTitle)
                "
            >
                <div class="flex flex-col gap-3">
                    <span>
                        {{
                            variant === 'loader-change'
                                ? formatMessage(messages.loaderChangeBody)
                                : formatMessage(messages.gameVersionWarningBody)
                        }}
                    </span>
                </div>
            </Admonition>
        </div>

        <template #actions>
            <div class="flex justify-end gap-2">
                <Button :disabled="loading" @click="handleCancel"
                    ><XIcon />
                    {{ formatMessage(commonMessages.cancelButton) }}
                </Button>
                <template v-if="variant === 'game-version-change'">
                    <Button :disabled="loading" @click="handleDisableConflicts"
                        ><SpinnerIcon
                            v-if="loading && loadingAction === 'disable-conflicts'"
                            class="size-5 animate-spin"
                        />
                        <PowerOffIcon v-else class="size-5" />
                        {{ formatMessage(messages.disableConflictsButton) }}
                    </Button>
                    <Button type="colored" color="orange" :disabled="loading" @click="handleAutoFix"
                        ><SpinnerIcon
                            v-if="loading && loadingAction === 'auto-fix'"
                            class="size-5 animate-spin"
                        />
                        <HammerIcon v-else class="size-5" />
                        {{ formatMessage(messages.autoFixButton) }}
                    </Button>
                </template>
                <template v-else>
                    <Button
                        type="colored"
                        color="red"
                        :disabled="loading"
                        @click="handleConfirmLoaderChange"
                        ><SpinnerIcon v-if="loading" class="size-5 animate-spin" />
                        <CircleAlertIcon v-else class="size-5" />
                        {{ formatMessage(messages.changeLoaderButton) }}
                    </Button>
                </template>
            </div>
        </template>
    </NewModal>
</template>

<script setup lang="ts">
import { CircleAlertIcon, HammerIcon, PowerOffIcon, SpinnerIcon, XIcon } from '@modrinth/assets'
import { ref } from 'vue'

import Admonition from '#ui/components/base/Admonition.vue'
import Button from '#ui/components/base/buttons/Button.vue'
import NewModal from '#ui/components/modal/NewModal.vue'
import { defineMessages, useVIntl } from '#ui/composables/i18n'
import { commonMessages } from '#ui/utils/common-messages'

const modal = ref<InstanceType<typeof NewModal>>()
const loadingAction = ref<'auto-fix' | 'disable-conflicts' | null>(null)
const { formatMessage } = useVIntl()

defineProps<{
    variant: 'loader-change' | 'game-version-change'
    loading?: boolean
}>()

const emit = defineEmits<{
    'confirm-loader-change': []
    'auto-fix': []
    'disable-conflicts': []
    cancel: []
}>()

function show(e?: MouseEvent) {
    loadingAction.value = null
    modal.value?.show(e)
}

function hide() {
    modal.value?.hide()
}

function handleCancel() {
    hide()
    emit('cancel')
}

function handleConfirmLoaderChange() {
    emit('confirm-loader-change')
}

function handleAutoFix() {
    loadingAction.value = 'auto-fix'
    emit('auto-fix')
}

function handleDisableConflicts() {
    loadingAction.value = 'disable-conflicts'
    emit('disable-conflicts')
}

const messages = defineMessages({
    header: {
        id: 'installation-settings.incompatible-content.header',
        defaultMessage: 'Incompatible content installed',
    },
    loaderChangeTitle: {
        id: 'installation-settings.incompatible-content.loader-change-title',
        defaultMessage: 'Changing loaders is destructive',
    },
    loaderChangeBody: {
        id: 'installation-settings.incompatible-content.loader-change-body',
        defaultMessage:
            'When changing the loader, all installed content will be disabled. We recommend resetting your server instead.',
    },
    gameVersionWarningTitle: {
        id: 'installation-settings.incompatible-content.game-version-warning-title',
        defaultMessage: 'Incompatibility warning',
    },
    gameVersionWarningBody: {
        id: 'installation-settings.incompatible-content.game-version-warning-body',
        defaultMessage:
            'When changing the game version, we can either disable incompatible installed content or attempt to resolve the incompatibilities.',
    },
    changeLoaderButton: {
        id: 'installation-settings.incompatible-content.change-loader-button',
        defaultMessage: 'Change loader',
    },
    autoFixButton: {
        id: 'installation-settings.incompatible-content.auto-fix-button',
        defaultMessage: 'Auto-fix',
    },
    disableConflictsButton: {
        id: 'installation-settings.incompatible-content.disable-conflicts-button',
        defaultMessage: 'Disable conflicts',
    },
})

defineExpose({ show, hide })
</script>
