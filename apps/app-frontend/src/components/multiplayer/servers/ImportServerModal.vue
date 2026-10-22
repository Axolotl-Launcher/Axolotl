<script setup lang="ts">
import { requiredJavaMajorVersion, SERVER_TYPES, type ServerTypeId } from '@modrinth/server'
import {
    Button,
    commonMessages,
    defineMessages,
    NewModal,
    Slider,
    StyledInput,
    useVIntl,
} from '@modrinth/ui'
import { computed, ref } from 'vue'
import { useRouter } from 'vue-router'

import JavaSelector from '@/components/ui/JavaSelector.vue'
import { refresh } from '@/composables/useServers'
import { get_max_memory } from '@/helpers/jre'
import { type ServerImportPreview, servers } from '@/helpers/servers'

import { findServerJava, javaMajorFromVersion, toErrorMessage } from './server-flow-utils'

const { formatMessage } = useVIntl()
const router = useRouter()
const modal = ref<InstanceType<typeof NewModal> | null>(null)
const preview = ref<ServerImportPreview | null>(null)
const sourcePath = ref('')
const name = ref('')
const java = ref({ path: '', version: '' })
const memory = ref(4096)
const maxMemory = ref(8192)
const busy = ref(false)
const inspecting = ref(false)
const error = ref('')
const messages = defineMessages({
    title: { id: 'app.servers.import.title', defaultMessage: 'Import server' },
    name: { id: 'app.servers.wizard.name', defaultMessage: 'Server name' },
    java: { id: 'app.servers.settings.java', defaultMessage: 'Java' },
    memory: { id: 'app.servers.settings.memory', defaultMessage: 'Memory' },
    memoryValue: { id: 'app.servers.wizard.memory-value', defaultMessage: '{value} MB' },
    version: {
        id: 'app.servers.import.version',
        defaultMessage: 'Minecraft {gameVersion} · {serverType}',
    },
    loader: { id: 'app.servers.import.loader', defaultMessage: 'Loader {version}' },
    ready: {
        id: 'app.servers.import.files-present',
        defaultMessage: 'Server files are present. The server will stay stopped after import.',
    },
    install: {
        id: 'app.servers.import.install-needed',
        defaultMessage:
            'Server dependencies need to be installed. The server will stay stopped after import.',
    },
    submit: { id: 'app.servers.import.submit', defaultMessage: 'Import' },
    installSubmit: {
        id: 'app.servers.import.install-submit',
        defaultMessage: 'Import and install',
    },
    working: { id: 'app.servers.import.working', defaultMessage: 'Importing server…' },
    inspecting: { id: 'app.servers.import.inspecting', defaultMessage: 'Checking server files…' },
    none: {
        id: 'app.servers.import.not-found',
        defaultMessage:
            'No supported server was found. Choose a folder or ZIP containing the server launcher and its files.',
    },
    missing: {
        id: 'app.servers.import.missing-version',
        defaultMessage:
            'The server version could not be determined. Include its original launcher or installer and try again.',
    },
    conflict: {
        id: 'app.servers.import.conflicting-versions',
        defaultMessage:
            'This package contains conflicting server versions. Keep the files for one server version and try again.',
    },
    multiple: {
        id: 'app.servers.import.multiple-launchers',
        defaultMessage:
            'This package has multiple server launchers. Keep the launcher you want to use and try again.',
    },
    client: {
        id: 'app.servers.import.client-server-conflict',
        defaultMessage:
            'This folder contains both client instance and server files. Choose a separate server folder.',
    },
    unsupported: {
        id: 'app.servers.import.unsupported-type',
        defaultMessage: 'This server type cannot be imported yet.',
    },
    javaRequired: {
        id: 'app.servers.import.java-required',
        defaultMessage: 'Select a Java installation compatible with this Minecraft version.',
    },
    failed: {
        id: 'app.servers.import.failed',
        defaultMessage:
            'Installation did not finish. Your imported files were kept. Open the server to review the error and retry installation.',
    },
    view: { id: 'app.servers.import.view-server', defaultMessage: 'View server' },
})
const importedId = ref('')
const requiredJava = computed(() => requiredJavaMajorVersion(preview.value?.gameVersion || ''))
const serverTypeLabel = computed(() =>
    preview.value
        ? (SERVER_TYPES[preview.value.serverType as ServerTypeId]?.label ??
          preview.value.serverType)
        : '',
)
const problemMessage = computed(() => {
    const problem = preview.value?.problem
    const map = {
        'missing-version': messages.missing,
        'conflicting-versions': messages.conflict,
        'multiple-launchers': messages.multiple,
        'client-server-conflict': messages.client,
        'unsupported-type': messages.unsupported,
    }
    return problem ? formatMessage(map[problem as keyof typeof map] ?? messages.none) : ''
})
const canImport = computed(
    () =>
        !busy.value &&
        !inspecting.value &&
        !!preview.value &&
        !preview.value.problem &&
        !!name.value.trim() &&
        !!java.value.path.trim() &&
        !importedId.value,
)

async function show(path: string) {
    if (busy.value || inspecting.value) return
    sourcePath.value = path
    preview.value = null
    error.value = ''
    importedId.value = ''
    java.value = { path: '', version: '' }
    name.value = (path.split(/[/\\]/).pop() || 'server').replace(/\.zip$/i, '')
    inspecting.value = true
    modal.value?.show()
    try {
        preview.value = await servers.inspectImport(path)
        if (!preview.value) {
            error.value = formatMessage(messages.none)
            return
        }
        if (preview.value.problem) return
        const found = await findServerJava(preview.value.gameVersion)
        if (found) java.value = found
        maxMemory.value = Math.max(1024, Math.floor(Number(await get_max_memory()) / 1024))
        memory.value = Math.min(4096, maxMemory.value)
    } catch (e) {
        error.value = toErrorMessage(e)
    } finally {
        inspecting.value = false
    }
}
async function submit() {
    if (!canImport.value || !preview.value) return
    const major = javaMajorFromVersion(java.value.version)
    if (major !== null && major < requiredJava.value) {
        error.value = formatMessage(messages.javaRequired)
        return
    }
    busy.value = true
    error.value = ''
    try {
        const result = await servers.importExternal({
            sourcePath: sourcePath.value,
            expected: preview.value,
            name: name.value,
            javaPath: java.value.path,
            memoryMb: memory.value,
        })
        importedId.value = result.id
        await refresh()
        if (result.installState) error.value = formatMessage(messages.failed)
        else await viewServer()
    } catch (e) {
        error.value = toErrorMessage(e)
    } finally {
        busy.value = false
    }
}
async function viewServer() {
    modal.value?.hide()
    await router.push(`/multiplayer/servers/${encodeURIComponent(importedId.value)}`)
}
defineExpose({ show })
</script>

<template>
    <NewModal
        ref="modal"
        :header="formatMessage(messages.title)"
        :disable-close="busy || inspecting"
        width="min(36rem, calc(95vw - 2rem))"
        scrollable
    >
        <div class="flex flex-col gap-4">
            <p v-if="inspecting" role="status">{{ formatMessage(messages.inspecting) }}</p>
            <template v-if="preview">
                <div class="flex flex-col gap-1">
                    <span class="font-semibold">{{
                        formatMessage(messages.version, {
                            gameVersion: preview.gameVersion || '?',
                            serverType: serverTypeLabel,
                        })
                    }}</span>
                    <span v-if="preview.loaderVersion">{{
                        formatMessage(messages.loader, { version: preview.loaderVersion })
                    }}</span>
                    <p v-if="problemMessage" class="text-orange" role="alert">
                        {{ problemMessage }}
                    </p>
                    <p v-else>
                        {{
                            formatMessage(preview.needsInstall ? messages.install : messages.ready)
                        }}
                    </p>
                </div>
                <template v-if="!preview.problem && !importedId">
                    <label class="flex flex-col gap-2" for="import-server-name">
                        <span>{{ formatMessage(messages.name) }}</span>
                        <StyledInput id="import-server-name" v-model="name" :disabled="busy" />
                    </label>
                    <div class="flex flex-col gap-2">
                        <span>{{ formatMessage(messages.java) }}</span>
                        <JavaSelector
                            id="import-server-java"
                            v-model="java"
                            :version="requiredJava"
                            :disabled="busy"
                            select-all-versions
                        />
                    </div>
                    <div class="flex flex-col gap-2">
                        <div class="flex justify-between gap-2">
                            <span>{{ formatMessage(messages.memory) }}</span
                            ><span>{{
                                formatMessage(messages.memoryValue, { value: memory })
                            }}</span>
                        </div>
                        <Slider
                            v-model="memory"
                            :min="1024"
                            :max="maxMemory"
                            :step="512"
                            :disabled="busy"
                        />
                    </div>
                </template>
            </template>
            <p v-if="error" class="text-red" role="alert">{{ error }}</p>
            <p v-if="busy" role="status">{{ formatMessage(messages.working) }}</p>
        </div>
        <template #actions>
            <Button :disabled="busy || inspecting" @click="modal?.hide()">{{
                formatMessage(commonMessages.cancelButton)
            }}</Button>
            <Button v-if="importedId" color="brand" @click="viewServer">{{
                formatMessage(messages.view)
            }}</Button>
            <Button v-else color="brand" :disabled="!canImport" @click="submit">{{
                formatMessage(preview?.needsInstall ? messages.installSubmit : messages.submit)
            }}</Button>
        </template>
    </NewModal>
</template>
