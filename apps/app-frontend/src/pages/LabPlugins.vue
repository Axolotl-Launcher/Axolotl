<script setup lang="ts">
import { ClipboardCopyIcon, CopyIcon, PlugIcon, TriangleAlertIcon } from '@modrinth/assets'
import {
    Button,
    Card,
    defineMessages,
    EmptyState,
    type MessageDescriptor,
    TagItem,
    Toggle,
    useVIntl,
} from '@modrinth/ui'
import { computed, onBeforeUnmount, ref } from 'vue'

import {
    type LabPluginRegistration,
    labPluginRegistrations,
    labPluginRejections,
} from '@/lab/plugins/registry'
import {
    getLabPluginFailure,
    isLabPluginEnabled,
    retryLabPlugin,
    setLabPluginEnabled,
} from '@/lab/plugins/state'
import type { LabPluginPermission, LabPluginSlotName } from '@/lab/plugins/types'
import { useLabPluginText } from '@/lab/plugins/use-lab-plugin-text'

const { formatMessage } = useVIntl()
const resolveText = useLabPluginText()

/** Folder a plugin has to live in to be discovered by the build. */
const PLUGIN_SOURCE_PATH = 'apps/app-frontend/src/lab/plugins/packages'

const copied = ref(false)
let copiedTimer: ReturnType<typeof setTimeout> | undefined

const messages = defineMessages({
    title: { id: 'app.lab.plugins.title', defaultMessage: 'Plugins' },
    subtitle: {
        id: 'app.lab.plugins.subtitle',
        defaultMessage: '{count, plural, one {# plugin installed} other {# plugins installed}}',
    },
    enabledSubtitle: {
        id: 'app.lab.plugins.enabled-subtitle',
        defaultMessage: '{count, plural, one {# plugin on} other {# plugins on}}',
    },
    folderLabel: { id: 'app.lab.plugins.folder-label', defaultMessage: 'Plugin folder' },
    folderHint: {
        id: 'app.lab.plugins.folder-hint',
        defaultMessage: 'Plugins are compiled into the launcher. Add a folder here and rebuild.',
    },
    copyPath: { id: 'app.lab.plugins.copy-path', defaultMessage: 'Copy path' },
    copied: { id: 'app.lab.plugins.copied', defaultMessage: 'Copied' },
    emptyTitle: { id: 'app.lab.plugins.empty-title', defaultMessage: 'No plugins in this build' },
    emptyDescription: {
        id: 'app.lab.plugins.empty-description',
        defaultMessage: 'Add a plugin folder to the source tree and rebuild to see it here.',
    },
    toolPage: { id: 'app.lab.plugins.tool-page', defaultMessage: 'Tool page' },
    singleContribution: {
        id: 'app.lab.plugins.single-contribution',
        defaultMessage: 'Adds itself to the launcher, nothing else',
    },
    enable: { id: 'app.lab.plugins.enable', defaultMessage: 'Turn {name} on' },
    disable: { id: 'app.lab.plugins.disable', defaultMessage: 'Turn {name} off' },
    failed: { id: 'app.lab.plugins.failed', defaultMessage: 'Turned off after an error' },
    retry: { id: 'app.lab.plugins.retry', defaultMessage: 'Turn on' },
    rejectedTitle: { id: 'app.lab.plugins.rejected-title', defaultMessage: 'Skipped folders' },
    rejectedDescription: {
        id: 'app.lab.plugins.rejected-description',
        defaultMessage:
            'These folders do not describe a working plugin, so the launcher ignored them.',
    },
    permissionInstancesRead: {
        id: 'app.lab.plugins.permission.instances-read',
        defaultMessage: 'Reads your instances',
    },
    permissionFilesRead: {
        id: 'app.lab.plugins.permission.files-read',
        defaultMessage: 'Reads files you select',
    },
    permissionFilesWrite: {
        id: 'app.lab.plugins.permission.files-write',
        defaultMessage: 'Writes files you select',
    },
    permissionNetwork: {
        id: 'app.lab.plugins.permission.network',
        defaultMessage: 'Uses the network',
    },
    permissionClipboard: {
        id: 'app.lab.plugins.permission.clipboard',
        defaultMessage: 'Uses the clipboard',
    },
    slotLabFooter: {
        id: 'app.lab.plugins.slot.lab-footer',
        defaultMessage: 'Lab footer',
    },
    slotInstanceActions: {
        id: 'app.lab.plugins.slot.instance-actions',
        defaultMessage: 'Instance actions',
    },
})

const permissionMessages: Record<LabPluginPermission, MessageDescriptor> = {
    'instances:read': messages.permissionInstancesRead,
    'files:read': messages.permissionFilesRead,
    'files:write': messages.permissionFilesWrite,
    network: messages.permissionNetwork,
    clipboard: messages.permissionClipboard,
}

const slotMessages: Record<LabPluginSlotName, MessageDescriptor> = {
    'lab.hub.footer': messages.slotLabFooter,
    'instance.actions': messages.slotInstanceActions,
}

type PluginRow = {
    id: string
    name: string
    description: string
    /** Version and author joined for display, e.g. `1.0.0 · Axolotl`. */
    meta: string
    permissions: LabPluginPermission[]
    slots: LabPluginSlotName[]
    hasTool: boolean
    enabled: boolean
    failure?: string
}

const rows = computed<PluginRow[]>(() =>
    labPluginRegistrations.map((registration: LabPluginRegistration) => {
        const { manifest, tool, slots } = registration.plugin
        return {
            id: manifest.id,
            name: resolveText(manifest.name),
            description: resolveText(manifest.description),
            meta: [manifest.version, manifest.author].filter(Boolean).join(' · '),
            permissions: manifest.permissions ?? [],
            slots: (slots ?? []).map((contribution) => contribution.slot),
            hasTool: Boolean(tool),
            enabled: isLabPluginEnabled(manifest.id),
            failure: getLabPluginFailure(manifest.id),
        }
    }),
)

const enabledCount = computed(() => rows.value.filter((row) => row.enabled).length)

async function copyPluginPath() {
    try {
        await navigator.clipboard.writeText(PLUGIN_SOURCE_PATH)
        copied.value = true
        if (copiedTimer) clearTimeout(copiedTimer)
        copiedTimer = setTimeout(() => {
            copied.value = false
        }, 2000)
    } catch {
        copied.value = false
    }
}

onBeforeUnmount(() => {
    if (copiedTimer) clearTimeout(copiedTimer)
})

function togglePlugin(row: PluginRow, enabled: boolean | undefined) {
    const next = enabled ?? false
    if (next && row.failure) {
        retryLabPlugin(row.id)
        return
    }
    setLabPluginEnabled(row.id, next)
}
</script>

<template>
    <main class="flex w-full flex-col gap-6 p-6">
        <header class="flex min-w-0 flex-wrap items-start justify-between gap-4">
            <div class="min-w-0">
                <h1 class="m-0 text-2xl font-bold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.title) }}
                </h1>
                <p class="m-0 mt-1 text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.subtitle, { count: rows.length }) }}
                </p>
            </div>
            <TagItem v-if="enabledCount !== rows.length">
                {{ formatMessage(messages.enabledSubtitle, { count: enabledCount }) }}
            </TagItem>
        </header>

        <section
            class="flex flex-wrap items-center justify-between gap-3 rounded-[var(--radius-lg)] border border-surface-5 bg-surface-2 p-4"
        >
            <div class="min-w-0">
                <p class="m-0 text-sm font-bold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.folderLabel) }}
                </p>
                <code class="mt-1 block break-all text-xs text-[var(--color-text-tertiary)]">
                    {{ PLUGIN_SOURCE_PATH }}
                </code>
                <p class="m-0 mt-1 text-xs text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.folderHint) }}
                </p>
            </div>
            <Button type="outlined" size="sm" @click="copyPluginPath">
                <component :is="copied ? ClipboardCopyIcon : CopyIcon" />
                {{ formatMessage(copied ? messages.copied : messages.copyPath) }}
            </Button>
        </section>

        <section v-if="rows.length" class="flex flex-col gap-3" aria-label="Installed plugins">
            <Card v-for="row in rows" :key="row.id" class="!m-0 flex flex-col gap-3 !p-4">
                <div class="flex items-start gap-3">
                    <div
                        class="flex size-10 shrink-0 items-center justify-center rounded-[var(--radius-md)] bg-brand-highlight text-brand"
                    >
                        <PlugIcon class="size-5" />
                    </div>
                    <div class="min-w-0 flex-1">
                        <div class="flex min-w-0 flex-wrap items-baseline gap-2">
                            <h2
                                class="m-0 truncate text-base font-bold text-[var(--color-text-primary)]"
                            >
                                {{ row.name }}
                            </h2>
                            <span class="text-xs text-[var(--color-text-tertiary)]">
                                {{ row.meta }}
                            </span>
                        </div>
                        <p class="m-0 mt-1 text-sm text-[var(--color-text-tertiary)]">
                            {{ row.description }}
                        </p>
                    </div>
                    <Toggle
                        :model-value="row.enabled"
                        :aria-label="
                            formatMessage(row.enabled ? messages.disable : messages.enable, {
                                name: row.name,
                            })
                        "
                        @update:model-value="togglePlugin(row, $event)"
                    />
                </div>

                <div class="flex flex-wrap items-center gap-1.5">
                    <TagItem v-if="row.hasTool">{{ formatMessage(messages.toolPage) }}</TagItem>
                    <TagItem v-for="slot in row.slots" :key="slot">
                        {{ formatMessage(slotMessages[slot]) }}
                    </TagItem>
                    <TagItem v-for="permission in row.permissions" :key="permission">
                        {{ formatMessage(permissionMessages[permission]) }}
                    </TagItem>
                    <span
                        v-if="!row.hasTool && !row.slots.length && !row.permissions.length"
                        class="text-xs text-[var(--color-text-tertiary)]"
                    >
                        {{ formatMessage(messages.singleContribution) }}
                    </span>
                </div>

                <div
                    v-if="row.failure"
                    class="flex flex-wrap items-center gap-2 text-xs text-[var(--color-text-tertiary)]"
                >
                    <TriangleAlertIcon class="size-3.5 shrink-0" />
                    <span>{{ formatMessage(messages.failed) }}</span>
                    <span class="min-w-0 break-all">— {{ row.failure }}</span>
                    <Button type="outlined" size="sm" @click="retryLabPlugin(row.id)">
                        {{ formatMessage(messages.retry) }}
                    </Button>
                </div>
            </Card>
        </section>

        <EmptyState
            v-else
            type="empty"
            :heading="formatMessage(messages.emptyTitle)"
            :description="formatMessage(messages.emptyDescription)"
        />

        <section
            v-if="labPluginRejections.length"
            class="rounded-[var(--radius-lg)] border border-surface-5 bg-surface-2 p-4"
        >
            <h2 class="m-0 text-sm font-bold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.rejectedTitle) }}
            </h2>
            <p class="m-0 mt-1 text-xs text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.rejectedDescription) }}
            </p>
            <ul class="m-0 mt-2 flex list-none flex-col gap-1 p-0">
                <li
                    v-for="rejection in labPluginRejections"
                    :key="rejection.folder"
                    class="text-xs text-[var(--color-text-tertiary)]"
                >
                    <code>{{ rejection.folder }}</code> — {{ rejection.reason }}
                </li>
            </ul>
        </section>
    </main>
</template>
