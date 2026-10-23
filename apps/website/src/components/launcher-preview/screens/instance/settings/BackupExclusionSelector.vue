<script setup lang="ts">
import FileIcon from '@modrinth/assets/icons/file.svg?component'
import FolderIcon from '@modrinth/assets/icons/folder.svg?component'
import MinusIcon from '@modrinth/assets/icons/minus.svg?component'
import PlusIcon from '@modrinth/assets/icons/plus.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { ref, useTemplateRef } from 'vue'

import { backupExclusionMessages as messages } from '../messages'
import {
    type BackupExclusion,
    type BackupExclusionKind,
    canonicalizeBackupExclusions,
    formatBackupExclusionPath,
} from './backups'
import { useSimulatedInvoke } from './state'

/**
 * Port of `apps/app-frontend/src/components/ui/instance_settings/BackupExclusionSelector.vue`.
 * The system open dialog starts in the instance folder; the browser's file and
 * folder pickers stand in for it, and the picked entry is taken as a path inside
 * the instance (`instance_normalize_backup_exclusion`).
 */
const props = defineProps<{
    instanceId: string
    instanceRoot: string
    modelValue: BackupExclusion[]
    disabled?: boolean
}>()
const emit = defineEmits<{ 'update:modelValue': [value: BackupExclusion[]] }>()
const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const simulate = useSimulatedInvoke()
const picking = ref<BackupExclusionKind | null>(null)
const fileInput = useTemplateRef<HTMLInputElement>('fileInput')
const folderInput = useTemplateRef<HTMLInputElement>('folderInput')

/** Resolves with the picked file or folder name, or null when the picker is dismissed. */
function open(kind: BackupExclusionKind): Promise<string | null> {
    if (kind === 'directory' && typeof window !== 'undefined' && 'showDirectoryPicker' in window) {
        const picker = (
            window as unknown as { showDirectoryPicker: () => Promise<{ name: string }> }
        ).showDirectoryPicker
        return picker.call(window).then(
            (handle) => handle.name,
            () => null,
        )
    }
    const input = kind === 'directory' ? folderInput.value : fileInput.value
    if (!input) return Promise.resolve(null)
    return new Promise((resolve) => {
        const done = (value: string | null) => {
            input.removeEventListener('change', onChange)
            input.removeEventListener('cancel', onCancel)
            resolve(value)
        }
        const onChange = () => {
            const file = input.files?.[0]
            input.value = ''
            if (!file) return done(null)
            const relative = (file as File & { webkitRelativePath?: string }).webkitRelativePath
            done(kind === 'directory' && relative ? relative.split('/')[0] : file.name)
        }
        const onCancel = () => done(null)
        input.addEventListener('change', onChange)
        input.addEventListener('cancel', onCancel)
        input.value = ''
        input.click()
    })
}

async function addExclusion(kind: BackupExclusionKind) {
    if (props.disabled || picking.value) return
    picking.value = kind
    try {
        const selectedPath = await open(kind)
        if (!selectedPath) return
        await simulate(120)
        const exclusion: BackupExclusion = { path: selectedPath.replaceAll('\\', '/'), kind }
        emit('update:modelValue', canonicalizeBackupExclusions([...props.modelValue, exclusion]))
    } catch (error) {
        handleError(error)
    } finally {
        picking.value = null
    }
}

function removeExclusion(path: string) {
    emit(
        'update:modelValue',
        props.modelValue.filter((exclusion) => exclusion.path !== path),
    )
}
</script>

<template>
    <div class="flex flex-col gap-3">
        <p v-if="modelValue.length === 0" class="m-0 text-sm text-[var(--color-text-tertiary)]">
            {{ formatMessage(messages.empty) }}
        </p>
        <div
            v-else
            class="divide-y divide-solid divide-surface-4 rounded-lg border border-solid border-surface-4"
        >
            <div
                v-for="exclusion in modelValue"
                :key="exclusion.path"
                class="flex min-h-11 items-center gap-3 px-3 py-2"
            >
                <FolderIcon
                    v-if="exclusion.kind === 'directory'"
                    class="size-5 shrink-0 text-[var(--color-text-tertiary)]"
                />
                <FileIcon v-else class="size-5 shrink-0 text-[var(--color-text-tertiary)]" />
                <code class="min-w-0 flex-1 truncate text-sm text-[var(--color-text-primary)]">{{
                    formatBackupExclusionPath(exclusion)
                }}</code>
                <Button
                    type="quiet"
                    size="2xs"
                    circular
                    icon-only
                    :disabled="disabled || picking !== null"
                    :aria-label="
                        formatMessage(messages.remove, {
                            path: formatBackupExclusionPath(exclusion),
                        })
                    "
                    @click="removeExclusion(exclusion.path)"
                    ><MinusIcon />
                </Button>
            </div>
        </div>
        <div class="flex flex-wrap gap-2">
            <Button
                type="outlined"
                :disabled="disabled || picking !== null"
                @click="addExclusion('file')"
                ><PlusIcon />
                {{ formatMessage(messages.addFile) }}
            </Button>
            <Button
                type="outlined"
                :disabled="disabled || picking !== null"
                @click="addExclusion('directory')"
                ><PlusIcon />
                {{ formatMessage(messages.addFolder) }}
            </Button>
        </div>
        <input ref="fileInput" type="file" class="hidden" />
        <input ref="folderInput" type="file" webkitdirectory class="hidden" />
    </div>
</template>
