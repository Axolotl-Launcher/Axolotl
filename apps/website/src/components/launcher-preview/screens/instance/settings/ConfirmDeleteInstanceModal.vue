<template>
    <PreviewConfirmActionModal
        ref="modal"
        :header="formatMessage(count > 1 ? messages.batchHeader : messages.header)"
        fade="danger"
        max-width="500px"
        :confirm-action="deleteAction"
        :confirm-icon="TrashIcon"
        confirm-color="red"
        :confirm-label="
            formatMessage(count > 1 ? messages.batchDeleteButton : messages.deleteButton, { count })
        "
    >
        <Admonition
            v-if="!symlinkTarget && count <= 1"
            type="critical"
            :header="formatMessage(messages.admonitionHeader)"
        >
            {{ formatMessage(messages.admonitionBody) }}
        </Admonition>
        <Admonition
            v-else-if="!symlinkTarget"
            type="critical"
            :header="formatMessage(messages.admonitionHeader)"
        >
            {{ formatMessage(messages.batchAdmonitionBody, { count }) }}
        </Admonition>
        <Admonition v-else type="critical">
            {{ formatMessage(messages.symlinkDeleteWarning, { path: symlinkTarget }) }}
        </Admonition>
        <ul
            v-if="instances.length"
            class="m-0 flex max-h-48 list-none flex-col gap-3 overflow-y-auto p-0"
        >
            <li
                v-for="instance in instances"
                :key="instance.id"
                class="flex min-w-0 items-start gap-3"
            >
                <InstanceIcon
                    :instance-id="instance.id"
                    :icon-path="instance.icon_path"
                    :loader="instance.loader"
                    class="size-10 shrink-0"
                />
                <div class="min-w-0 flex-1">
                    <div
                        class="break-words font-semibold text-[var(--color-text-primary)] [overflow-wrap:anywhere]"
                    >
                        {{ instance.name }}
                    </div>
                    <div
                        v-if="instance.loader || instance.game_version"
                        class="text-sm capitalize text-[var(--color-text-tertiary)]"
                    >
                        {{ instance.loader }} {{ instance.game_version }}
                    </div>
                    <div
                        v-if="
                            instance.path &&
                            instances.some(
                                (other) => other.id !== instance.id && other.name === instance.name,
                            )
                        "
                        class="break-words text-sm text-[var(--color-text-tertiary)] [overflow-wrap:anywhere]"
                    >
                        {{ instance.path }}
                    </div>
                </div>
            </li>
        </ul>
        <p v-if="backupSummary?.snapshot_count" class="m-0 text-[var(--color-text-tertiary)]">
            {{
                formatMessage(messages.backupCascadeWarning, {
                    count: backupSummary.snapshot_count,
                    size: formatBytes(backupSummary.logical_size),
                })
            }}
        </p>
    </PreviewConfirmActionModal>
</template>

<script setup lang="ts">
import TrashIcon from '@modrinth/assets/icons/trash.svg?component'
import Admonition from '@modrinth/ui/src/components/base/Admonition.vue'
import { useFormatBytes } from '@modrinth/ui/src/composables/format-bytes.ts'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { ref } from 'vue'

import type { GameInstance } from '../../../lib/types'
import PreviewConfirmActionModal from '../../../overlays/PreviewConfirmActionModal.vue'
import InstanceIcon from '../../home/components/InstanceIcon.vue'
import { confirmDeleteInstanceMessages as messages } from '../messages'
import { type BackupDeleteSummary, useInstanceBackups } from './backups'
import { useSimulatedInvoke } from './state'

/**
 * Port of `apps/app-frontend/src/components/ui/modal/ConfirmDeleteInstanceModal.vue`,
 * used by instance settings and the Library as in the app.
 */
const { formatMessage } = useVIntl()
const formatBytes = useFormatBytes()
const backups = useInstanceBackups()
const simulate = useSimulatedInvoke()

const props = withDefaults(
    defineProps<{
        deleteAction: () => Promise<void>
        instanceId?: string | null
        symlinkTarget?: string | null
        count?: number
        instances?: (Pick<GameInstance, 'id' | 'name'> &
            Partial<Pick<GameInstance, 'icon_path' | 'loader' | 'game_version' | 'path'>>)[]
    }>(),
    {
        instanceId: null,
        symlinkTarget: null,
        count: 1,
        instances: () => [],
    },
)

const modal = ref<InstanceType<typeof PreviewConfirmActionModal>>()
const backupSummary = ref<BackupDeleteSummary | null>(null)

/** `getBackupDeleteSummary`: the instance's snapshots that the deletion cascades to. */
async function show() {
    backupSummary.value = null
    modal.value?.show()
    if (props.instanceId && props.count <= 1) {
        const instanceId = props.instanceId
        await simulate(120)
        backupSummary.value = backups.deleteSummary(instanceId)
    }
}

defineExpose({
    show,
})
</script>
