<script setup lang="ts">
import type { Labrinth } from '@modrinth/api-client'
import { CheckIcon, DownloadIcon, XIcon } from '@modrinth/assets'
import {
    Avatar,
    Badge,
    BUILTIN_GAME_DIRECTORY,
    Button,
    Combobox,
    commonMessages,
    type ConfiguredMinecraftDirectory,
    defineMessages,
    GameDirectoryPicker,
    injectNotificationManager,
    loadMinecraftDirectories,
    NewModal,
    StyledInput,
    useVIntl,
} from '@modrinth/ui'
import { join } from '@tauri-apps/api/path'
import { computed, ref } from 'vue'

import { releaseColor } from '@/helpers/utils'

export interface ModpackInstallExistingInstance {
    id: string
    name: string
}

export interface ModpackInstallModalData {
    project: Pick<Labrinth.Projects.v2.Project, 'id' | 'title' | 'icon_url'>
    versions: Labrinth.Versions.v2.Version[]
    initialVersionId?: string | null
    existingInstancesByVersion: Record<string, ModpackInstallExistingInstance[]>
    instancePaths: string[]
}

const emit = defineEmits<{
    install: [versionId: string, name: string, gameDirOverride: string | null]
    cancel: []
}>()

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
let generation = 0
const modal = ref<InstanceType<typeof NewModal>>()
const data = ref<ModpackInstallModalData | null>(null)
const selectedVersionId = ref('')
const instanceName = ref('')
const configuredDirectories = ref<ConfiguredMinecraftDirectory[]>([])
const selectedGameDirectory = ref(BUILTIN_GAME_DIRECTORY)
const submitting = ref(false)
const submitted = ref(false)

const messages = defineMessages({
    title: {
        id: 'app.modpack-install.title',
        defaultMessage: 'Install modpack',
    },
    version: {
        id: 'app.modpack-install.version',
        defaultMessage: 'Version',
    },
    instanceName: {
        id: 'app.modpack-install.instance-name',
        defaultMessage: 'Instance name',
    },
    instanceNamePlaceholder: {
        id: 'app.modpack-install.instance-name-placeholder',
        defaultMessage: 'My modpack instance',
    },
    installed: {
        id: 'app.modpack-install.already-installed',
        defaultMessage: 'This version is already installed',
    },
    installedDescription: {
        id: 'app.modpack-install.already-installed-description',
        defaultMessage:
            'Creating another instance will not change these existing instances: {names}',
    },
    install: {
        id: 'app.modpack-install.install',
        defaultMessage: 'Install',
    },
    selectVersion: {
        id: 'app.modpack-install.select-version',
        defaultMessage: 'Select a version',
    },
    folderName: {
        id: 'app.modpack-install.folder-name',
        defaultMessage: 'Instance folder: {name}',
    },
    folderNameConflict: {
        id: 'app.modpack-install.folder-name-conflict',
        defaultMessage:
            'A folder with this name already exists. The instance folder will be: {name}',
    },
})

const selectedVersion = computed(() =>
    data.value?.versions.find((version) => version.id === selectedVersionId.value),
)

const versionOptions = computed(() =>
    (data.value?.versions ?? []).map((version) => ({
        value: version.id,
        label: versionLabel(version),
    })),
)

const existingInstances = computed(
    () => data.value?.existingInstancesByVersion[selectedVersionId.value] ?? [],
)

const folderName = computed(() => {
    const sanitized = instanceName.value.trim().replace(/[\\/?*:'"|<>!]/g, '_')
    const baseName = sanitized === '.' || sanitized === '..' ? '_' : sanitized
    let candidate = baseName
    let index = 1
    while (candidate && data.value?.instancePaths.includes(candidate)) {
        candidate = `${baseName} (${index++})`
    }
    return candidate
})

const hasFolderNameConflict = computed(() => folderName.value !== instanceName.value.trim())

const canInstall = computed(
    () =>
        !!selectedVersion.value &&
        instanceName.value.trim().length > 0 &&
        !['.', '..'].includes(instanceName.value.trim()) &&
        !submitting.value &&
        !submitted.value,
)

function versionLabel(version: Labrinth.Versions.v2.Version) {
    return [version.version_number || version.name, version.game_versions.join(', ')]
        .filter(Boolean)
        .join(' · ')
}

function show(nextData: ModpackInstallModalData) {
    generation += 1
    configuredDirectories.value = loadMinecraftDirectories()
    selectedGameDirectory.value = BUILTIN_GAME_DIRECTORY
    data.value = nextData
    selectedVersionId.value =
        nextData.versions.find((version) => version.id === nextData.initialVersionId)?.id ??
        nextData.versions[0]?.id ??
        ''
    instanceName.value = nextData.project.title
    submitting.value = false
    submitted.value = false
    modal.value?.show()
}

function hide() {
    generation += 1
    modal.value?.hide()
}

async function submit() {
    if (!canInstall.value) return
    const submissionGeneration = generation
    const versionId = selectedVersionId.value
    const name = instanceName.value.trim()
    const folder = folderName.value
    const directory = configuredDirectories.value.find(
        (entry) => entry.path === selectedGameDirectory.value,
    )
    submitting.value = true
    try {
        const gameDirOverride = directory
            ? directory.mode === 'shared'
                ? directory.path
                : await join(directory.path, 'versions', folder)
            : null
        if (submissionGeneration !== generation) return
        submitted.value = true
        emit('install', versionId, name, gameDirOverride)
        modal.value?.hide()
    } catch (error) {
        if (submissionGeneration !== generation) return
        submitting.value = false
        handleError(error)
    }
}

function handleHide() {
    generation += 1
    if (!submitted.value) emit('cancel')
}

defineExpose({ show, hide })
</script>

<template>
    <NewModal
        ref="modal"
        :header="formatMessage(messages.title)"
        width="min(36rem, calc(95vw - 10rem))"
        max-width="36rem"
        :on-hide="handleHide"
    >
        <div v-if="data" class="flex min-w-0 flex-col gap-4">
            <div class="flex min-w-0 items-center gap-3">
                <Avatar
                    :src="data.project.icon_url"
                    :alt="data.project.title"
                    size="3rem"
                    :tint-by="data.project.title"
                    no-shadow
                />
                <span
                    class="min-w-0 truncate text-lg font-semibold text-[var(--color-text-primary)]"
                    >{{ data.project.title }}</span
                >
            </div>

            <label class="flex flex-col gap-2">
                <span class="font-semibold text-[var(--color-text-primary)]">{{
                    formatMessage(messages.version)
                }}</span>
                <Combobox
                    v-model="selectedVersionId"
                    :options="versionOptions"
                    :name="formatMessage(messages.version)"
                    :display-value="
                        selectedVersion
                            ? versionLabel(selectedVersion)
                            : formatMessage(messages.selectVersion)
                    "
                />
            </label>

            <div
                v-if="selectedVersion"
                class="flex flex-wrap items-center gap-2 text-sm text-[var(--color-text-tertiary)]"
            >
                <Badge
                    :color="releaseColor(selectedVersion.version_type)"
                    :type="selectedVersion.version_type"
                />
                <span v-if="selectedVersion.loaders.length">{{
                    selectedVersion.loaders.join(', ')
                }}</span>
                <span v-if="selectedVersion.game_versions.length">{{
                    selectedVersion.game_versions.join(', ')
                }}</span>
            </div>

            <label class="flex flex-col gap-2">
                <span class="font-semibold text-[var(--color-text-primary)]">{{
                    formatMessage(messages.instanceName)
                }}</span>
                <StyledInput
                    v-model="instanceName"
                    :placeholder="formatMessage(messages.instanceNamePlaceholder)"
                    autocomplete="off"
                    wrapper-class="w-full"
                />
            </label>

            <p v-if="folderName" class="m-0 text-sm text-[var(--color-text-tertiary)]">
                {{
                    formatMessage(
                        hasFolderNameConflict ? messages.folderNameConflict : messages.folderName,
                        {
                            name: folderName,
                        },
                    )
                }}
            </p>

            <GameDirectoryPicker
                v-if="configuredDirectories.length"
                v-model="selectedGameDirectory"
                :directories="configuredDirectories"
            />

            <div
                v-if="existingInstances.length"
                class="flex items-start gap-2 rounded-lg border border-warning bg-warning-bg p-3"
            >
                <CheckIcon class="mt-0.5 shrink-0" />
                <div class="min-w-0">
                    <p class="m-0 font-semibold text-[var(--color-text-primary)]">
                        {{ formatMessage(messages.installed) }}
                    </p>
                    <p class="mt-1 mb-0 text-sm text-[var(--color-text-tertiary)]">
                        {{
                            formatMessage(messages.installedDescription, {
                                names: existingInstances
                                    .map((instance) => instance.name)
                                    .join(', '),
                            })
                        }}
                    </p>
                </div>
            </div>
        </div>

        <template #actions>
            <div class="flex justify-end gap-2">
                <Button type="outlined" :disabled="submitting" @click="hide"
                    ><XIcon />
                    {{ formatMessage(commonMessages.cancelButton) }}
                </Button>
                <Button type="colored" color="brand" :disabled="!canInstall" @click="submit"
                    ><DownloadIcon />
                    {{ formatMessage(messages.install) }}
                </Button>
            </div>
        </template>
    </NewModal>
</template>
