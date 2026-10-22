<template>
    <ConfirmActionModal
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
    </ConfirmActionModal>
</template>

<script setup lang="ts">
import { TrashIcon } from '@modrinth/assets'
import {
    Admonition,
    ConfirmActionModal,
    defineMessages,
    useFormatBytes,
    useVIntl,
} from '@modrinth/ui'
import { ref } from 'vue'

import InstanceIcon from '@/components/ui/InstanceIcon.vue'
import { type BackupDeleteSummary, getBackupDeleteSummary } from '@/helpers/instance-backup'
import type { GameInstance } from '@/helpers/types'

const { formatMessage } = useVIntl()
const formatBytes = useFormatBytes()

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

const messages = defineMessages({
    header: {
        id: 'app.instance.confirm-delete.header',
        defaultMessage: 'Delete instance',
    },
    batchHeader: {
        id: 'app.instance.confirm-delete.batch-header',
        defaultMessage: 'Delete instances',
    },
    admonitionHeader: {
        id: 'app.instance.confirm-delete.admonition-header',
        defaultMessage: 'This action cannot be undone',
    },
    admonitionBody: {
        id: 'app.instance.confirm-delete.admonition-body',
        defaultMessage:
            'All data for your instance will be permanently deleted, including your worlds, configs, and all installed content.',
    },
    batchAdmonitionBody: {
        id: 'app.instance.confirm-delete.batch-admonition-body',
        defaultMessage:
            '{count, plural, one {# instance} other {# instances}} will be permanently deleted, including worlds, configs, and all installed content.',
    },
    symlinkDeleteWarning: {
        id: 'app.instance.confirm-delete.symlink-warning',
        defaultMessage:
            'This is a shared instance linked to "{path}". Only the link will be removed; the original files will not be deleted.',
    },
    backupCascadeWarning: {
        id: 'app.instance.confirm-delete.backup-cascade-warning',
        defaultMessage:
            '{count, plural, one {# backup} other {# backups}} ({size} total before deduplication) will also be permanently deleted.',
    },
    deleteButton: {
        id: 'app.instance.confirm-delete.delete-button',
        defaultMessage: 'Delete instance',
    },
    batchDeleteButton: {
        id: 'app.instance.confirm-delete.batch-delete-button',
        defaultMessage: 'Delete {count, plural, one {# instance} other {# instances}}',
    },
})

const modal = ref<InstanceType<typeof ConfirmActionModal>>()
const backupSummary = ref<BackupDeleteSummary | null>(null)

async function show() {
    backupSummary.value = null
    modal.value?.show()
    if (props.instanceId && props.count <= 1) {
        backupSummary.value = await getBackupDeleteSummary(props.instanceId).catch(() => null)
    }
}

defineExpose({
    show,
})
</script>
