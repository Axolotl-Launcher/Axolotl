<template>
    <PreviewConfirmActionModal
        ref="modal"
        :header="formatMessage(count > 1 ? messages.deleteBatchHeader : messages.deleteHeader)"
        fade="danger"
        max-width="500px"
        :confirm-action="deleteAction"
        :confirm-icon="TrashIcon"
        confirm-color="red"
        :confirm-label="
            formatMessage(count > 1 ? messages.deleteBatchButton : messages.deleteButton, {
                count,
            })
        "
    >
        <Admonition
            v-if="!symlinkTarget && count <= 1"
            type="critical"
            :header="formatMessage(messages.deleteAdmonitionHeader)"
        >
            {{ formatMessage(messages.deleteAdmonitionBody) }}
        </Admonition>
        <Admonition
            v-else-if="!symlinkTarget"
            type="critical"
            :header="formatMessage(messages.deleteAdmonitionHeader)"
        >
            {{ formatMessage(messages.deleteBatchAdmonitionBody, { count }) }}
        </Admonition>
        <Admonition v-else type="critical">
            {{ formatMessage(messages.deleteSymlinkWarning, { path: symlinkTarget }) }}
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
                formatMessage(messages.deleteBackupCascadeWarning, {
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

import { usePreview } from '../../../lib/state'
import type { GameInstance } from '../../../lib/types'
import PreviewConfirmActionModal from '../../../overlays/PreviewConfirmActionModal.vue'
import InstanceIcon from '../../home/components/InstanceIcon.vue'
import { messages } from '../messages'

/**
 * Port of `components/ui/modal/ConfirmDeleteInstanceModal.vue` for the Library,
 * with the Library's strings so its chunk does not carry the instance page's.
 */
const { formatMessage } = useVIntl()
const formatBytes = useFormatBytes()
const preview = usePreview()

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

interface BackupDeleteSummary {
    snapshot_count: number
    logical_size: number
}

const modal = ref<InstanceType<typeof PreviewConfirmActionModal>>()
const backupSummary = ref<BackupDeleteSummary | null>(null)

/**
 * `getBackupDeleteSummary`. Snapshots exist only once the instance settings'
 * Backups tab has made them, so an instance without that store has none.
 */
async function show() {
    backupSummary.value = null
    modal.value?.show()
    if (props.instanceId && props.count <= 1) {
        const instanceId = props.instanceId
        await new Promise<void>((resolve) => preview.clock.after(120, resolve))
        const backups = preview.domains.get('instance-settings:backups')?.value as
            { deleteSummary: (id: string) => BackupDeleteSummary } | undefined
        backupSummary.value = backups?.deleteSummary(instanceId) ?? null
    }
}

defineExpose({
    show,
})
</script>
