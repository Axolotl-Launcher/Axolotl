<script setup lang="ts">
import { SpinnerIcon } from '@modrinth/assets'
import { requiredJavaMajorVersion, SERVER_TYPES, type ServerTypeId } from '@modrinth/server'
import {
    Button,
    Checkbox,
    Combobox,
    commonMessages,
    defineMessages,
    loaderVersionsForGameVersion,
    NewModal,
    Slider,
    StyledInput,
    useVIntl,
} from '@modrinth/ui'
import { computed, ref, watch } from 'vue'
import { useRouter } from 'vue-router'

import JavaSelector from '@/components/ui/JavaSelector.vue'
import { refresh } from '@/composables/useServers'
import { find_filtered_jres, get_java_default_versions, get_max_memory } from '@/helpers/jre'
import { get_game_versions, get_loader_versions } from '@/helpers/metadata'
import {
    type ServerImportConfiguration,
    type ServerImportPreview,
    servers,
} from '@/helpers/servers'

import { javaMajorFromVersion, toErrorMessage } from './server-flow-utils'

type LoaderManifest = NonNullable<Parameters<typeof loaderVersionsForGameVersion>[0]>

const serverTypeOptions = (['vanilla', 'fabric', 'forge'] as const).map((value) => ({
    value,
    label: SERVER_TYPES[value].label,
}))

const { formatMessage } = useVIntl()
const router = useRouter()
const modal = ref<InstanceType<typeof NewModal> | null>(null)
const preview = ref<ServerImportPreview | null>(null)
const sourcePath = ref('')
const sourceStopped = ref(false)
const configuredType = ref<ServerImportConfiguration['serverType'] | ''>('')
const configuredGame = ref('')
const configuredLoader = ref('')
const gameOptions = ref<Array<{ value: string; label: string }>>([])
const loaderOptions = ref<Array<{ value: string; label: string }>>([])
const versionsError = ref('')
function withCustomVersion(options: Array<{ value: string; label: string }>, value: string) {
    return value && !options.some((option) => option.value === value)
        ? [{ value, label: value }, ...options]
        : options
}
const selectableGames = computed(() => withCustomVersion(gameOptions.value, configuredGame.value))
const selectableLoaders = computed(() =>
    withCustomVersion(loaderOptions.value, configuredLoader.value),
)

watch([configuredType, configuredGame], ([type, game], _, onCleanup) => {
    java.value = { path: '', version: '' }
    configuredLoader.value = ''
    loaderOptions.value = []
    versionsError.value = ''
    if (!needsConfiguration.value || !type || type === 'vanilla' || !game.trim()) return
    let active = true
    const timer = setTimeout(async () => {
        try {
            const manifest = (await get_loader_versions(type, game.trim())) as LoaderManifest
            if (active) {
                loaderOptions.value = loaderVersionsForGameVersion(manifest, game.trim()).map(
                    ({ id }) => ({ value: id, label: id }),
                )
            }
        } catch (e) {
            if (active) versionsError.value = toErrorMessage(e)
        }
    }, 300)
    onCleanup(() => {
        active = false
        clearTimeout(timer)
    })
})

async function loadGameOptions() {
    try {
        const manifest = (await get_game_versions()) as { versions: Array<{ id: string }> }
        gameOptions.value = manifest.versions.map(({ id }) => ({ value: id, label: id }))
    } catch (e) {
        versionsError.value = toErrorMessage(e)
    }
}
const needsConfiguration = computed(() => preview.value?.problem === 'configuration-required')
const effectiveGame = computed(() =>
    needsConfiguration.value ? configuredGame.value.trim() : preview.value?.gameVersion || '',
)
const effectiveType = computed(() =>
    needsConfiguration.value ? configuredType.value : preview.value?.serverType,
)
const effectiveLoader = computed(() =>
    needsConfiguration.value ? configuredLoader.value : preview.value?.loaderVersion,
)
const exactJava = computed(() => effectiveType.value === 'forge' && requiredJava.value === 8)
const configurationComplete = computed(
    () =>
        !!configuredType.value &&
        !!configuredGame.value.trim() &&
        (configuredType.value === 'vanilla' || !!configuredLoader.value.trim()),
)
const name = ref('')
const java = ref({ path: '', version: '' })
const memory = ref(4096)
const maxMemory = ref(8192)
const busy = ref(false)
const inspecting = ref(false)
const error = ref('')
const messages = defineMessages({
    sourceStopped: {
        id: 'app.servers.import.source-stopped',
        defaultMessage:
            'The source server is stopped and will remain stopped until import finishes.',
    },
    title: { id: 'app.servers.import.title', defaultMessage: 'Import server' },
    configuration: {
        id: 'app.servers.import.configuration-required',
        defaultMessage:
            'Choose the server type and exact versions required by this package. Its launcher will be installed after import.',
    },
    type: { id: 'app.servers.wizard.type-title', defaultMessage: 'Server type' },
    gameVersion: { id: 'app.servers.wizard.game-version', defaultMessage: 'Game version' },
    loaderVersion: { id: 'app.servers.wizard.loader-version', defaultMessage: 'Loader version' },
    exactJava: {
        id: 'app.servers.import.exact-java',
        defaultMessage: 'Select Java {version} for this Forge version.',
    },
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
const requiredJava = computed(() => requiredJavaMajorVersion(effectiveGame.value))
const serverTypeLabel = computed(() =>
    effectiveType.value
        ? (SERVER_TYPES[effectiveType.value as ServerTypeId]?.label ?? effectiveType.value)
        : '',
)
const problemMessage = computed(() => {
    const problem = preview.value?.problem
    if (problem === 'configuration-required') return ''
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
        (!preview.value.sourceIsDirectory || sourceStopped.value) &&
        (!preview.value.problem || (needsConfiguration.value && configurationComplete.value)) &&
        !!name.value.trim() &&
        !!java.value.path.trim() &&
        !importedId.value,
)

/** Reuses launcher defaults and discovery, preferring the game's matching Java major. */
async function findServerJava(
    gameVersion: string,
): Promise<{ path: string; version: string } | undefined> {
    const major = requiredJavaMajorVersion(gameVersion)
    const defaults = (await get_java_default_versions().catch(() => [])) as Array<{
        parsed_version: number
        path: string
        version: string
    }>
    const exact = defaults.find((entry) => entry.parsed_version === major)
    if (exact) return { path: exact.path, version: exact.version }
    const found = (await find_filtered_jres(major).catch(() => [])) as Array<{
        path: string
        version: string
    }>
    const discovered = found.find((entry) => javaMajorFromVersion(entry.version) === major)
    return (
        discovered ??
        (exactJava.value
            ? undefined
            : (defaults.find((entry) => entry.parsed_version >= major) ?? found[0]))
    )
}

function changeConfiguredType() {
    java.value = { path: '', version: '' }
    configuredLoader.value = ''
    void loadConfiguredJava()
}

async function loadConfiguredJava() {
    if (!effectiveGame.value || busy.value || java.value.path) return
    const game = effectiveGame.value
    const type = effectiveType.value
    const found = await findServerJava(game)
    if (game === effectiveGame.value && type === effectiveType.value && !java.value.path) {
        java.value = found ?? { path: '', version: '' }
    }
}

async function show(path: string) {
    if (busy.value || inspecting.value) return
    sourceStopped.value = false
    sourcePath.value = path
    preview.value = null
    configuredType.value = ''
    configuredGame.value = ''
    configuredLoader.value = ''
    error.value = ''
    versionsError.value = ''
    gameOptions.value = []
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
        if (preview.value.problem && !needsConfiguration.value) return
        maxMemory.value = Math.max(1024, Math.floor(Number(await get_max_memory()) / 1024))
        memory.value = Math.min(4096, maxMemory.value)
        if (needsConfiguration.value) void loadGameOptions()
        else await loadConfiguredJava()
    } catch (e) {
        error.value = toErrorMessage(e)
    } finally {
        inspecting.value = false
    }
}
async function submit() {
    if (!canImport.value || !preview.value) return
    const major = javaMajorFromVersion(java.value.version)
    if (exactJava.value && major !== requiredJava.value) {
        error.value = formatMessage(messages.exactJava, { version: requiredJava.value })
        return
    }
    if (major !== null && major < requiredJava.value) {
        error.value = formatMessage(messages.javaRequired)
        return
    }
    busy.value = true
    error.value = ''
    try {
        const result = await servers.importExternal({
            sourcePath: sourcePath.value,
            sourceStopped: sourceStopped.value,
            expected: preview.value,
            ...(needsConfiguration.value
                ? {
                      configuration: {
                          serverType:
                              configuredType.value as ServerImportConfiguration['serverType'],
                          gameVersion: configuredGame.value.trim(),
                          loaderVersion:
                              configuredType.value === 'vanilla'
                                  ? null
                                  : configuredLoader.value.trim(),
                      },
                  }
                : {}),
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
            <p v-if="inspecting" role="status" class="flex items-center gap-2">
                <SpinnerIcon class="size-5 shrink-0 animate-spin" aria-hidden="true" />
                {{ formatMessage(messages.inspecting) }}
            </p>
            <template v-if="preview">
                <Checkbox
                    v-if="preview.sourceIsDirectory && !importedId"
                    v-model="sourceStopped"
                    :disabled="busy"
                    :label="formatMessage(messages.sourceStopped)"
                />
                <div v-if="needsConfiguration && !importedId" class="flex flex-col gap-3">
                    <p>{{ formatMessage(messages.configuration) }}</p>
                    <div class="flex flex-col gap-2">
                        <span>{{ formatMessage(messages.type) }}</span>
                        <Combobox
                            id="import-server-type"
                            v-model="configuredType"
                            :disabled="busy"
                            :options="serverTypeOptions"
                            @update:model-value="changeConfiguredType"
                        />
                    </div>
                    <label class="flex flex-col gap-2" for="import-server-game">
                        <span>{{ formatMessage(messages.gameVersion) }}</span>
                        <Combobox
                            id="import-server-game"
                            v-model="configuredGame"
                            :options="selectableGames"
                            :search-placeholder="formatMessage(messages.gameVersion)"
                            searchable
                            sync-with-selection
                            :search-value="configuredGame"
                            :disabled="busy"
                            @search-input="configuredGame = $event"
                            @update:model-value="java = { path: '', version: '' }"
                            @search-blur="loadConfiguredJava"
                        />
                    </label>
                    <label
                        v-if="configuredType && configuredType !== 'vanilla'"
                        class="flex flex-col gap-2"
                        for="import-server-loader"
                    >
                        <span>{{ formatMessage(messages.loaderVersion) }}</span>
                        <Combobox
                            id="import-server-loader"
                            v-model="configuredLoader"
                            :options="selectableLoaders"
                            :search-placeholder="formatMessage(messages.loaderVersion)"
                            searchable
                            sync-with-selection
                            :search-value="configuredLoader"
                            :disabled="busy"
                            @search-input="configuredLoader = $event"
                        />
                    </label>
                    <p v-if="versionsError" class="text-orange" role="alert">{{ versionsError }}</p>
                </div>
                <div v-else class="flex flex-col gap-1">
                    <span class="font-semibold">{{
                        formatMessage(messages.version, {
                            gameVersion: effectiveGame || '?',
                            serverType: serverTypeLabel,
                        })
                    }}</span>
                    <span v-if="effectiveLoader">{{
                        formatMessage(messages.loader, { version: effectiveLoader })
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
                <template v-if="(!preview.problem || needsConfiguration) && !importedId">
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
            <p v-if="busy" role="status" class="flex items-center gap-2">
                <SpinnerIcon class="size-5 shrink-0 animate-spin" aria-hidden="true" />
                {{ formatMessage(messages.working) }}
            </p>
        </div>
        <template #actions>
            <Button :disabled="busy || inspecting" @click="modal?.hide()">{{
                formatMessage(commonMessages.cancelButton)
            }}</Button>
            <Button v-if="importedId" type="colored" color="brand" @click="viewServer">{{
                formatMessage(messages.view)
            }}</Button>
            <Button v-else type="colored" color="brand" :disabled="!canImport" @click="submit">{{
                formatMessage(preview?.needsInstall ? messages.installSubmit : messages.submit)
            }}</Button>
        </template>
    </NewModal>
</template>
