<script setup lang="ts">
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { onBeforeUnmount, onMounted, reactive, ref } from 'vue'

import SettingsRow from '../../components/SettingsRow.vue'
import SettingsSection from '../../components/SettingsSection.vue'
import { messages } from '../../messages'
import { type SharedLog, useLaunchBackend } from './state'

/**
 * Shared logs (apps/app-frontend/src/components/ui/settings/SharedLogsSettings.vue):
 * every diagnostic the user shared, newest first, each deletable. The app hands a
 * log's link to the system browser; the preview opens what was uploaded in a new
 * tab instead, so no link leads to a share page that does not exist.
 */
const { formatMessage } = useVIntl()
const { addNotification, handleError } = injectNotificationManager()
const { list_shared_logs, delete_shared_log, shared_log_content } = useLaunchBackend()
const logs = ref<SharedLog[]>([])
const loading = ref(true)
const deletingId = ref<string | null>(null)
/** Object URLs of the uploaded contents, keyed by log id. */
const contentUrls = reactive<Record<string, string>>({})

function revokeContentUrl(id: string) {
    const url = contentUrls[id]
    if (!url) return
    URL.revokeObjectURL(url)
    Reflect.deleteProperty(contentUrls, id)
}

function syncContentUrls() {
    const ids = new Set(logs.value.map((log) => log.id))
    for (const id of Object.keys(contentUrls)) if (!ids.has(id)) revokeContentUrl(id)
    for (const log of logs.value) {
        if (contentUrls[log.id]) continue
        const content = shared_log_content(log.id)
        if (content === null) continue
        contentUrls[log.id] = URL.createObjectURL(
            new Blob([content], { type: 'text/plain;charset=utf-8' }),
        )
    }
}

async function refresh(): Promise<void> {
    try {
        logs.value = await list_shared_logs()
        syncContentUrls()
    } catch (error) {
        handleError(error as Error)
    } finally {
        loading.value = false
    }
}

async function remove(log: SharedLog): Promise<void> {
    if (deletingId.value) return
    deletingId.value = log.id
    try {
        await delete_shared_log(log.id, log.token)
        logs.value = logs.value.filter((item) => item.id !== log.id)
        revokeContentUrl(log.id)
        addNotification({
            title: formatMessage(messages.appLogShareSharedLogsDeleted),
            type: 'success',
        })
    } catch (error) {
        handleError(error as Error)
        addNotification({
            title: formatMessage(messages.appLogShareSharedLogsDeleteFailed),
            type: 'error',
        })
    } finally {
        deletingId.value = null
    }
}

function formatDate(timestamp: number): string {
    return new Date(timestamp * 1000).toLocaleString()
}

onMounted(refresh)
onBeforeUnmount(() => {
    for (const id of Object.keys(contentUrls)) revokeContentUrl(id)
})
</script>

<template>
    <SettingsSection
        id="settings-target-shared-logs"
        :title="formatMessage(messages.appLogShareSharedLogsTitle)"
        :description="formatMessage(messages.appLogShareSharedLogsDescription)"
    >
        <SettingsRow stacked>
            <template #control>
                <div class="flex w-full min-w-0 flex-col gap-2">
                    <p v-if="loading" class="m-0 p-2 text-sm text-[var(--color-text-tertiary)]">
                        …
                    </p>
                    <p
                        v-else-if="!logs.length"
                        class="m-0 p-2 text-sm text-[var(--color-text-tertiary)]"
                    >
                        {{ formatMessage(messages.appLogShareSharedLogsEmpty) }}
                    </p>
                    <ul
                        v-else
                        class="m-0 flex max-h-72 list-none flex-col gap-1 overflow-y-auto p-2"
                    >
                        <li
                            v-for="log in logs"
                            :key="log.id"
                            class="flex items-center gap-3 rounded-lg bg-surface-2 px-3 py-2"
                        >
                            <a
                                :href="contentUrls[log.id] ?? '#'"
                                target="_blank"
                                rel="noopener noreferrer"
                                class="min-w-0 flex-1 truncate text-[var(--color-text-default)] underline"
                            >
                                {{ log.url || log.id }}
                            </a>
                            <span class="shrink-0 text-xs text-[var(--color-text-tertiary)]">
                                {{ log.provider }}
                                <template v-if="log.truncated">
                                    · {{ formatMessage(messages.appLogShareSharedLogsTruncated) }}
                                </template>
                                <template v-if="log.instance_name">
                                    · {{ log.instance_name }}
                                </template>
                                · {{ formatDate(log.created_at) }}
                            </span>
                            <Button
                                type="outlined"
                                :disabled="deletingId === log.id"
                                @click="remove(log)"
                                >{{
                                    deletingId === log.id
                                        ? formatMessage(messages.appLogShareSharedLogsDeleting)
                                        : formatMessage(messages.appLogShareSharedLogsDelete)
                                }}
                            </Button>
                        </li>
                    </ul>
                </div>
            </template>
        </SettingsRow>
    </SettingsSection>
</template>
