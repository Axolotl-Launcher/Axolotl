<script setup lang="ts">
import ImageIcon from '@modrinth/assets/icons/image.svg?component'
import SaveIcon from '@modrinth/assets/icons/save.svg?component'
import SpinnerIcon from '@modrinth/assets/icons/spinner.svg?component'
import TrashIcon from '@modrinth/assets/icons/trash.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Admonition from '@modrinth/ui/src/components/base/Admonition.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Card from '@modrinth/ui/src/components/base/Card.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import Toggle from '@modrinth/ui/src/components/base/Toggle.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectFilePicker } from '@modrinth/ui/src/providers/file-picker.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { computed, onMounted, ref, useTemplateRef, watch } from 'vue'
import type { ComponentExposed } from 'vue-component-type-helpers'

import { requiredJavaMajorVersion } from '../../../lib/server-core.ts'
import ConfirmModal from '../../../overlays/PreviewConfirmModal.vue'
import JavaSelector from '../../instance/settings/java/JavaSelector.vue'
import { get_jre } from '../app-helpers.ts'
import { servers as serversApi } from '../backend.ts'
import { type ServerView, useServers } from '../composables/useServers.ts'
import ServerIcon from './ServerIcon.vue'
import ServerPropertiesEditor from './ServerPropertiesEditor.vue'

const props = defineProps<{
    server: ServerView
}>()

const emit = defineEmits<{
    deleted: []
}>()

const { formatMessage } = useVIntl()
const messages = defineMessages({
    stopCommand: { id: 'app.servers.settings.stop-command', defaultMessage: 'Stop command' },
    stopCommandHint: {
        id: 'app.servers.settings.stop-command-hint',
        defaultMessage:
            'Defaults to stop. Enter ^C to send Ctrl+C. Saved changes apply immediately.',
    },
    general: { id: 'app.servers.settings.general', defaultMessage: 'General' },
    name: { id: 'app.servers.settings.name', defaultMessage: 'Server name' },
    icon: { id: 'app.servers.settings.icon', defaultMessage: 'Icon' },
    selectIcon: { id: 'app.servers.icon.select', defaultMessage: 'Select icon' },
    changeIcon: { id: 'app.servers.icon.change', defaultMessage: 'Change icon' },
    removeIcon: { id: 'app.servers.icon.remove', defaultMessage: 'Remove icon' },
    java: { id: 'app.servers.settings.java', defaultMessage: 'Java' },
    memory: { id: 'app.servers.settings.memory-mb', defaultMessage: 'Memory (MB)' },
    jvmArgs: { id: 'app.servers.settings.jvm-args', defaultMessage: 'JVM arguments' },
    jvmArgsHint: {
        id: 'app.servers.settings.jvm-args-hint',
        defaultMessage: 'Space-separated arguments, e.g. -XX:+UseG1GC',
    },
    preLaunchHook: {
        id: 'app.settings.defaults.pre-launch-hook',
        defaultMessage: 'Pre-launch hook',
    },
    preLaunchHookHint: {
        id: 'app.servers.settings.pre-launch-hook-hint',
        defaultMessage:
            'Runs in the server directory before the server starts. A non-zero exit code cancels the start.',
    },
    save: { id: 'app.servers.settings.save', defaultMessage: 'Save changes' },
    saved: { id: 'app.servers.settings.saved', defaultMessage: 'Server settings saved' },
    cancel: { id: 'app.servers.settings.cancel', defaultMessage: 'Cancel' },
    deleteTitle: { id: 'app.servers.settings.delete', defaultMessage: 'Delete server' },
    deleteHint: {
        id: 'app.servers.settings.delete-hint',
        defaultMessage: 'Permanently remove this server and all of its files.',
    },
    deleteConfirm: {
        id: 'app.servers.settings.delete-confirm',
        defaultMessage: 'Delete {name} and all of its files? This cannot be undone.',
    },
    deleteProceed: { id: 'app.servers.settings.delete-proceed', defaultMessage: 'Delete' },
    configFiles: { id: 'app.servers.settings.config', defaultMessage: 'Configuration' },
    runningTitle: {
        id: 'app.servers.settings.running-title',
        defaultMessage: 'Server is running',
    },
    runningHint: {
        id: 'app.servers.settings.running-hint',
        defaultMessage: 'Your changes will take effect the next time the server starts.',
    },
    pinHome: {
        id: 'app.servers.settings.pin-home',
        defaultMessage: 'Pin to Home',
    },
})

const { deleteServer, refresh } = useServers()
const { addNotification, handleError } = injectNotificationManager()
const filePicker = injectFilePicker()

const name = ref(props.server.name)
const iconPath = ref<string | null>(props.server.iconPath ?? null)
const javaSelection = ref<{ path: string; version: string }>({
    path: props.server.javaPath ?? '',
    version: '',
})
const memoryMb = ref(props.server.memoryMb ?? 2048)
const jvmArgsText = ref((props.server.jvmArgs ?? []).join(' '))
const preLaunchHookText = ref(props.server.preLaunchHook ?? '')
const stopCommand = ref(props.server.stopCommand ?? 'stop')
const pinnedToHome = ref(Boolean(props.server.homePinnedAt))
const isSaving = ref(false)
const deleteModal = useTemplateRef<ComponentExposed<typeof ConfirmModal>>('deleteModal')
const editor = useTemplateRef<ComponentExposed<typeof ServerPropertiesEditor>>('editor')

const requiredJava = computed(() => requiredJavaMajorVersion(props.server.gameVersion))

// Same busy boundary as the files panel: a running server holds its
// configuration in memory and may overwrite external edits, and
// manifest changes only take effect after a restart anyway.
const isRunning = computed(() => props.server.running || props.server.starting)

const baseline = ref({
    name: props.server.name,
    iconPath: props.server.iconPath ?? null,
    javaPath: props.server.javaPath ?? '',
    javaVersion: '',
    memoryMb: props.server.memoryMb ?? 2048,
    jvmArgs: (props.server.jvmArgs ?? []).join(' '),
    preLaunchHook: props.server.preLaunchHook ?? '',
    stopCommand: props.server.stopCommand ?? 'stop',
    homePinned: Boolean(props.server.homePinnedAt),
})

onMounted(async () => {
    if (!javaSelection.value.path) return
    try {
        const jre = await get_jre(javaSelection.value.path)
        if (jre) {
            javaSelection.value.version = jre.version
            baseline.value.javaVersion = jre.version
        }
    } catch {
        // Keep the path; the selector validates against the required major version.
    }
})

watch(
    () => props.server.iconPath,
    (value) => {
        const synced = value ?? null
        iconPath.value = synced
        baseline.value.iconPath = synced
    },
)

const generalDirty = computed(
    () =>
        name.value !== baseline.value.name ||
        iconPath.value !== baseline.value.iconPath ||
        javaSelection.value.path !== baseline.value.javaPath ||
        javaSelection.value.version !== baseline.value.javaVersion ||
        memoryMb.value !== baseline.value.memoryMb ||
        jvmArgsText.value !== baseline.value.jvmArgs ||
        preLaunchHookText.value !== baseline.value.preLaunchHook ||
        pinnedToHome.value !== baseline.value.homePinned,
)

const isDirty = computed(
    () =>
        generalDirty.value ||
        stopCommand.value !== baseline.value.stopCommand ||
        (editor.value?.isDirty ?? false),
)
const saveDisabled = computed(
    () =>
        isSaving.value ||
        (isRunning.value && (generalDirty.value || (editor.value?.isDirty ?? false))),
)

async function save() {
    if (saveDisabled.value) return
    isSaving.value = true
    try {
        const jvmArgs = jvmArgsText.value.trim().split(/\s+/).filter(Boolean)
        const trimmedStop = stopCommand.value.trim() || 'stop'
        const stopCommandValue = trimmedStop.toLowerCase() === '^c' ? '^C' : trimmedStop
        if (isRunning.value) {
            await serversApi.updateSettings(props.server.id, { stopCommand: stopCommandValue })
            stopCommand.value = stopCommandValue
            baseline.value.stopCommand = stopCommandValue
            await refresh()
            addNotification({ type: 'success', title: formatMessage(messages.saved) })
            return
        }
        const parsedMemory = Number(memoryMb.value)
        const memoryMbValue =
            Number.isFinite(parsedMemory) && parsedMemory > 0
                ? parsedMemory
                : baseline.value.memoryMb
        await serversApi.updateSettings(props.server.id, {
            name: name.value.trim(),
            javaPath: javaSelection.value.path,
            memoryMb: memoryMbValue,
            jvmArgs,
            preLaunchHook: preLaunchHookText.value,
            stopCommand: stopCommandValue,
            homePinned: pinnedToHome.value,
        })
        if (iconPath.value !== baseline.value.iconPath) {
            await serversApi.setIcon(props.server.id, iconPath.value)
        }
        const propsSaved = (await editor.value?.save()) ?? true
        if (!propsSaved) return
        name.value = name.value.trim()
        memoryMb.value = memoryMbValue
        jvmArgsText.value = jvmArgs.join(' ')
        preLaunchHookText.value = preLaunchHookText.value.trim()
        stopCommand.value = stopCommandValue
        baseline.value = {
            name: name.value,
            iconPath: iconPath.value,
            javaPath: javaSelection.value.path,
            javaVersion: javaSelection.value.version,
            memoryMb: memoryMbValue,
            jvmArgs: jvmArgsText.value,
            preLaunchHook: preLaunchHookText.value,
            stopCommand: stopCommandValue,
            homePinned: pinnedToHome.value,
        }
        await refresh()
        addNotification({ type: 'success', title: formatMessage(messages.saved) })
    } catch (error) {
        handleError(error)
    } finally {
        isSaving.value = false
    }
}

function cancel() {
    name.value = baseline.value.name
    iconPath.value = baseline.value.iconPath
    javaSelection.value = { path: baseline.value.javaPath, version: baseline.value.javaVersion }
    memoryMb.value = baseline.value.memoryMb
    jvmArgsText.value = baseline.value.jvmArgs
    preLaunchHookText.value = baseline.value.preLaunchHook
    stopCommand.value = baseline.value.stopCommand
    pinnedToHome.value = baseline.value.homePinned
    editor.value?.cancel()
}

async function pickIcon() {
    try {
        const picked = await (filePicker.pickInstanceIcon?.() ?? filePicker.pickImage())
        if (picked?.path) iconPath.value = picked.path
    } catch (error) {
        handleError(error)
    }
}

async function confirmDelete() {
    const ok = await deleteServer(props.server.id)
    if (ok) emit('deleted')
}
</script>

<template>
    <div class="flex min-h-full flex-col">
        <div class="flex flex-col gap-6 pb-20">
            <Admonition
                v-if="isRunning"
                type="warning"
                :header="formatMessage(messages.runningTitle)"
            >
                {{ formatMessage(messages.runningHint) }}
            </Admonition>

            <Card data-onboarding-id="server-settings" class="!m-0">
                <div class="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
                    <h3
                        class="col-span-full m-0 text-base font-semibold text-[var(--color-text-primary)]"
                    >
                        {{ formatMessage(messages.general) }}
                    </h3>

                    <div class="flex min-w-0 items-center gap-3 sm:col-span-2 xl:col-span-4">
                        <ServerIcon
                            :icon-path="iconPath"
                            :server-type="server.serverType"
                            :server-id="server.id"
                            size="48px"
                        />
                        <div class="flex flex-col gap-1">
                            <span class="font-semibold text-[var(--color-text-primary)]">{{
                                formatMessage(messages.icon)
                            }}</span>
                            <div class="flex gap-2">
                                <Button type="outlined" size="2xs" @click="pickIcon"
                                    ><ImageIcon />
                                    {{
                                        iconPath
                                            ? formatMessage(messages.changeIcon)
                                            : formatMessage(messages.selectIcon)
                                    }}
                                </Button>
                                <Button
                                    v-if="iconPath"
                                    type="outlined"
                                    color="red"
                                    size="2xs"
                                    @click="iconPath = null"
                                    ><TrashIcon />
                                    {{ formatMessage(messages.removeIcon) }}
                                </Button>
                            </div>
                        </div>
                    </div>

                    <label class="flex min-w-0 flex-col gap-2" for="server-settings-name">
                        <span class="font-semibold text-[var(--color-text-primary)]">{{
                            formatMessage(messages.name)
                        }}</span>
                        <StyledInput id="server-settings-name" v-model="name" />
                    </label>

                    <label class="flex min-w-0 flex-col gap-2" for="server-settings-memory">
                        <span class="font-semibold text-[var(--color-text-primary)]">{{
                            formatMessage(messages.memory)
                        }}</span>
                        <StyledInput
                            id="server-settings-memory"
                            v-model="memoryMb"
                            inputmode="numeric"
                            wrapper-class="max-w-40"
                        />
                    </label>

                    <div class="flex min-w-0 flex-col gap-2 sm:col-span-2 xl:col-span-2">
                        <span class="font-semibold text-[var(--color-text-primary)]">{{
                            formatMessage(messages.java)
                        }}</span>
                        <JavaSelector
                            id="server-settings-java"
                            v-model="javaSelection"
                            :version="requiredJava"
                            select-all-versions
                        />
                    </div>

                    <label
                        class="flex min-w-0 flex-col gap-2 sm:col-span-2 xl:col-span-4"
                        for="server-settings-jvm"
                    >
                        <span class="font-semibold text-[var(--color-text-primary)]">{{
                            formatMessage(messages.jvmArgs)
                        }}</span>
                        <StyledInput id="server-settings-jvm" v-model="jvmArgsText" />
                        <span class="text-xs text-[var(--color-text-tertiary)]">{{
                            formatMessage(messages.jvmArgsHint)
                        }}</span>
                    </label>

                    <label
                        class="flex min-w-0 flex-col gap-2 sm:col-span-2 xl:col-span-4"
                        for="server-settings-pre-launch-hook"
                    >
                        <span class="font-semibold text-[var(--color-text-primary)]">{{
                            formatMessage(messages.preLaunchHook)
                        }}</span>
                        <StyledInput
                            id="server-settings-pre-launch-hook"
                            v-model="preLaunchHookText"
                        />
                        <span class="text-xs text-[var(--color-text-tertiary)]">{{
                            formatMessage(messages.preLaunchHookHint)
                        }}</span>
                    </label>
                    <label
                        class="flex min-w-0 flex-col gap-2 sm:col-span-2 xl:col-span-4"
                        for="server-settings-stop-command"
                    >
                        <span class="font-semibold text-[var(--color-text-primary)]">{{
                            formatMessage(messages.stopCommand)
                        }}</span>
                        <StyledInput
                            id="server-settings-stop-command"
                            v-model="stopCommand"
                            :disabled="isSaving"
                            placeholder="stop"
                            :spellcheck="false"
                            autocomplete="off"
                        />
                        <span class="text-xs text-[var(--color-text-tertiary)]">{{
                            formatMessage(messages.stopCommandHint)
                        }}</span>
                    </label>
                    <div
                        class="flex min-w-0 items-center justify-between gap-3 rounded-xl border border-solid border-surface-5 bg-surface-4 p-3 sm:col-span-2 xl:col-span-4"
                    >
                        <span class="font-semibold text-[var(--color-text-primary)]">{{
                            formatMessage(messages.pinHome)
                        }}</span>
                        <Toggle id="server-settings-pin-home" v-model="pinnedToHome" />
                    </div>
                </div>
            </Card>

            <Card class="!m-0">
                <ServerPropertiesEditor ref="editor" :server-id="server.id" />
            </Card>

            <Card class="!m-0">
                <div class="flex flex-wrap items-center justify-between gap-3">
                    <div class="flex min-w-0 items-start gap-3">
                        <div
                            class="bg-red-highlight flex size-9 shrink-0 items-center justify-center rounded-lg text-red"
                        >
                            <TrashIcon class="size-4" />
                        </div>
                        <div class="min-w-0">
                            <h3
                                class="m-0 text-base font-semibold text-[var(--color-text-primary)]"
                            >
                                {{ formatMessage(messages.deleteTitle) }}
                            </h3>
                            <p class="mb-0 mt-1 text-sm text-[var(--color-text-tertiary)]">
                                {{ formatMessage(messages.deleteHint) }}
                            </p>
                        </div>
                    </div>
                    <Button
                        type="outlined"
                        color="red"
                        :disabled="server.running || server.starting"
                        @click="deleteModal?.show()"
                        ><TrashIcon />
                        {{ formatMessage(messages.deleteTitle) }}
                    </Button>
                </div>
            </Card>
        </div>

        <div
            v-if="isDirty"
            class="fixed bottom-4 z-50 flex"
            :style="{
                left: 'calc(var(--left-bar-width) + 1.5rem)',
                width: 'calc(100% - var(--left-bar-width) - var(--right-bar-width) - 3rem)',
            }"
        >
            <div class="flex w-full items-center justify-end">
                <div
                    class="flex items-center gap-2 rounded-xl border border-solid border-surface-4 bg-surface-3 px-3 py-2 shadow-lg"
                >
                    <Button type="outlined" :disabled="isSaving" @click="cancel"
                        ><XIcon />
                        {{ formatMessage(messages.cancel) }}
                    </Button>
                    <Button type="colored" color="brand" :disabled="saveDisabled" @click="save"
                        ><SpinnerIcon v-if="isSaving" class="animate-spin" />
                        <SaveIcon v-else />
                        {{ formatMessage(messages.save) }}
                    </Button>
                </div>
            </div>
        </div>

        <ConfirmModal
            ref="deleteModal"
            :title="formatMessage(messages.deleteTitle)"
            :description="formatMessage(messages.deleteConfirm, { name: server.name })"
            :proceed-label="formatMessage(messages.deleteProceed)"
            @proceed="confirmDelete"
        />
    </div>
</template>
