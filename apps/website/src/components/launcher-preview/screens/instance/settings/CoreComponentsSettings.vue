<script setup lang="ts">
import ArrowDownIcon from '@modrinth/assets/icons/arrow-down.svg?component'
import ArrowUpIcon from '@modrinth/assets/icons/arrow-up.svg?component'
import DownloadIcon from '@modrinth/assets/icons/download.svg?component'
import ExternalIcon from '@modrinth/assets/icons/external.svg?component'
import EyeIcon from '@modrinth/assets/icons/eye.svg?component'
import FileArchiveIcon from '@modrinth/assets/icons/file-archive.svg?component'
import PlusIcon from '@modrinth/assets/icons/plus.svg?component'
import RestoreIcon from '@modrinth/assets/icons/restore.svg?component'
import TrashIcon from '@modrinth/assets/icons/trash.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Checkbox from '@modrinth/ui/src/components/base/Checkbox.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { computed, ref, shallowRef, useTemplateRef, watch } from 'vue'

import { formatPreviewMessage } from '../../../lib/i18n'
import { desktopAppMessages } from '../../../lib/messages'
import { coreComponentsMessages as messages } from '../messages'
import { injectInstanceSettings } from './context'
import {
    canInstallModLoaderFor,
    type CoreComponent,
    useInstanceCoreComponents,
} from './core-components'
import { useSimulatedInvoke } from './state'

/**
 * Port of `apps/app-frontend/src/components/ui/instance_settings/CoreComponentsSettings.vue`.
 * The system open dialog ("Choose core archive", .jar/.zip) is the browser's file
 * picker; the archive is read, validated and hashed in the browser.
 */
const { instance } = injectInstanceSettings()
const { addNotification, handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const coreComponents = useInstanceCoreComponents()
const simulate = useSimulatedInvoke()
const busy = ref(false)

/** `useQuery(['core-components', id])`. */
const components = shallowRef<CoreComponent[] | undefined>(undefined)
async function refresh() {
    await simulate(80)
    components.value = coreComponents.list(instance.value.id)
}
watch(
    () => instance.value.id,
    () => {
        components.value = undefined
        void refresh()
    },
    { immediate: true },
)

const activeComponents = computed(() =>
    (components.value ?? []).filter((component) => !component.removed),
)
const deletedComponents = computed(() =>
    (components.value ?? []).filter((component) => component.removed),
)
const canInstallModLoader = computed(() => canInstallModLoaderFor(instance.value.game_version))
const manualModLoader = ref<{
    fileName: string
    pageUrl: string | null
    expectedSha256: string | null
} | null>(null)

const fileInput = useTemplateRef<HTMLInputElement>('fileInput')

/** `open({ filters: [{ name: 'Minecraft archives', extensions: ['jar', 'zip'] }] })`. */
function open(): Promise<File | null> {
    const input = fileInput.value
    if (!input) return Promise.resolve(null)
    return new Promise((resolve) => {
        const done = (value: File | null) => {
            input.removeEventListener('change', onChange)
            input.removeEventListener('cancel', onCancel)
            resolve(value)
        }
        const onChange = () => {
            const file = input.files?.[0] ?? null
            input.value = ''
            done(file)
        }
        const onCancel = () => done(null)
        input.addEventListener('change', onChange)
        input.addEventListener('cancel', onCancel)
        input.value = ''
        input.click()
    })
}

async function pick(kind: 'jar_mod' | 'replacement_jar') {
    const file = await open()
    if (!file) return
    busy.value = true
    try {
        await simulate(200)
        await coreComponents.add(instance.value.id, file, kind, instance.value.game_version)
        await refresh()
    } catch (error) {
        handleError(error)
    } finally {
        busy.value = false
    }
}

async function run(action: () => unknown) {
    busy.value = true
    try {
        await simulate(140)
        await action()
        await refresh()
    } catch (error) {
        handleError(error)
    } finally {
        busy.value = false
    }
}

async function preview() {
    busy.value = true
    try {
        await simulate(700)
        const result = await coreComponents.preview(instance.value.id)
        if (result) {
            addNotification({
                type: 'success',
                title: formatMessage(messages.previewed, {
                    components: result.componentCount,
                    entries: result.entries,
                }),
            })
        }
        await refresh()
    } catch (error) {
        handleError(error)
        await refresh()
    } finally {
        busy.value = false
    }
}

/**
 * `install_mcarchive_modloader` downloads the release from MCArchive, an online
 * service only the desktop app reaches.
 */
async function installModLoader() {
    busy.value = true
    manualModLoader.value = null
    try {
        await simulate(900)
        throw new Error(formatPreviewMessage(desktopAppMessages.onlineService))
    } catch (error) {
        handleError(error)
    } finally {
        busy.value = false
    }
}

async function importModLoader() {
    if (!manualModLoader.value) return
    const file = await open()
    if (!file) return
    busy.value = true
    try {
        // The file is checked against MCArchive's hash, which only the desktop app fetches.
        await simulate(300)
        throw new Error(formatPreviewMessage(desktopAppMessages.onlineService))
    } catch (error) {
        handleError(error)
    } finally {
        busy.value = false
    }
}

function openUrl(url: string) {
    window.open(url, '_blank', 'noopener,noreferrer')
}
</script>

<template>
    <div class="flex flex-col gap-4">
        <div class="flex flex-wrap gap-2">
            <Button :disabled="busy" @click="pick('jar_mod')"
                ><PlusIcon />
                {{ formatMessage(messages.add) }}
            </Button>
            <Button type="outlined" :disabled="busy" @click="pick('replacement_jar')"
                ><FileArchiveIcon />
                {{ formatMessage(messages.replace) }}
            </Button>
            <Button
                v-if="canInstallModLoader"
                type="outlined"
                :disabled="busy"
                @click="installModLoader"
                ><DownloadIcon />
                {{ formatMessage(messages.modLoader) }}
            </Button>
            <Button type="quiet" :disabled="busy" @click="preview"
                ><EyeIcon />
                {{ formatMessage(messages.preview) }}
            </Button>
        </div>

        <div
            v-if="manualModLoader"
            class="flex flex-wrap items-center justify-between gap-3 border-y border-surface-4 py-3"
        >
            <p class="m-0 min-w-0 flex-1 text-sm text-[var(--color-text-tertiary)]">
                {{
                    formatMessage(messages.modLoaderManual, { fileName: manualModLoader.fileName })
                }}
            </p>
            <Button
                v-if="manualModLoader.pageUrl"
                type="outlined"
                @click="openUrl(manualModLoader!.pageUrl!)"
                ><ExternalIcon />
                {{ formatMessage(messages.openSource) }}
            </Button>
            <Button :disabled="busy" @click="importModLoader"
                ><FileArchiveIcon />
                {{ formatMessage(messages.importModLoader) }}
            </Button>
        </div>

        <div class="overflow-hidden rounded-lg border border-surface-4">
            <div
                v-for="(component, index) in activeComponents"
                :key="component.id"
                class="flex items-center gap-3 border-b border-surface-4 px-3 py-3 last:border-b-0"
            >
                <Checkbox
                    :model-value="component.enabled"
                    :disabled="busy"
                    @update:model-value="
                        (enabled) =>
                            run(() => coreComponents.setEnabled(instance.id, component.id, enabled))
                    "
                />
                <div class="min-w-0 flex-1">
                    <div class="truncate font-medium text-[var(--color-text-primary)]">
                        {{ component.fileName }}
                    </div>
                    <div class="text-xs text-[var(--color-text-tertiary)]">
                        {{
                            formatMessage(
                                component.kind === 'replacement_jar'
                                    ? messages.replacement
                                    : messages.jarMod,
                            )
                        }}
                        <span v-if="component.targetGameVersion">
                            · {{ formatMessage(messages.targetVersion) }}
                            {{ component.targetGameVersion }}
                        </span>
                        <span v-if="component.source">
                            · {{ formatMessage(messages.source) }}
                            {{ component.source.provider }}</span
                        >
                    </div>
                    <div
                        v-if="component.sha256 || component.sha1"
                        class="truncate text-xs text-[var(--color-text-tertiary)]"
                    >
                        <span v-if="component.sha256"
                            >{{ formatMessage(messages.sha256) }} {{ component.sha256 }}</span
                        >
                        <span v-if="component.sha256 && component.sha1"> · </span>
                        <span v-if="component.sha1"
                            >{{ formatMessage(messages.sha1) }} {{ component.sha1 }}</span
                        >
                    </div>
                    <div v-if="component.failureReason" class="text-xs text-red">
                        {{ formatMessage(messages.failure) }}: {{ component.failureReason }}
                    </div>
                </div>
                <div class="flex shrink-0 items-center gap-1">
                    <Button
                        v-tooltip="formatMessage(messages.moveUp)"
                        type="quiet"
                        size="2xs"
                        circular
                        icon-only
                        :aria-label="formatMessage(messages.moveUp)"
                        :disabled="busy || index === 0"
                        @click="run(() => coreComponents.move(instance.id, component.id, -1))"
                        ><ArrowUpIcon />
                    </Button>
                    <Button
                        v-tooltip="formatMessage(messages.moveDown)"
                        type="quiet"
                        size="2xs"
                        circular
                        icon-only
                        :aria-label="formatMessage(messages.moveDown)"
                        :disabled="busy || index === activeComponents.length - 1"
                        @click="run(() => coreComponents.move(instance.id, component.id, 1))"
                        ><ArrowDownIcon />
                    </Button>
                    <Button
                        v-tooltip="formatMessage(messages.remove)"
                        type="quiet"
                        color="red"
                        size="2xs"
                        circular
                        icon-only
                        :aria-label="formatMessage(messages.remove)"
                        :disabled="busy"
                        @click="run(() => coreComponents.remove(instance.id, component.id))"
                        ><TrashIcon />
                    </Button>
                </div>
            </div>
        </div>

        <div
            v-if="deletedComponents.length"
            class="overflow-hidden rounded-lg border border-surface-4"
        >
            <div
                v-for="component in deletedComponents"
                :key="component.id"
                class="flex items-center gap-3 px-3 py-3"
            >
                <div class="min-w-0 flex-1 truncate text-[var(--color-text-tertiary)]">
                    {{ component.fileName }}
                </div>
                <Button
                    v-tooltip="formatMessage(messages.restore)"
                    type="quiet"
                    size="2xs"
                    circular
                    icon-only
                    :aria-label="formatMessage(messages.restore)"
                    :disabled="busy"
                    @click="run(() => coreComponents.restore(instance.id, component.id))"
                    ><RestoreIcon />
                </Button>
            </div>
        </div>
        <input ref="fileInput" type="file" accept=".jar,.zip" class="hidden" />
    </div>
</template>
