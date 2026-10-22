<script setup>
import { BoxIcon, FolderOpenIcon, FolderSearchIcon, PlusIcon, TrashIcon } from '@modrinth/assets'
import {
    Combobox,
    defineMessages,
    IconButton,
    injectNotificationManager,
    Slider,
    StyledInput,
    Toggle,
    useVIntl,
} from '@modrinth/ui'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { computed, ref, watch } from 'vue'

import {
    configureCurseForgeManualDownloadWatcher,
    getCurseForgeApiKeyOverride,
    getCurseForgeCapability,
    setCurseForgeApiKeyOverride,
    validateCurseForgeCredentials,
} from '@/helpers/curseforge'
import { syncConfiguredDirectLinks } from '@/helpers/direct-link-sync'
import {
    getMissingContentScannerSettings,
    setMissingContentScannerSettings,
} from '@/helpers/downloads-scanner'
import { resourcePreferredSource, resourceSetPreferredSource } from '@/helpers/resources'
import { get, getProxyConfig, setProxyConfig, testProxyConfig, update } from '@/helpers/settings.ts'
import { createSettingsPatchSaver } from '@/helpers/settings-patch'
import { showAppDbBackupsFolder } from '@/helpers/utils.js'

import SettingsRow from './SettingsRow.vue'
import SettingsSection from './SettingsSection.vue'

const props = defineProps({
    scope: {
        type: String,
        default: 'content-downloads',
        validator: (value) =>
            ['content-downloads', 'network-multiplayer', 'storage-backups'].includes(value),
    },
})

const { handleError } = injectNotificationManager()
const settings = ref(await get())
const saveDraft = createSettingsPatchSaver(settings.value, update)
const missingContentScannerSettings = ref(getMissingContentScannerSettings())
const minecraftDirectories = ref(loadMinecraftDirectories())
const minecraftDirectoryError = ref(null)
const { formatMessage } = useVIntl()
const preferredResourceSource = ref('modrinth')
try {
    preferredResourceSource.value = await resourcePreferredSource()
} catch (error) {
    handleError(error)
}
const savingResourceSource = ref(false)
const resourceSourceOptions = [
    { value: 'modrinth', label: 'Modrinth' },
    { value: 'curseforge', label: 'CurseForge' },
    { value: 'mcarchive', label: 'MCArchive' },
    { value: 'planet_minecraft', label: 'Planet Minecraft' },
]
const resourceSourceIds = ['modrinth', 'curseforge', 'mcarchive', 'planet_minecraft']
const resourceSourceChoice = computed({
    get: () => preferredResourceSource.value,
    set: (value) => {
        void saveResourceSource(value)
    },
})
async function saveResourceSource(value) {
    savingResourceSource.value = true
    try {
        const source = resourceSourceIds.includes(value) ? value : resourceSourceIds[0]
        await resourceSetPreferredSource(source)
        preferredResourceSource.value = source
    } catch (error) {
        handleError(error)
    } finally {
        savingResourceSource.value = false
    }
}

const isPortable = ref(false)

try {
    isPortable.value = await invoke('is_portable_mode')
} catch (error) {
    console.error('Failed to determine portable mode', error)
}

// Proxy settings
/** @type {import('@/helpers/settings.ts').ProxyConfig} */
const proxyConfig = ref({ mode: 'none', url: '', username: '', password: '' })
const proxyTesting = ref(false)
/** @type {import('vue').Ref<string>} */
const proxyTestResult = ref('')

try {
    proxyConfig.value = await getProxyConfig()
} catch (error) {
    console.error('Failed to load proxy configuration', error)
}

async function saveProxyConfig(silent = true) {
    try {
        await setProxyConfig(proxyConfig.value)
    } catch (error) {
        if (!silent) {
            handleError(error)
        }
    }
}

async function testProxy() {
    if (proxyConfig.value.mode === 'custom' && !proxyConfig.value.url?.trim()) {
        proxyTestResult.value = formatMessage(messages.proxyUrlRequired)
        return
    }
    proxyTesting.value = true
    proxyTestResult.value = ''
    try {
        const result = await testProxyConfig(proxyConfig.value)
        if (result.success) {
            proxyTestResult.value =
                result.latency_ms != null
                    ? formatMessage(messages.proxyTestSuccessWithLatency, {
                          latency: result.latency_ms,
                      })
                    : formatMessage(messages.proxyTestSuccess)
        } else {
            proxyTestResult.value = result.message || formatMessage(messages.proxyTestFailed)
        }
    } catch (error) {
        proxyTestResult.value = errorMessage(error, formatMessage(messages.proxyTestFailed))
    } finally {
        proxyTesting.value = false
    }
}

function errorMessage(error, fallback) {
    if (error instanceof Error) return error.message
    if (typeof error === 'string') return error
    if (typeof error === 'object' && error !== null && 'message' in error) {
        return String(error.message)
    }
    return fallback
}

// CurseForge API key
/** @type {import('vue').Ref<import('@/helpers/curseforge').CurseForgeCapability | null>} */
const curseforgeCapability = ref(null)
const curseforgeApiKeyInput = ref('')
const curseforgeApiKeySaved = ref('')
const curseforgeApiKeySaving = ref(false)
const curseforgeApiKeyVerifying = ref(false)
/** @type {import('vue').Ref<string>} */
const curseforgeApiKeyResult = ref('')

if (props.scope === 'content-downloads') {
    try {
        const [capability, override] = await Promise.all([
            getCurseForgeCapability(),
            getCurseForgeApiKeyOverride(),
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
        return formatMessage(messages.curseforgeApiKeyStatusUnauthorized)
    }
    switch (curseforgeCapability.value?.key_source) {
        case 'override':
            return formatMessage(messages.curseforgeApiKeyStatusOverride)
        case 'launcher':
            return formatMessage(messages.curseforgeApiKeyStatusLauncher)
        default:
            return formatMessage(messages.curseforgeApiKeyStatusMissing)
    }
})

async function writeCurseForgeApiKey(apiKey) {
    const submittedInput = curseforgeApiKeyInput.value
    curseforgeApiKeySaving.value = true
    curseforgeApiKeyResult.value = ''
    try {
        curseforgeCapability.value = await setCurseForgeApiKeyOverride(apiKey)
        curseforgeApiKeySaved.value = apiKey ?? ''
        // Keep edits made while the request was running instead of replacing
        // them with the key that was just stored.
        if (curseforgeApiKeyInput.value === submittedInput) {
            curseforgeApiKeyInput.value = apiKey ?? ''
        }
    } catch (error) {
        handleError(error)
    } finally {
        curseforgeApiKeySaving.value = false
    }
}

function saveCurseForgeApiKey() {
    return writeCurseForgeApiKey(curseforgeApiKeyInput.value.trim() || null)
}

async function verifyCurseForgeApiKey() {
    curseforgeApiKeyVerifying.value = true
    curseforgeApiKeyResult.value = ''
    try {
        curseforgeCapability.value = await validateCurseForgeCredentials()
        curseforgeApiKeyResult.value = formatMessage(messages.curseforgeApiKeyVerified)
    } catch (error) {
        console.error('CurseForge key check failed', error)
        const capability = await getCurseForgeCapability().catch(() => curseforgeCapability.value)
        curseforgeCapability.value = capability
        curseforgeApiKeyResult.value = formatMessage(
            capability?.status === 'unauthorized'
                ? messages.curseforgeApiKeyVerifyFailed
                : messages.curseforgeApiKeyVerifyUnavailable,
        )
    } finally {
        curseforgeApiKeyVerifying.value = false
    }
}

watch(curseforgeApiKeyInput, () => {
    curseforgeApiKeyResult.value = ''
})

const messages = defineMessages({
    chineseNames: {
        id: 'app.settings.resources.chinese-names',
        defaultMessage: 'Add known Chinese names to downloads',
    },
    chineseNamesDescription: {
        id: 'app.settings.resources.chinese-names-description',
        defaultMessage:
            'Prefix new resource filenames with the known Chinese project name, when available.',
    },
    preserveNames: {
        id: 'app.settings.resources.preserve-names',
        defaultMessage: 'Preserve local prefixes and suffixes when updating',
    },
    preserveNamesDescription: {
        id: 'app.settings.resources.preserve-names-description',
        defaultMessage:
            'Updates and version changes replace only the known original filename. Custom names that cannot be matched are kept intact.',
    },
    preferredResourceSource: {
        id: 'app.resources.preferred-source',
        defaultMessage: 'Preferred resource source',
    },
    preferredResourceSourceDescription: {
        id: 'app.resources.preferred-source.description',
        defaultMessage: 'Used for default descriptions and equivalent releases. Default: Modrinth.',
    },
    dataStorageSectionTitle: {
        id: 'app.settings.resources.sections.data-storage',
        defaultMessage: 'Data directories & cache',
    },
    downloadBehaviorSectionTitle: {
        id: 'app.settings.resources.sections.download-behavior',
        defaultMessage: 'Download behavior',
    },
    networkAuthSectionTitle: {
        id: 'app.settings.resources.sections.network-authentication',
        defaultMessage: 'Network authentication',
    },
    missingContentSectionTitle: {
        id: 'app.settings.resources.sections.missing-content',
        defaultMessage: 'Missing content',
    },
    selectDirectory: {
        id: 'app.settings.resources.select-directory',
        defaultMessage: 'Select a new app directory',
    },
    appDirectory: { id: 'app.settings.resources.app-directory', defaultMessage: 'App directory' },
    axolotlDataDirectory: {
        id: 'app.settings.resources.axolotl-data-directory',
        defaultMessage: 'Axolotl data directory',
    },
    appDirectoryDescription: {
        id: 'app.settings.resources.app-directory-description',
        defaultMessage:
            'The directory where the launcher stores all of its files. Changes apply after restarting the launcher.',
    },
    appDirectoryDescriptionPortable: {
        id: 'app.settings.resources.app-directory-description-portable',
        defaultMessage:
            'You are currently running in portable mode. The app directory is fixed and cannot be changed.',
    },
    minecraftDirectories: {
        id: 'app.settings.resources.minecraft-directories',
        defaultMessage: 'Minecraft directories',
    },
    minecraftDirectoriesDescription: {
        id: 'app.settings.resources.minecraft-directories-description',
        defaultMessage:
            'Add one or more .minecraft folders for instances that use an external game directory. These folders are kept separate from Axolotl data.',
    },
    addMinecraftDirectory: {
        id: 'app.settings.resources.add-minecraft-directory',
        defaultMessage: 'Add .minecraft directory',
    },
    selectMinecraftDirectory: {
        id: 'app.settings.resources.select-minecraft-directory',
        defaultMessage: 'Select a .minecraft directory',
    },
    removeMinecraftDirectory: {
        id: 'app.settings.resources.remove-minecraft-directory',
        defaultMessage: 'Remove .minecraft directory',
    },
    minecraftDirectoryMode: {
        id: 'app.settings.resources.minecraft-directory-mode',
        defaultMessage: 'Game directory mode',
    },
    minecraftDirectoryModeIsolated: {
        id: 'app.settings.resources.minecraft-directory-mode.isolated',
        defaultMessage: 'Version-isolated',
    },
    minecraftDirectoryModeShared: {
        id: 'app.settings.resources.minecraft-directory-mode.shared',
        defaultMessage: 'Shared .minecraft directory',
    },
    minecraftDirectoryModeAutomatic: {
        id: 'app.settings.resources.minecraft-directory-mode.automatic',
        defaultMessage: 'Automatic',
    },
    downloadMirrors: {
        id: 'app.settings.resources.download-mirrors',
        defaultMessage: 'Download sources',
    },
    downloadMirrorsDescription: {
        id: 'app.settings.resources.download-mirrors-description',
        defaultMessage:
            'Automatic mode chooses between official and mirror sources based on your local environment and recent connection quality.',
    },
    automaticSource: {
        id: 'app.settings.resources.source.automatic',
        defaultMessage: 'Automatic (recommended)',
    },
    officialPreferredSource: {
        id: 'app.settings.resources.source.official-preferred',
        defaultMessage: 'Prefer official sources',
    },
    officialOnlySource: {
        id: 'app.settings.resources.source.official-only',
        defaultMessage: 'Original sources only (no mirrors)',
    },
    openBmclApiSource: {
        id: 'app.settings.resources.source.open-bmcl-api',
        defaultMessage: 'Prefer OpenBMCLAPI',
    },
    minecraftMetadataSource: {
        id: 'app.settings.resources.minecraft-metadata-source',
        defaultMessage: 'Minecraft metadata',
    },
    minecraftMetadataSourceDescription: {
        id: 'app.settings.resources.minecraft-metadata-source-description',
        defaultMessage: 'Version manifests and metadata for Minecraft and supported mod loaders.',
    },
    minecraftFileSource: {
        id: 'app.settings.resources.minecraft-file-source',
        defaultMessage: 'Minecraft files, loaders, and Java',
    },
    minecraftFileSourceDescription: {
        id: 'app.settings.resources.minecraft-file-source-description',
        defaultMessage: 'Game files, assets, libraries, mod loaders, and Java runtimes.',
    },
    modrinthMirror: {
        id: 'app.settings.resources.modrinth-mirror',
        defaultMessage: 'Modrinth',
    },
    modrinthMirrorDescription: {
        id: 'app.settings.resources.modrinth-mirror-description',
        defaultMessage: 'Modrinth file downloads.',
    },
    curseforgeMirror: {
        id: 'app.settings.resources.curseforge-mirror',
        defaultMessage: 'CurseForge',
    },
    curseforgeMirrorDescription: {
        id: 'app.settings.resources.curseforge-mirror-description',
        defaultMessage: 'CurseForge file downloads.',
    },
    curseforgeRestrictionBypass: {
        id: 'app.settings.resources.curseforge-restriction-bypass',
        defaultMessage: 'Automatically download restricted CurseForge files',
    },
    curseforgeRestrictionBypassDescription: {
        id: 'app.settings.resources.curseforge-restriction-bypass-description',
        defaultMessage:
            'When CurseForge does not provide a download address, derive its CDN address and try downloading the file automatically. Disable this to use the manual download workflow.',
    },
    curseforgeApiKey: {
        id: 'app.settings.resources.curseforge-api-key',
        defaultMessage: 'CurseForge API key',
    },
    curseforgeApiKeyDescription: {
        id: 'app.settings.resources.curseforge-api-key-description',
        defaultMessage:
            'Used for CurseForge search, project pages, and installations. A key entered here replaces the one the launcher provides.',
    },
    curseforgeApiKeyField: {
        id: 'app.settings.resources.curseforge-api-key-field',
        defaultMessage: 'API key',
    },
    curseforgeApiKeyPlaceholder: {
        id: 'app.settings.resources.curseforge-api-key-placeholder',
        defaultMessage: 'Leave empty to drop the key entered here',
    },
    curseforgeApiKeyStatusOverride: {
        id: 'app.settings.resources.curseforge-api-key-status-override',
        defaultMessage: 'Using the key entered here. The launcher key is not used.',
    },
    curseforgeApiKeyStatusLauncher: {
        id: 'app.settings.resources.curseforge-api-key-status-launcher',
        defaultMessage: 'No key is set here, so the launcher key is used.',
    },
    curseforgeApiKeyStatusMissing: {
        id: 'app.settings.resources.curseforge-api-key-status-missing',
        defaultMessage:
            'No key is available. CurseForge content cannot be searched or installed until one is set here.',
    },
    curseforgeApiKeyStatusUnauthorized: {
        id: 'app.settings.resources.curseforge-api-key-status-unauthorized',
        defaultMessage: 'CurseForge rejected this key. Check it or enter another one.',
    },
    curseforgeApiKeySave: {
        id: 'app.settings.resources.curseforge-api-key-save',
        defaultMessage: 'Save key',
    },
    curseforgeApiKeySaving: {
        id: 'app.settings.resources.curseforge-api-key-saving',
        defaultMessage: 'Saving…',
    },
    curseforgeApiKeyUnsaved: {
        id: 'app.settings.resources.curseforge-api-key-unsaved',
        defaultMessage: 'Unsaved changes',
    },
    curseforgeApiKeyVerify: {
        id: 'app.settings.resources.curseforge-api-key-verify',
        defaultMessage: 'Check key',
    },
    curseforgeApiKeyVerifying: {
        id: 'app.settings.resources.curseforge-api-key-verifying',
        defaultMessage: 'Checking…',
    },
    curseforgeApiKeyVerified: {
        id: 'app.settings.resources.curseforge-api-key-verified',
        defaultMessage: 'CurseForge accepted the key.',
    },
    curseforgeApiKeyVerifyFailed: {
        id: 'app.settings.resources.curseforge-api-key-verify-failed',
        defaultMessage: 'CurseForge did not accept the key.',
    },
    curseforgeApiKeyVerifyUnavailable: {
        id: 'app.settings.resources.curseforge-api-key-verify-unavailable',
        defaultMessage: 'Could not check the key. Try again later.',
    },
    mojangAuthService: {
        id: 'app.settings.resources.mojang-auth-service',
        defaultMessage: 'Mojang authentication service',
    },
    mojangAuthServiceDescription: {
        id: 'app.settings.resources.mojang-auth-service-description',
        defaultMessage:
            'Mojang login, profile, skin, cape, and session verification requests used by the launcher and the game.',
    },
    mojangAuthOfficialPreferred: {
        id: 'app.settings.resources.source.mojang-official-preferred',
        defaultMessage: 'Prefer official source',
    },
    mojangAuthMirrorPreferred: {
        id: 'app.settings.resources.source.mojang-mirror-preferred',
        defaultMessage: 'Prefer Fallen-Proxy',
    },
    mojangAuthOfficialOnly: {
        id: 'app.settings.resources.source.mojang-official-only',
        defaultMessage: 'Official source only',
    },
    maximumDownloads: {
        id: 'app.settings.resources.maximum-downloads',
        defaultMessage: 'Maximum concurrent downloads',
    },
    ignoreSslErrors: {
        id: 'app.settings.resources.ignore-ssl-errors',
        defaultMessage: 'Ignore SSL certificate errors',
    },
    ignoreSslErrorsDescription: {
        id: 'app.settings.resources.ignore-ssl-errors-description',
        defaultMessage:
            'Allows downloads through proxies or network tools that replace HTTPS certificates. This disables certificate verification and can expose downloads to tampering. Enable it only when necessary.',
    },
    maximumDownloadsDescription: {
        id: 'app.settings.resources.maximum-downloads-description',
        defaultMessage:
            'Automatic mode uses 64 concurrent downloads. Manual changes apply immediately.',
    },
    manualConcurrency: {
        id: 'app.settings.resources.concurrency.manual',
        defaultMessage: 'Manual',
    },
    maximumWrites: {
        id: 'app.settings.resources.maximum-writes',
        defaultMessage: 'Maximum concurrent writes',
    },
    maximumWritesDescription: {
        id: 'app.settings.resources.maximum-writes-description',
        defaultMessage:
            'The maximum number of files the launcher can write to disk at once. Use a lower value if you frequently get I/O errors. An app restart is required.',
    },
    proxySettings: {
        id: 'app.settings.resources.proxy-settings',
        defaultMessage: 'Proxy settings',
    },
    proxySettingsDescription: {
        id: 'app.settings.resources.proxy-settings-description',
        defaultMessage:
            'Configure how the launcher connects to the internet. System proxy follows your OS settings. Custom proxy lets you specify a URL with optional authentication.',
    },
    proxyMode: {
        id: 'app.settings.resources.proxy-mode',
        defaultMessage: 'Proxy mode',
    },
    proxyModeNone: {
        id: 'app.settings.resources.proxy-mode.none',
        defaultMessage: 'No proxy (direct connection)',
    },
    proxyModeSystem: {
        id: 'app.settings.resources.proxy-mode.system',
        defaultMessage: 'Use system proxy',
    },
    proxyModeCustom: {
        id: 'app.settings.resources.proxy-mode.custom',
        defaultMessage: 'Custom proxy',
    },
    proxyUrl: {
        id: 'app.settings.resources.proxy-url',
        defaultMessage: 'Proxy URL',
    },
    proxyUrlRequired: {
        id: 'app.settings.resources.proxy-url-required',
        defaultMessage: 'Proxy URL is required',
    },
    proxyUrlPlaceholder: {
        id: 'app.settings.resources.proxy-url-placeholder',
        defaultMessage: 'http/https/socks5://ip:port',
    },
    proxyUsername: {
        id: 'app.settings.resources.proxy-username',
        defaultMessage: 'Username',
    },
    proxyUsernamePlaceholder: {
        id: 'app.settings.resources.proxy-username-placeholder',
        defaultMessage: 'Username (optional)',
    },
    proxyPassword: {
        id: 'app.settings.resources.proxy-password',
        defaultMessage: 'Password',
    },
    proxyPasswordPlaceholder: {
        id: 'app.settings.resources.proxy-password-placeholder',
        defaultMessage: 'Password (optional)',
    },
    proxyTest: {
        id: 'app.settings.resources.proxy-test',
        defaultMessage: 'Test connection',
    },
    proxyTesting: {
        id: 'app.settings.resources.proxy-testing',
        defaultMessage: 'Testing...',
    },
    proxyTestSuccess: {
        id: 'app.settings.resources.proxy-test-success',
        defaultMessage: 'Connection successful',
    },
    proxyTestSuccessWithLatency: {
        id: 'app.settings.resources.proxy-test-success-latency',
        defaultMessage: 'Connection successful ({latency} ms)',
    },
    proxyTestFailed: {
        id: 'app.settings.resources.proxy-test-failed',
        defaultMessage: 'Connection failed',
    },
    dohSettings: {
        id: 'app.settings.resources.doh-settings',
        defaultMessage: 'DNS over HTTPS',
    },
    dohSettingsDescription: {
        id: 'app.settings.resources.doh-settings-description',
        defaultMessage: 'Resolve download hosts through a built-in pool of encrypted DNS services.',
    },
    dohEnabled: {
        id: 'app.settings.resources.doh-enabled',
        defaultMessage: 'Use DNS over HTTPS',
    },
    missingContentAutoImport: {
        id: 'app.settings.resources.missing-content-auto-import',
        defaultMessage: 'Automatically import missing modpack files',
    },
    missingContentAutoImportDescription: {
        id: 'app.settings.resources.missing-content-auto-import-description',
        defaultMessage:
            'Watch one folder while resolving missing modpack files, then verify and import matching files automatically.',
    },
    missingContentImportDirectory: {
        id: 'app.settings.resources.missing-content-import-directory',
        defaultMessage: 'Monitored import folder',
    },
    missingContentImportDirectoryDescription: {
        id: 'app.settings.resources.missing-content-import-directory-description',
        defaultMessage:
            'The system Downloads folder is used when no custom folder is selected. Subfolders are not scanned.',
    },
    systemDownloadsDirectory: {
        id: 'app.settings.resources.system-downloads-directory',
        defaultMessage: 'System Downloads folder',
    },
    selectImportDirectory: {
        id: 'app.settings.resources.select-import-directory',
        defaultMessage: 'Select monitored folder',
    },
    resetImportDirectory: {
        id: 'app.settings.resources.reset-import-directory',
        defaultMessage: 'Use system Downloads folder',
    },
    databaseBackups: {
        id: 'app.settings.resources.database-backups',
        defaultMessage: 'App database backups',
    },
    openBackupsFolder: {
        id: 'app.settings.resources.open-backups-folder',
        defaultMessage: 'Open backups folder',
    },
    databaseBackupsDescription: {
        id: 'app.settings.resources.database-backups-description',
        defaultMessage:
            'Backups of important app data are stored here in case you need to recover them later.',
    },
})

const MINECRAFT_DIRECTORIES_STORAGE_KEY = 'axolotl-minecraft-directories'

/** @typedef {import('@/helpers/instance').ExternalMinecraftRoot} ExternalMinecraftRoot */

/** @returns {ExternalMinecraftRoot[]} */
function loadMinecraftDirectories() {
    try {
        const raw = localStorage.getItem(MINECRAFT_DIRECTORIES_STORAGE_KEY)
        if (!raw) return []
        const parsed = JSON.parse(raw)
        if (!Array.isArray(parsed)) return []
        const directories = parsed.flatMap((value) => {
            if (typeof value === 'string') return [{ path: value, mode: 'automatic' }]
            if (value && typeof value === 'object' && typeof value.path === 'string') {
                return [
                    {
                        path: value.path,
                        mode:
                            value.mode === 'shared'
                                ? 'shared'
                                : value.mode === 'automatic'
                                  ? 'automatic'
                                  : 'isolated',
                    },
                ]
            }
            return []
        })
        return [
            ...new Map(directories.map((directory) => [directory.path.trim(), directory])).values(),
        ]
    } catch {
        return []
    }
}

/** @param {ExternalMinecraftRoot[]} values */
function persistMinecraftDirectories(values) {
    try {
        const validValues = [
            ...new Map(
                values
                    .map((value) => ({
                        path: value.path.trim(),
                        mode:
                            value.mode === 'shared'
                                ? 'shared'
                                : value.mode === 'automatic'
                                  ? 'automatic'
                                  : 'isolated',
                    }))
                    .map((value) => [value.path, value]),
            ).values(),
        ]
        localStorage.setItem(MINECRAFT_DIRECTORIES_STORAGE_KEY, JSON.stringify(validValues))
    } catch {
        // Local storage may be unavailable in an embedded or restricted webview.
    }
}

function downloadSourceModel(setting) {
    return computed({
        get: () => settings.value[setting],
        set: (source) => {
            settings.value[setting] = source
        },
    })
}

const minecraftMetadataSource = downloadSourceModel('minecraft_metadata_source')
const minecraftFileSource = downloadSourceModel('minecraft_file_source')
const modrinthDownloadSource = downloadSourceModel('modrinth_source')
const curseforgeDownloadSource = downloadSourceModel('curseforge_source')
const automaticSourceOption = computed(() => ({
    value: 'auto',
    label: formatMessage(messages.automaticSource),
}))
const officialPreferredSourceOption = computed(() => ({
    value: 'official_preferred',
    label: formatMessage(messages.officialPreferredSource),
}))
const officialOnlySourceOption = computed(() => ({
    value: 'official_only',
    label: formatMessage(messages.officialOnlySource),
}))
const minecraftSourceOptions = computed(() => [
    automaticSourceOption.value,
    officialPreferredSourceOption.value,
    { value: 'mirror_preferred', label: formatMessage(messages.openBmclApiSource) },
    officialOnlySourceOption.value,
])
const minecraftDirectoryModeOptions = computed(() => [
    { value: 'automatic', label: formatMessage(messages.minecraftDirectoryModeAutomatic) },
    { value: 'isolated', label: formatMessage(messages.minecraftDirectoryModeIsolated) },
    { value: 'shared', label: formatMessage(messages.minecraftDirectoryModeShared) },
])
const modrinthSourceOptions = computed(() => [
    automaticSourceOption.value,
    officialPreferredSourceOption.value,
    officialOnlySourceOption.value,
])
const curseforgeSourceOptions = computed(() => [
    automaticSourceOption.value,
    officialPreferredSourceOption.value,
    officialOnlySourceOption.value,
])
const mojangAuthSource = downloadSourceModel('mojang_auth_source')
const mojangAuthSourceOptions = computed(() => [
    automaticSourceOption.value,
    { value: 'official_preferred', label: formatMessage(messages.mojangAuthOfficialPreferred) },
    { value: 'mirror_preferred', label: formatMessage(messages.mojangAuthMirrorPreferred) },
    { value: 'official_only', label: formatMessage(messages.mojangAuthOfficialOnly) },
])
const downloadConcurrencyMode = computed({
    get: () => (settings.value.auto_concurrent_downloads ? 'auto' : 'manual'),
    set: (mode) => {
        settings.value.auto_concurrent_downloads = mode === 'auto'
    },
})
const downloadConcurrencyOptions = computed(() => [
    {
        value: 'auto',
        label: formatMessage(messages.automaticSource),
    },
    {
        value: 'manual',
        label: formatMessage(messages.manualConcurrency),
    },
])
const proxyModeOptions = computed(() => [
    {
        value: 'none',
        label: formatMessage(messages.proxyModeNone),
    },
    {
        value: 'system',
        label: formatMessage(messages.proxyModeSystem),
    },
    {
        value: 'custom',
        label: formatMessage(messages.proxyModeCustom),
    },
])

const appDirectoryDescriptionText = computed(() =>
    isPortable.value
        ? formatMessage(messages.appDirectoryDescriptionPortable)
        : formatMessage(messages.appDirectoryDescription),
)

watch(
    settings,
    () => {
        const setSettings = JSON.parse(JSON.stringify(settings.value))

        if (!setSettings.custom_dir) {
            setSettings.custom_dir = null
        }

        void saveDraft(setSettings).catch(handleError)
    },
    { deep: true },
)

watch(
    minecraftDirectories,
    (value) => {
        persistMinecraftDirectories(value)
        void syncDirectLinkInstances()
    },
    { deep: true },
)

async function syncDirectLinkInstances() {
    try {
        await syncConfiguredDirectLinks(minecraftDirectories.value)
    } catch (error) {
        handleError(error)
    }
}

watch(
    missingContentScannerSettings,
    (value) => {
        setMissingContentScannerSettings(value)
        void configureCurseForgeManualDownloadWatcher(value.enabled, value.directory).catch(
            handleError,
        )
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
    await showAppDbBackupsFolder().catch(handleError)
}

async function findLauncherDir() {
    const newDir = await open({
        multiple: false,
        directory: true,
        title: formatMessage(messages.selectDirectory),
    })

    if (newDir) {
        settings.value.custom_dir = newDir
    }
}

async function findMissingContentImportDirectory() {
    const directory = await open({
        multiple: false,
        directory: true,
        title: formatMessage(messages.selectImportDirectory),
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
    const directory = await open({
        multiple: false,
        directory: true,
        title: formatMessage(messages.selectMinecraftDirectory),
    })
    if (typeof directory !== 'string') return

    const normalized = directory.trim().replace(/[\\/]+$/, '')
    if (!normalized) return
    if (!minecraftDirectories.value.some((entry) => entry.path === normalized)) {
        minecraftDirectories.value.push({ path: normalized, mode: 'automatic' })
    }
}

function removeMinecraftDirectory(index) {
    minecraftDirectories.value.splice(index, 1)
    if (minecraftDirectoryError.value) minecraftDirectoryError.value = null
}

function validateMinecraftDirectory(_value) {
    minecraftDirectoryError.value = null
}
</script>

<template>
    <div class="flex flex-col gap-6">
        <SettingsSection
            v-if="props.scope === 'storage-backups'"
            :title="formatMessage(messages.dataStorageSectionTitle)"
            title-id="settings-target-resources-data-storage"
        >
            <SettingsRow stacked>
                <template #label>
                    <span id="settings-target-storage-app-directory" tabindex="-1">
                        {{ formatMessage(messages.axolotlDataDirectory) }}
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
                                :label="formatMessage(messages.appDirectory)"
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
                        {{ formatMessage(messages.minecraftDirectories) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.minecraftDirectoriesDescription)
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
                                :aria-label="formatMessage(messages.minecraftDirectoryMode)"
                                :options="minecraftDirectoryModeOptions"
                                class="min-w-32 flex-1"
                            />
                            <IconButton
                                :label="formatMessage(messages.removeMinecraftDirectory)"
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
                            {{ formatMessage(messages.addMinecraftDirectory) }}
                        </button>
                    </div>
                </template>
            </SettingsRow>
            <SettingsRow>
                <template #label>
                    <span id="settings-target-resources-database-backups" tabindex="-1">
                        {{ formatMessage(messages.databaseBackups) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.databaseBackupsDescription)
                }}</template>
                <template #control>
                    <button
                        id="open-db-backups-folder"
                        class="btn min-w-max"
                        @click="openDbBackupsFolder"
                    >
                        <FolderOpenIcon />
                        {{ formatMessage(messages.openBackupsFolder) }}
                    </button>
                </template>
            </SettingsRow>
        </SettingsSection>

        <SettingsSection v-if="props.scope === 'content-downloads'">
            <SettingsRow>
                <template #label>{{ formatMessage(messages.preferredResourceSource) }}</template>
                <template #description>{{
                    formatMessage(messages.preferredResourceSourceDescription)
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
                        formatMessage(messages.chineseNames)
                    }}</span></template
                >
                <template #description>{{
                    formatMessage(messages.chineseNamesDescription)
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
                        formatMessage(messages.preserveNames)
                    }}</span></template
                >
                <template #description>{{
                    formatMessage(messages.preserveNamesDescription)
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
                    {{ formatMessage(messages.downloadMirrors) }}
                </h2>
                <p class="m-0 mt-1 text-sm leading-relaxed text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.downloadMirrorsDescription) }}
                </p>
            </template>
            <SettingsRow>
                <template #label>{{ formatMessage(messages.minecraftMetadataSource) }}</template>
                <template #description>{{
                    formatMessage(messages.minecraftMetadataSourceDescription)
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
                <template #label>{{ formatMessage(messages.minecraftFileSource) }}</template>
                <template #description>{{
                    formatMessage(messages.minecraftFileSourceDescription)
                }}</template>
                <template #control>
                    <div class="w-full">
                        <Combobox v-model="minecraftFileSource" :options="minecraftSourceOptions" />
                    </div>
                </template>
            </SettingsRow>
            <SettingsRow>
                <template #label>{{ formatMessage(messages.modrinthMirror) }}</template>
                <template #description>{{
                    formatMessage(messages.modrinthMirrorDescription)
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
                <template #label>{{ formatMessage(messages.curseforgeMirror) }}</template>
                <template #description>{{
                    formatMessage(messages.curseforgeMirrorDescription)
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
                    formatMessage(messages.curseforgeRestrictionBypass)
                }}</template>
                <template #description>
                    {{ formatMessage(messages.curseforgeRestrictionBypassDescription) }}
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
                    {{ formatMessage(messages.curseforgeApiKey) }}
                </h2>
                <p class="m-0 mt-1 text-sm leading-relaxed text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.curseforgeApiKeyDescription) }}
                </p>
            </template>
            <SettingsRow stacked>
                <template #label>{{ formatMessage(messages.curseforgeApiKeyField) }}</template>
                <template #description>{{ curseforgeApiKeyStatus }}</template>
                <template #control>
                    <div class="flex w-full flex-col gap-2">
                        <div class="flex w-full flex-wrap items-center gap-3">
                            <StyledInput
                                id="curseforge-api-key"
                                v-model="curseforgeApiKeyInput"
                                type="password"
                                :placeholder="formatMessage(messages.curseforgeApiKeyPlaceholder)"
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
                                            ? messages.curseforgeApiKeySaving
                                            : messages.curseforgeApiKeySave,
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
                                            ? messages.curseforgeApiKeyVerifying
                                            : messages.curseforgeApiKeyVerify,
                                    )
                                }}
                            </button>
                        </div>
                        <span
                            v-if="curseforgeApiKeyDirty"
                            class="text-sm text-[var(--color-text-tertiary)]"
                            >{{ formatMessage(messages.curseforgeApiKeyUnsaved) }}</span
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
            :title="formatMessage(messages.downloadBehaviorSectionTitle)"
            title-id="settings-target-resources-download-behavior"
        >
            <SettingsRow>
                <template #label>
                    <span id="settings-target-resources-ignore-ssl-errors" tabindex="-1">
                        {{ formatMessage(messages.ignoreSslErrors) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.ignoreSslErrorsDescription)
                }}</template>
                <template #control>
                    <Toggle id="ignore-ssl-errors" v-model="settings.ignore_ssl_errors" />
                </template>
            </SettingsRow>
            <SettingsRow stacked>
                <template #label>
                    <span id="settings-target-resources-maximum-downloads" tabindex="-1">
                        {{ formatMessage(messages.maximumDownloads) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.maximumDownloadsDescription)
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
                <template #label>{{ formatMessage(messages.maximumWrites) }}</template>
                <template #description>{{
                    formatMessage(messages.maximumWritesDescription)
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
            :title="formatMessage(messages.networkAuthSectionTitle)"
            title-id="settings-target-resources-network-auth"
        >
            <SettingsRow>
                <template #label>
                    <span id="settings-target-network-mojang-auth-source" tabindex="-1">
                        {{ formatMessage(messages.mojangAuthService) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.mojangAuthServiceDescription)
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
            :title="formatMessage(messages.dohSettings)"
        >
            <template #header>
                <p class="m-0 mt-1 text-sm leading-relaxed text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.dohSettingsDescription) }}
                </p>
            </template>
            <SettingsRow>
                <template #label>{{ formatMessage(messages.dohEnabled) }}</template>
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
                    {{ formatMessage(messages.proxySettings) }}
                </h2>
                <p class="m-0 mt-1 text-sm leading-relaxed text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.proxySettingsDescription) }}
                </p>
            </template>
            <SettingsRow>
                <template #label>{{ formatMessage(messages.proxyMode) }}</template>
                <template #control>
                    <div class="w-full">
                        <Combobox v-model="proxyConfig.mode" :options="proxyModeOptions" />
                    </div>
                </template>
            </SettingsRow>
            <SettingsRow v-if="proxyConfig.mode === 'custom'" stacked>
                <template #label>{{ formatMessage(messages.proxyUrl) }}</template>
                <template #control>
                    <StyledInput
                        id="proxy-url"
                        v-model="proxyConfig.url"
                        type="text"
                        :placeholder="formatMessage(messages.proxyUrlPlaceholder)"
                        wrapper-class="w-full"
                        @blur="saveProxyConfig(false)"
                    />
                </template>
            </SettingsRow>
            <SettingsRow v-if="proxyConfig.mode === 'custom'" stacked>
                <template #label>{{ formatMessage(messages.proxyUsername) }}</template>
                <template #control>
                    <StyledInput
                        id="proxy-username"
                        v-model="proxyConfig.username"
                        type="text"
                        :placeholder="formatMessage(messages.proxyUsernamePlaceholder)"
                        wrapper-class="w-full"
                        @blur="saveProxyConfig(false)"
                    />
                </template>
            </SettingsRow>
            <SettingsRow v-if="proxyConfig.mode === 'custom'" stacked>
                <template #label>{{ formatMessage(messages.proxyPassword) }}</template>
                <template #control>
                    <StyledInput
                        id="proxy-password"
                        v-model="proxyConfig.password"
                        type="password"
                        :placeholder="formatMessage(messages.proxyPasswordPlaceholder)"
                        wrapper-class="w-full"
                        @blur="saveProxyConfig(false)"
                    />
                </template>
            </SettingsRow>
            <SettingsRow compact>
                <template #label>{{ formatMessage(messages.proxyTest) }}</template>
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
                                    proxyTesting ? messages.proxyTesting : messages.proxyTest,
                                )
                            }}
                        </button>
                    </div>
                </template>
            </SettingsRow>
        </SettingsSection>

        <SettingsSection
            v-if="props.scope === 'content-downloads'"
            :title="formatMessage(messages.missingContentSectionTitle)"
            title-id="settings-target-resources-missing-content"
        >
            <SettingsRow>
                <template #label>
                    <span id="settings-target-resources-missing-content-import" tabindex="-1">
                        {{ formatMessage(messages.missingContentAutoImport) }}
                    </span>
                </template>
                <template #description>
                    {{ formatMessage(messages.missingContentAutoImportDescription) }}
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
                    formatMessage(messages.missingContentImportDirectory)
                }}</template>
                <template #description>
                    {{ formatMessage(messages.missingContentImportDirectoryDescription) }}
                </template>
                <template #control>
                    <div class="flex w-full flex-wrap items-center gap-2">
                        <div class="min-w-0 flex-1">
                            <StyledInput
                                id="missing-content-import-directory"
                                :model-value="
                                    missingContentScannerSettings.directory ??
                                    formatMessage(messages.systemDownloadsDirectory)
                                "
                                :icon="FolderOpenIcon"
                                type="text"
                                readonly
                                wrapper-class="w-full"
                            >
                                <template #right>
                                    <IconButton
                                        type="base"
                                        :label="formatMessage(messages.selectImportDirectory)"
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
                            {{ formatMessage(messages.resetImportDirectory) }}
                        </button>
                    </div>
                </template>
            </SettingsRow>
        </SettingsSection>
    </div>
</template>
