<script setup lang="ts">
import BoxIcon from '@modrinth/assets/icons/box.svg?component'
import FolderOpenIcon from '@modrinth/assets/icons/folder-open.svg?component'
import FolderSearchIcon from '@modrinth/assets/icons/folder-search.svg?component'
import PlusIcon from '@modrinth/assets/icons/plus.svg?component'
import TrashIcon from '@modrinth/assets/icons/trash.svg?component'
import IconButton from '@modrinth/ui/src/components/base/buttons/IconButton.vue'
import Combobox from '@modrinth/ui/src/components/base/Combobox.vue'
import Slider from '@modrinth/ui/src/components/base/Slider.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import Toggle from '@modrinth/ui/src/components/base/Toggle.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { computed, ref, watch } from 'vue'

import { desktopAppMessages } from '../../../lib/messages'
import SettingsRow from '../components/SettingsRow.vue'
import SettingsSection from '../components/SettingsSection.vue'
import { messages } from '../messages'
import { type DownloadSourceMode, useSettingsPage } from '../state'
import { type ExternalMinecraftRoot, useDirectLinkSync } from './resources/direct-links'
import MacFinderWindow from './resources/MacFinderWindow.vue'
import MacOpenPanel from './resources/MacOpenPanel.vue'
import { type ProxyConfig, useProxySettings } from './resources/proxy'
import {
    type CurseForgeCapability,
    type ResourceSourceId,
    useResourceStore,
} from './resources/resource-store'

/**
 * Resource management (apps/app-frontend/src/components/ui/settings/ResourceManagementSettings.vue),
 * shared by three categories through `scope`: Content & downloads (sources,
 * download behaviour, DNS over HTTPS, missing content), Network & multiplayer
 * (Mojang authentication, proxy) and Storage & backups (data directories).
 *
 * Settings write straight into the window's settings, where the app saves a
 * patch of every edit. Native work is simulated: the folder dialogs are the
 * macOS open panel over the preview Mac's disk, "Open backups folder" opens
 * the folder in a Finder window, the `.minecraft` roots reconcile direct-link
 * instances into the library, and the proxy is validated, stored and tested
 * as the Rust commands do.
 */
const props = withDefaults(
    defineProps<{
        scope?: 'content-downloads' | 'network-multiplayer' | 'storage-backups'
    }>(),
    { scope: 'content-downloads' },
)

const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const { settings, simulate } = useSettingsPage()
const resources = useResourceStore()
const proxy = useProxySettings()
const directLinks = useDirectLinkSync()

const openPanel = ref<InstanceType<typeof MacOpenPanel> | null>(null)
const finderWindow = ref<InstanceType<typeof MacFinderWindow> | null>(null)

// `settings_get`, `resource_preferred_source`, `is_portable_mode`, `proxy_get`.
await simulate(60)

const missingContentScannerSettings = ref(resources.getMissingContentScannerSettings())

// CurseForge API key
const curseforgeCapability = ref<CurseForgeCapability | null>(null)
const curseforgeApiKeyInput = ref('')
const curseforgeApiKeySaved = ref('')
const curseforgeApiKeySaving = ref(false)
const curseforgeApiKeyVerifying = ref(false)
const curseforgeApiKeyResult = ref('')

if (props.scope === 'content-downloads') {
    try {
        const [capability, override] = await Promise.all([
            resources.getCurseForgeCapability(),
            resources.getCurseForgeApiKeyOverride(),
        ])
        curseforgeCapability.value = capability
        curseforgeApiKeyInput.value = override ?? ''
        curseforgeApiKeySaved.value = override ?? ''
    } catch (error) {
        console.error('Failed to load the CurseForge API key', error)
    }
}

const curseforgeApiKeyDirty = computed(
    () => curseforgeApiKeyInput.value.trim() !== curseforgeApiKeySaved.value,
)

const curseforgeApiKeyStatus = computed(() => {
    if (curseforgeCapability.value?.status === 'unauthorized') {
        return formatMessage(messages.appSettingsResourcesCurseforgeApiKeyStatusUnauthorized)
    }
    switch (curseforgeCapability.value?.key_source) {
        case 'override':
            return formatMessage(messages.appSettingsResourcesCurseforgeApiKeyStatusOverride)
        case 'launcher':
            return formatMessage(messages.appSettingsResourcesCurseforgeApiKeyStatusLauncher)
        default:
            return formatMessage(messages.appSettingsResourcesCurseforgeApiKeyStatusMissing)
    }
})

async function writeCurseForgeApiKey(apiKey: string | null) {
    const submittedInput = curseforgeApiKeyInput.value
    curseforgeApiKeySaving.value = true
    curseforgeApiKeyResult.value = ''
    try {
        curseforgeCapability.value = await resources.setCurseForgeApiKeyOverride(apiKey)
        curseforgeApiKeySaved.value = apiKey ?? ''
        // Keep edits made while the request was running instead of replacing
        // them with the key that was just stored.
        if (curseforgeApiKeyInput.value === submittedInput) {
            curseforgeApiKeyInput.value = apiKey ?? ''
        }
    } catch (error) {
        handleError(error as Error)
    } finally {
        curseforgeApiKeySaving.value = false
    }
}

function saveCurseForgeApiKey() {
    return writeCurseForgeApiKey(curseforgeApiKeyInput.value.trim() || null)
}

/** `curseforge_validate_credentials` asks CurseForge; the preview sends no request. */
function verifyCurseForgeApiKey() {
    curseforgeApiKeyResult.value = formatMessage(desktopAppMessages.onlineService)
}

watch(curseforgeApiKeyInput, () => {
    curseforgeApiKeyResult.value = ''
})
const minecraftDirectories = ref<ExternalMinecraftRoot[]>(resources.loadMinecraftDirectories())
const minecraftDirectoryError = ref<string | null>(null)
const preferredResourceSource = ref<ResourceSourceId>('modrinth')
try {
    preferredResourceSource.value = await resources.resourcePreferredSource()
} catch (error) {
    handleError(error)
}
const savingResourceSource = ref(false)
const resourceSourceOptions: { value: ResourceSourceId; label: string }[] = [
    { value: 'modrinth', label: 'Modrinth' },
    { value: 'curseforge', label: 'CurseForge' },
    { value: 'mcarchive', label: 'MCArchive' },
    { value: 'planet_minecraft', label: 'Planet Minecraft' },
]
const resourceSourceIds: ResourceSourceId[] = [
    'modrinth',
    'curseforge',
    'mcarchive',
    'planet_minecraft',
]
const resourceSourceChoice = computed({
    get: () => preferredResourceSource.value,
    set: (value: ResourceSourceId) => {
        void saveResourceSource(value)
    },
})
async function saveResourceSource(value: ResourceSourceId) {
    savingResourceSource.value = true
    try {
        const source = resourceSourceIds.includes(value) ? value : resourceSourceIds[0]
        await resources.resourceSetPreferredSource(source)
        preferredResourceSource.value = source
    } catch (error) {
        handleError(error)
    } finally {
        savingResourceSource.value = false
    }
}

/** The installed macOS app keeps its data in Application Support; only a portable build is fixed. */
const isPortable = ref(false)

// Proxy settings
const proxyConfig = ref<ProxyConfig>({ mode: 'none', url: '', username: '', password: '' })
const proxyTesting = ref(false)
const proxyTestResult = ref('')

try {
    proxyConfig.value = await proxy.getProxyConfig()
} catch (error) {
    console.error('Failed to load proxy configuration', error)
}

async function saveProxyConfig(silent = true) {
    try {
        await proxy.setProxyConfig(proxyConfig.value)
    } catch (error) {
        if (!silent) {
            handleError(error)
        }
    }
}

async function testProxy() {
    if (proxyConfig.value.mode === 'custom' && !proxyConfig.value.url?.trim()) {
        proxyTestResult.value = formatMessage(messages.appSettingsResourcesProxyUrlRequired)
        return
    }
    proxyTesting.value = true
    proxyTestResult.value = ''
    try {
        const result = await proxy.testProxyConfig(proxyConfig.value)
        if (result.success) {
            proxyTestResult.value =
                result.latency_ms != null
                    ? formatMessage(messages.appSettingsResourcesProxyTestSuccessLatency, {
                          latency: result.latency_ms,
                      })
                    : formatMessage(messages.appSettingsResourcesProxyTestSuccess)
        } else {
            proxyTestResult.value =
                result.message || formatMessage(messages.appSettingsResourcesProxyTestFailed)
        }
    } catch (error) {
        const message =
            error instanceof Error
                ? error.message
                : typeof error === 'object' && error !== null && 'message' in error
                  ? String(error.message)
                  : typeof error === 'string'
                    ? error
                    : JSON.stringify(error)
        proxyTestResult.value = message
    } finally {
        proxyTesting.value = false
    }
}

type DownloadSourceSetting =
    | 'minecraft_metadata_source'
    | 'minecraft_file_source'
    | 'modrinth_source'
    | 'curseforge_source'
    | 'mojang_auth_source'

function downloadSourceModel(setting: DownloadSourceSetting) {
    return computed({
        get: () => settings[setting],
        set: (source: DownloadSourceMode) => {
            settings[setting] = source
        },
    })
}

const minecraftMetadataSource = downloadSourceModel('minecraft_metadata_source')
const minecraftFileSource = downloadSourceModel('minecraft_file_source')
const modrinthDownloadSource = downloadSourceModel('modrinth_source')
const curseforgeDownloadSource = downloadSourceModel('curseforge_source')
const automaticSourceOption = computed(() => ({
    value: 'auto' as const,
    label: formatMessage(messages.appSettingsResourcesSourceAutomatic),
}))
const officialPreferredSourceOption = computed(() => ({
    value: 'official_preferred' as const,
    label: formatMessage(messages.appSettingsResourcesSourceOfficialPreferred),
}))
const officialOnlySourceOption = computed(() => ({
    value: 'official_only' as const,
    label: formatMessage(messages.appSettingsResourcesSourceOfficialOnly),
}))
const minecraftSourceOptions = computed<{ value: DownloadSourceMode; label: string }[]>(() => [
    automaticSourceOption.value,
    officialPreferredSourceOption.value,
    {
        value: 'mirror_preferred',
        label: formatMessage(messages.appSettingsResourcesSourceOpenBmclApi),
    },
    officialOnlySourceOption.value,
])
const minecraftDirectoryModeOptions = computed(() => [
    {
        value: 'automatic' as const,
        label: formatMessage(messages.appSettingsResourcesMinecraftDirectoryModeAutomatic),
    },
    {
        value: 'isolated' as const,
        label: formatMessage(messages.appSettingsResourcesMinecraftDirectoryModeIsolated),
    },
    {
        value: 'shared' as const,
        label: formatMessage(messages.appSettingsResourcesMinecraftDirectoryModeShared),
    },
])
const modrinthSourceOptions = computed<{ value: DownloadSourceMode; label: string }[]>(() => [
    automaticSourceOption.value,
    officialPreferredSourceOption.value,
    officialOnlySourceOption.value,
])
const curseforgeSourceOptions = computed<{ value: DownloadSourceMode; label: string }[]>(() => [
    automaticSourceOption.value,
    officialPreferredSourceOption.value,
    officialOnlySourceOption.value,
])
const mojangAuthSource = downloadSourceModel('mojang_auth_source')
const mojangAuthSourceOptions = computed<{ value: DownloadSourceMode; label: string }[]>(() => [
    automaticSourceOption.value,
    {
        value: 'official_preferred',
        label: formatMessage(messages.appSettingsResourcesSourceMojangOfficialPreferred),
    },
    {
        value: 'mirror_preferred',
        label: formatMessage(messages.appSettingsResourcesSourceMojangMirrorPreferred),
    },
    {
        value: 'official_only',
        label: formatMessage(messages.appSettingsResourcesSourceMojangOfficialOnly),
    },
])
const downloadConcurrencyMode = computed({
    get: () => (settings.auto_concurrent_downloads ? 'auto' : 'manual'),
    set: (mode: 'auto' | 'manual') => {
        settings.auto_concurrent_downloads = mode === 'auto'
    },
})
const downloadConcurrencyOptions = computed(() => [
    {
        value: 'auto' as const,
        label: formatMessage(messages.appSettingsResourcesSourceAutomatic),
    },
    {
        value: 'manual' as const,
        label: formatMessage(messages.appSettingsResourcesConcurrencyManual),
    },
])
const proxyModeOptions = computed(() => [
    {
        value: 'none' as const,
        label: formatMessage(messages.appSettingsResourcesProxyModeNone),
    },
    {
        value: 'system' as const,
        label: formatMessage(messages.appSettingsResourcesProxyModeSystem),
    },
    {
        value: 'custom' as const,
        label: formatMessage(messages.appSettingsResourcesProxyModeCustom),
    },
])

const appDirectoryDescriptionText = computed(() =>
    isPortable.value
        ? formatMessage(messages.appSettingsResourcesAppDirectoryDescriptionPortable)
        : formatMessage(messages.appSettingsResourcesAppDirectoryDescription),
)

watch(
    minecraftDirectories,
    (value) => {
        resources.persistMinecraftDirectories(value)
        void syncDirectLinkInstances()
    },
    { deep: true },
)

async function syncDirectLinkInstances() {
    try {
        await directLinks.sync(minecraftDirectories.value)
    } catch (error) {
        handleError(error)
    }
}

watch(
    missingContentScannerSettings,
    (value) => {
        resources.setMissingContentScannerSettings(value)
        void resources
            .configureCurseForgeManualDownloadWatcher(value.enabled, value.directory)
            .catch(handleError)
    },
    { deep: true },
)

watch(
    proxyConfig,
    async () => {
        await saveProxyConfig(true)
    },
    { deep: true },
)

async function openDbBackupsFolder() {
    try {
        finderWindow.value?.reveal(await resources.appDbBackupsFolder())
    } catch (error) {
        handleError(error)
    }
}

async function findLauncherDir() {
    const newDir = await openPanel.value?.pick({
        directory: true,
        title: formatMessage(messages.appSettingsResourcesSelectDirectory),
    })

    if (newDir) {
        settings.custom_dir = newDir
    }
}

async function findMissingContentImportDirectory() {
    const directory = await openPanel.value?.pick({
        directory: true,
        title: formatMessage(messages.appSettingsResourcesSelectImportDirectory),
    })
    if (typeof directory === 'string') {
        missingContentScannerSettings.value.directory = directory
    }
}

function resetMissingContentImportDirectory() {
    missingContentScannerSettings.value.directory = null
}

async function addMinecraftDirectory() {
    minecraftDirectoryError.value = null
    const directory = await openPanel.value?.pick({
        directory: true,
        title: formatMessage(messages.appSettingsResourcesSelectMinecraftDirectory),
    })
    if (typeof directory !== 'string') return

    const normalized = directory.trim().replace(/[\\/]+$/, '')
    if (!normalized) return
    if (!minecraftDirectories.value.some((entry) => entry.path === normalized)) {
        minecraftDirectories.value.push({ path: normalized, mode: 'automatic' })
    }
}

function removeMinecraftDirectory(index: number) {
    minecraftDirectories.value.splice(index, 1)
    if (minecraftDirectoryError.value) minecraftDirectoryError.value = null
}

function validateMinecraftDirectory(_value: string) {
    minecraftDirectoryError.value = null
}
</script>

<template>
    <div class="flex flex-col gap-6">
        <SettingsSection
            v-if="props.scope === 'storage-backups'"
            :title="formatMessage(messages.appSettingsResourcesSectionsDataStorage)"
            title-id="settings-target-resources-data-storage"
        >
            <SettingsRow stacked>
                <template #label>
                    <span id="settings-target-storage-app-directory" tabindex="-1">
                        {{ formatMessage(messages.appSettingsResourcesAxolotlDataDirectory) }}
                    </span>
                </template>
                <template #description>{{ appDirectoryDescriptionText }}</template>
                <template #control>
                    <StyledInput
                        id="appDir"
                        v-model="settings.custom_dir"
                        :icon="BoxIcon"
                        type="text"
                        :disabled="isPortable"
                        wrapper-class="w-full"
                    >
                        <template #right>
                            <IconButton
                                :label="formatMessage(messages.appSettingsResourcesAppDirectory)"
                                class="ml-1.5"
                                :disabled="isPortable"
                                @click="findLauncherDir"
                            >
                                <FolderSearchIcon />
                            </IconButton>
                        </template>
                    </StyledInput>
                </template>
            </SettingsRow>
            <SettingsRow stacked>
                <template #label>
                    <span id="settings-target-storage-minecraft-directories" tabindex="-1">
                        {{ formatMessage(messages.appSettingsResourcesMinecraftDirectories) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.appSettingsResourcesMinecraftDirectoriesDescription)
                }}</template>
                <template #control>
                    <div class="flex w-full flex-col gap-2">
                        <div
                            v-for="(directory, index) in minecraftDirectories"
                            :key="`${directory}-${index}`"
                            class="flex min-w-0 items-center gap-2"
                        >
                            <StyledInput
                                :id="`minecraft-directory-${index}`"
                                v-model="minecraftDirectories[index].path"
                                :icon="BoxIcon"
                                type="text"
                                wrapper-class="min-w-0 flex-1"
                                @change="
                                    validateMinecraftDirectory(minecraftDirectories[index].path)
                                "
                            />
                            <Combobox
                                v-model="minecraftDirectories[index].mode"
                                :aria-label="
                                    formatMessage(
                                        messages.appSettingsResourcesMinecraftDirectoryMode,
                                    )
                                "
                                :options="minecraftDirectoryModeOptions"
                                class="min-w-32 flex-1"
                            />
                            <IconButton
                                :label="
                                    formatMessage(
                                        messages.appSettingsResourcesRemoveMinecraftDirectory,
                                    )
                                "
                                @click="removeMinecraftDirectory(index)"
                            >
                                <TrashIcon />
                            </IconButton>
                        </div>
                        <p v-if="minecraftDirectoryError" class="m-0 text-sm text-red">
                            {{ minecraftDirectoryError }}
                        </p>
                        <button class="btn min-w-max self-start" @click="addMinecraftDirectory">
                            <PlusIcon />
                            {{ formatMessage(messages.appSettingsResourcesAddMinecraftDirectory) }}
                        </button>
                    </div>
                </template>
            </SettingsRow>
            <SettingsRow>
                <template #label>
                    <span id="settings-target-resources-database-backups" tabindex="-1">
                        {{ formatMessage(messages.appSettingsResourcesDatabaseBackups) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.appSettingsResourcesDatabaseBackupsDescription)
                }}</template>
                <template #control>
                    <button
                        id="open-db-backups-folder"
                        class="btn min-w-max"
                        @click="openDbBackupsFolder"
                    >
                        <FolderOpenIcon />
                        {{ formatMessage(messages.appSettingsResourcesOpenBackupsFolder) }}
                    </button>
                </template>
            </SettingsRow>
        </SettingsSection>

        <SettingsSection v-if="props.scope === 'content-downloads'">
            <SettingsRow>
                <template #label>{{
                    formatMessage(messages.appResourcesPreferredSource)
                }}</template>
                <template #description>{{
                    formatMessage(messages.appResourcesPreferredSourceDescription)
                }}</template>
                <template #control
                    ><Combobox
                        v-model="resourceSourceChoice"
                        :options="resourceSourceOptions"
                        :disabled="savingResourceSource"
                /></template>
            </SettingsRow>
            <SettingsRow>
                <template #label
                    ><span id="settings-target-resources-chinese-names" tabindex="-1">{{
                        formatMessage(messages.appSettingsResourcesChineseNames)
                    }}</span></template
                >
                <template #description>{{
                    formatMessage(messages.appSettingsResourcesChineseNamesDescription)
                }}</template>
                <template #control
                    ><Toggle
                        id="resource-chinese-filenames"
                        v-model="settings.resource_chinese_filenames"
                /></template>
            </SettingsRow>
            <SettingsRow>
                <template #label
                    ><span id="settings-target-resources-preserve-names" tabindex="-1">{{
                        formatMessage(messages.appSettingsResourcesPreserveNames)
                    }}</span></template
                >
                <template #description>{{
                    formatMessage(messages.appSettingsResourcesPreserveNamesDescription)
                }}</template>
                <template #control
                    ><Toggle
                        id="resource-preserve-filenames"
                        v-model="settings.resource_preserve_filenames"
                /></template>
            </SettingsRow>
            <template #header>
                <h2
                    id="settings-target-resources-download-mirrors"
                    tabindex="-1"
                    class="m-0 text-lg font-semibold text-[var(--color-text-primary)]"
                >
                    {{ formatMessage(messages.appSettingsResourcesDownloadMirrors) }}
                </h2>
                <p class="m-0 mt-1 text-sm leading-relaxed text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.appSettingsResourcesDownloadMirrorsDescription) }}
                </p>
            </template>
            <SettingsRow>
                <template #label>{{
                    formatMessage(messages.appSettingsResourcesMinecraftMetadataSource)
                }}</template>
                <template #description>{{
                    formatMessage(messages.appSettingsResourcesMinecraftMetadataSourceDescription)
                }}</template>
                <template #control>
                    <div class="w-full">
                        <Combobox
                            v-model="minecraftMetadataSource"
                            :options="minecraftSourceOptions"
                        />
                    </div>
                </template>
            </SettingsRow>
            <SettingsRow>
                <template #label>{{
                    formatMessage(messages.appSettingsResourcesMinecraftFileSource)
                }}</template>
                <template #description>{{
                    formatMessage(messages.appSettingsResourcesMinecraftFileSourceDescription)
                }}</template>
                <template #control>
                    <div class="w-full">
                        <Combobox v-model="minecraftFileSource" :options="minecraftSourceOptions" />
                    </div>
                </template>
            </SettingsRow>
            <SettingsRow>
                <template #label>{{
                    formatMessage(messages.appSettingsResourcesModrinthMirror)
                }}</template>
                <template #description>{{
                    formatMessage(messages.appSettingsResourcesModrinthMirrorDescription)
                }}</template>
                <template #control>
                    <div class="w-full">
                        <Combobox
                            v-model="modrinthDownloadSource"
                            :options="modrinthSourceOptions"
                        />
                    </div>
                </template>
            </SettingsRow>
            <SettingsRow>
                <template #label>{{
                    formatMessage(messages.appSettingsResourcesCurseforgeMirror)
                }}</template>
                <template #description>{{
                    formatMessage(messages.appSettingsResourcesCurseforgeMirrorDescription)
                }}</template>
                <template #control>
                    <div class="w-full">
                        <Combobox
                            v-model="curseforgeDownloadSource"
                            :options="curseforgeSourceOptions"
                        />
                    </div>
                </template>
            </SettingsRow>
            <SettingsRow>
                <template #label>{{
                    formatMessage(messages.appSettingsResourcesCurseforgeRestrictionBypass)
                }}</template>
                <template #description>
                    {{
                        formatMessage(
                            messages.appSettingsResourcesCurseforgeRestrictionBypassDescription,
                        )
                    }}
                </template>
                <template #control>
                    <Toggle
                        id="curseforge-restriction-bypass"
                        v-model="settings.bypass_curseforge_download_restrictions"
                    />
                </template>
            </SettingsRow>
        </SettingsSection>

        <SettingsSection v-if="props.scope === 'content-downloads'">
            <template #header>
                <h2
                    id="settings-target-resources-curseforge-api-key"
                    tabindex="-1"
                    class="m-0 text-lg font-semibold text-[var(--color-text-primary)]"
                >
                    {{ formatMessage(messages.appSettingsResourcesCurseforgeApiKey) }}
                </h2>
                <p class="m-0 mt-1 text-sm leading-relaxed text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.appSettingsResourcesCurseforgeApiKeyDescription) }}
                </p>
            </template>
            <SettingsRow stacked>
                <template #label>{{
                    formatMessage(messages.appSettingsResourcesCurseforgeApiKeyField)
                }}</template>
                <template #description>{{ curseforgeApiKeyStatus }}</template>
                <template #control>
                    <div class="flex w-full flex-col gap-2">
                        <div class="flex w-full flex-wrap items-center gap-3">
                            <StyledInput
                                id="curseforge-api-key"
                                v-model="curseforgeApiKeyInput"
                                type="password"
                                :placeholder="
                                    formatMessage(
                                        messages.appSettingsResourcesCurseforgeApiKeyPlaceholder,
                                    )
                                "
                                wrapper-class="min-w-0 flex-1"
                            />
                            <button
                                class="btn min-w-max"
                                :disabled="
                                    curseforgeApiKeySaving ||
                                    curseforgeApiKeyVerifying ||
                                    !curseforgeApiKeyDirty
                                "
                                @click="saveCurseForgeApiKey"
                            >
                                {{
                                    formatMessage(
                                        curseforgeApiKeySaving
                                            ? messages.appSettingsResourcesCurseforgeApiKeySaving
                                            : messages.appSettingsResourcesCurseforgeApiKeySave,
                                    )
                                }}
                            </button>
                            <button
                                class="btn min-w-max"
                                :disabled="
                                    curseforgeApiKeyVerifying ||
                                    curseforgeApiKeySaving ||
                                    curseforgeApiKeyDirty ||
                                    !curseforgeCapability?.configured
                                "
                                @click="verifyCurseForgeApiKey"
                            >
                                {{
                                    formatMessage(
                                        curseforgeApiKeyVerifying
                                            ? messages.appSettingsResourcesCurseforgeApiKeyVerifying
                                            : messages.appSettingsResourcesCurseforgeApiKeyVerify,
                                    )
                                }}
                            </button>
                        </div>
                        <span
                            v-if="curseforgeApiKeyDirty"
                            class="text-sm text-[var(--color-text-tertiary)]"
                            >{{
                                formatMessage(messages.appSettingsResourcesCurseforgeApiKeyUnsaved)
                            }}</span
                        >
                        <span
                            v-else-if="curseforgeApiKeyResult"
                            class="text-sm text-[var(--color-text-tertiary)]"
                            >{{ curseforgeApiKeyResult }}</span
                        >
                    </div>
                </template>
            </SettingsRow>
        </SettingsSection>

        <SettingsSection
            v-if="props.scope === 'content-downloads'"
            :title="formatMessage(messages.appSettingsResourcesSectionsDownloadBehavior)"
            title-id="settings-target-resources-download-behavior"
        >
            <SettingsRow>
                <template #label>
                    <span id="settings-target-resources-ignore-ssl-errors" tabindex="-1">
                        {{ formatMessage(messages.appSettingsResourcesIgnoreSslErrors) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.appSettingsResourcesIgnoreSslErrorsDescription)
                }}</template>
                <template #control>
                    <Toggle id="ignore-ssl-errors" v-model="settings.ignore_ssl_errors" />
                </template>
            </SettingsRow>
            <SettingsRow stacked>
                <template #label>
                    <span id="settings-target-resources-maximum-downloads" tabindex="-1">
                        {{ formatMessage(messages.appSettingsResourcesMaximumDownloads) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.appSettingsResourcesMaximumDownloadsDescription)
                }}</template>
                <template #control>
                    <div class="flex w-full flex-col gap-3">
                        <div class="w-48 max-w-full">
                            <Combobox
                                v-model="downloadConcurrencyMode"
                                :options="downloadConcurrencyOptions"
                            />
                        </div>
                        <Slider
                            v-if="!settings.auto_concurrent_downloads"
                            id="max-downloads"
                            v-model="settings.max_concurrent_downloads"
                            :min="1"
                            :max="256"
                            :step="1"
                        />
                    </div>
                </template>
            </SettingsRow>
            <SettingsRow stacked>
                <template #label>{{
                    formatMessage(messages.appSettingsResourcesMaximumWrites)
                }}</template>
                <template #description>{{
                    formatMessage(messages.appSettingsResourcesMaximumWritesDescription)
                }}</template>
                <template #control>
                    <div class="w-full">
                        <Slider
                            id="max-writes"
                            v-model="settings.max_concurrent_writes"
                            :min="1"
                            :max="50"
                            :step="1"
                        />
                    </div>
                </template>
            </SettingsRow>
        </SettingsSection>

        <SettingsSection
            v-if="props.scope === 'network-multiplayer'"
            :title="formatMessage(messages.appSettingsResourcesSectionsNetworkAuthentication)"
            title-id="settings-target-resources-network-auth"
        >
            <SettingsRow>
                <template #label>
                    <span id="settings-target-network-mojang-auth-source" tabindex="-1">
                        {{ formatMessage(messages.appSettingsResourcesMojangAuthService) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.appSettingsResourcesMojangAuthServiceDescription)
                }}</template>
                <template #control>
                    <div class="w-full">
                        <Combobox v-model="mojangAuthSource" :options="mojangAuthSourceOptions" />
                    </div>
                </template>
            </SettingsRow>
        </SettingsSection>

        <SettingsSection
            v-if="props.scope === 'content-downloads'"
            :title="formatMessage(messages.appSettingsResourcesDohSettings)"
        >
            <template #header>
                <p class="m-0 mt-1 text-sm leading-relaxed text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.appSettingsResourcesDohSettingsDescription) }}
                </p>
            </template>
            <SettingsRow>
                <template #label>{{
                    formatMessage(messages.appSettingsResourcesDohEnabled)
                }}</template>
                <template #control>
                    <Toggle id="doh-enabled" v-model="settings.doh_enabled" />
                </template>
            </SettingsRow>
        </SettingsSection>

        <SettingsSection v-if="props.scope === 'network-multiplayer'">
            <template #header>
                <h2
                    id="settings-target-resources-proxy"
                    tabindex="-1"
                    class="m-0 text-lg font-semibold text-[var(--color-text-primary)]"
                >
                    {{ formatMessage(messages.appSettingsResourcesProxySettings) }}
                </h2>
                <p class="m-0 mt-1 text-sm leading-relaxed text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.appSettingsResourcesProxySettingsDescription) }}
                </p>
            </template>
            <SettingsRow>
                <template #label>{{
                    formatMessage(messages.appSettingsResourcesProxyMode)
                }}</template>
                <template #control>
                    <div class="w-full">
                        <Combobox v-model="proxyConfig.mode" :options="proxyModeOptions" />
                    </div>
                </template>
            </SettingsRow>
            <SettingsRow v-if="proxyConfig.mode === 'custom'" stacked>
                <template #label>{{
                    formatMessage(messages.appSettingsResourcesProxyUrl)
                }}</template>
                <template #control>
                    <StyledInput
                        id="proxy-url"
                        v-model="proxyConfig.url"
                        type="text"
                        :placeholder="
                            formatMessage(messages.appSettingsResourcesProxyUrlPlaceholder)
                        "
                        wrapper-class="w-full"
                        @blur="saveProxyConfig(false)"
                    />
                </template>
            </SettingsRow>
            <SettingsRow v-if="proxyConfig.mode === 'custom'" stacked>
                <template #label>{{
                    formatMessage(messages.appSettingsResourcesProxyUsername)
                }}</template>
                <template #control>
                    <StyledInput
                        id="proxy-username"
                        v-model="proxyConfig.username"
                        type="text"
                        :placeholder="
                            formatMessage(messages.appSettingsResourcesProxyUsernamePlaceholder)
                        "
                        wrapper-class="w-full"
                        @blur="saveProxyConfig(false)"
                    />
                </template>
            </SettingsRow>
            <SettingsRow v-if="proxyConfig.mode === 'custom'" stacked>
                <template #label>{{
                    formatMessage(messages.appSettingsResourcesProxyPassword)
                }}</template>
                <template #control>
                    <StyledInput
                        id="proxy-password"
                        v-model="proxyConfig.password"
                        type="password"
                        :placeholder="
                            formatMessage(messages.appSettingsResourcesProxyPasswordPlaceholder)
                        "
                        wrapper-class="w-full"
                        @blur="saveProxyConfig(false)"
                    />
                </template>
            </SettingsRow>
            <SettingsRow compact>
                <template #label>{{
                    formatMessage(messages.appSettingsResourcesProxyTest)
                }}</template>
                <template #control>
                    <div class="flex flex-wrap items-center justify-end gap-3">
                        <span
                            v-if="proxyTestResult"
                            class="text-sm text-[var(--color-text-tertiary)]"
                            >{{ proxyTestResult }}</span
                        >
                        <button
                            :disabled="
                                proxyTesting ||
                                proxyConfig.mode === 'none' ||
                                (proxyConfig.mode === 'custom' && !proxyConfig.url?.trim())
                            "
                            class="btn min-w-max"
                            @click="testProxy"
                        >
                            {{
                                formatMessage(
                                    proxyTesting
                                        ? messages.appSettingsResourcesProxyTesting
                                        : messages.appSettingsResourcesProxyTest,
                                )
                            }}
                        </button>
                    </div>
                </template>
            </SettingsRow>
        </SettingsSection>

        <SettingsSection
            v-if="props.scope === 'content-downloads'"
            :title="formatMessage(messages.appSettingsResourcesSectionsMissingContent)"
            title-id="settings-target-resources-missing-content"
        >
            <SettingsRow>
                <template #label>
                    <span id="settings-target-resources-missing-content-import" tabindex="-1">
                        {{ formatMessage(messages.appSettingsResourcesMissingContentAutoImport) }}
                    </span>
                </template>
                <template #description>
                    {{
                        formatMessage(
                            messages.appSettingsResourcesMissingContentAutoImportDescription,
                        )
                    }}
                </template>
                <template #control>
                    <Toggle
                        id="missing-content-auto-import"
                        v-model="missingContentScannerSettings.enabled"
                    />
                </template>
            </SettingsRow>
            <SettingsRow stacked>
                <template #label>{{
                    formatMessage(messages.appSettingsResourcesMissingContentImportDirectory)
                }}</template>
                <template #description>
                    {{
                        formatMessage(
                            messages.appSettingsResourcesMissingContentImportDirectoryDescription,
                        )
                    }}
                </template>
                <template #control>
                    <div class="flex w-full flex-wrap items-center gap-2">
                        <div class="min-w-0 flex-1">
                            <StyledInput
                                id="missing-content-import-directory"
                                :model-value="
                                    missingContentScannerSettings.directory ??
                                    formatMessage(
                                        messages.appSettingsResourcesSystemDownloadsDirectory,
                                    )
                                "
                                :icon="FolderOpenIcon"
                                type="text"
                                readonly
                                wrapper-class="w-full"
                            >
                                <template #right>
                                    <IconButton
                                        type="base"
                                        :label="
                                            formatMessage(
                                                messages.appSettingsResourcesSelectImportDirectory,
                                            )
                                        "
                                        class="ml-1.5"
                                        :disabled="!missingContentScannerSettings.enabled"
                                        @click="findMissingContentImportDirectory"
                                    >
                                        <FolderSearchIcon />
                                    </IconButton>
                                </template>
                            </StyledInput>
                        </div>
                        <button
                            v-if="missingContentScannerSettings.directory"
                            class="btn min-w-max"
                            :disabled="!missingContentScannerSettings.enabled"
                            @click="resetMissingContentImportDirectory"
                        >
                            {{ formatMessage(messages.appSettingsResourcesResetImportDirectory) }}
                        </button>
                    </div>
                </template>
            </SettingsRow>
        </SettingsSection>

        <MacOpenPanel v-if="props.scope !== 'network-multiplayer'" ref="openPanel" />
        <MacFinderWindow v-if="props.scope === 'storage-backups'" ref="finderWindow" />
    </div>
</template>
