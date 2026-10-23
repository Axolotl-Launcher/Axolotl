<script setup lang="ts">
import CheckIcon from '@modrinth/assets/icons/check.svg?component'
import CopyIcon from '@modrinth/assets/icons/copy.svg?component'
import ExternalIcon from '@modrinth/assets/icons/external.svg?component'
import GridIcon from '@modrinth/assets/icons/grid.svg?component'
import KeyIcon from '@modrinth/assets/icons/key.svg?component'
import LogOutIcon from '@modrinth/assets/icons/log-out.svg?component'
import PlugIcon from '@modrinth/assets/icons/plug.svg?component'
import PlusIcon from '@modrinth/assets/icons/plus.svg?component'
import RefreshCwIcon from '@modrinth/assets/icons/refresh-cw.svg?component'
import SearchIcon from '@modrinth/assets/icons/search.svg?component'
import SpinnerIcon from '@modrinth/assets/icons/spinner.svg?component'
import TrashIcon from '@modrinth/assets/icons/trash.svg?component'
import Admonition from '@modrinth/ui/src/components/base/Admonition.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Combobox from '@modrinth/ui/src/components/base/Combobox.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import Tabs from '@modrinth/ui/src/components/base/Tabs.vue'
import Toggle from '@modrinth/ui/src/components/base/Toggle.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'

import { usePreview } from '../../../lib/state/index'
import { messages } from '../messages'
import {
    type AIProviderConfig,
    type AIProviderDefinition,
    type AIState,
    type OAuthDeviceCode,
    type OAuthPollStatus,
    useAIBackend,
} from './ai/ai-backend'
import AIIcon from './ai/AIIcon.vue'

/**
 * AI providers (apps/app-frontend/src/components/ui/settings/AISettings.vue):
 * a searchable provider list beside the overview of every provider or the
 * selected provider's endpoint, credentials, text models and connection test.
 * The category is flush, so the layout fills the content panel and scrolls
 * its own columns.
 *
 * The plugin calls go to the window's simulated AI backend (ai/ai-backend.ts):
 * keys typed here stay in memory, tests and model discovery answer the way the
 * providers would for this Mac, and the browser the app hands OAuth device
 * authorization to is stood in for by the backend.
 */
const { formatMessage, locale } = useVIntl()
const { handleError } = injectNotificationManager()
const router = useRouter()
const { clock } = usePreview()
const {
    beginAIOAuth,
    disconnectAIOAuth,
    fetchAIModels,
    getAICatalog,
    getAIState,
    openAuthorizationPage,
    pollAIOAuth,
    removeAIModel,
    setAIProviderCredential,
    setAIProviderKey,
    sharedAIState,
    testAIProvider,
    updateAIModel,
    updateAIProvider,
    updateAISettings,
} = useAIBackend()

/** apps/app-frontend/src/data/sponsoredProviders.ts: marketing links, preferred over homepages. */
const sponsoredProviderWebsites: Record<string, string> = {
    codeflow: 'https://codeflow.sh/register?invite=4UHP2KYH',
}

const emptyState: AIState = {
    settings: { enabled: true },
    catalog_source: '',
    providers: [],
}
const catalog = ref<AIProviderDefinition[]>([])
const state = computed(() => sharedAIState.value ?? emptyState)
const loading = ref(true)
const search = ref('')
const selectedId = ref('')
const apiKey = ref('')
const credentialValues = ref<Record<string, string>>({})
const customModelId = ref('')
const customModelName = ref('')
const modelSearch = ref('')
const status = ref('')
const busy = ref(false)
const masterBusy = ref(false)
const oauthInfo = ref<OAuthDeviceCode | null>(null)
const oauthStatus = ref<OAuthPollStatus | null>(null)
const oauthChecking = ref(false)
let cancelOAuthTimer: (() => void) | undefined

const providerItems = computed(() =>
    catalog.value
        .map((definition) => ({
            definition,
            config: state.value.providers.find(
                (provider) => provider.provider_id === definition.id,
            ),
        }))
        .filter(
            (item): item is { definition: AIProviderDefinition; config: AIProviderConfig } =>
                item.config !== undefined,
        ),
)

const filteredProviderItems = computed(() => {
    const query = search.value.trim().toLocaleLowerCase()
    if (!query) return providerItems.value
    return providerItems.value.filter(
        ({ definition }) =>
            definition.name.toLocaleLowerCase().includes(query) || definition.id.includes(query),
    )
})

const enabledProviderItems = computed(() =>
    filteredProviderItems.value.filter(
        ({ config, definition }) => config.enabled && !definition.sponsored,
    ),
)
const sponsoredProviderItems = computed(() =>
    filteredProviderItems.value.filter(({ definition }) => definition.sponsored),
)
const disabledProviderItems = computed(() =>
    filteredProviderItems.value.filter(
        ({ config, definition }) => !config.enabled && !definition.sponsored,
    ),
)
const allEnabledProviderItems = computed(() =>
    providerItems.value.filter(({ config, definition }) => config.enabled && !definition.sponsored),
)
const allSponsoredProviderItems = computed(() =>
    providerItems.value.filter(({ definition }) => definition.sponsored),
)
const allDisabledProviderItems = computed(() =>
    providerItems.value.filter(
        ({ config, definition }) => !config.enabled && !definition.sponsored,
    ),
)

/* The app bundles both description files; here each locale is its own chunk. */
const providerDescriptions = ref<Record<string, string>>({})
const descriptionsZh = computed(() => locale.value.toLocaleLowerCase().startsWith('zh'))
let descriptionsToken = 0

async function loadProviderDescriptions(zh: boolean) {
    const token = ++descriptionsToken
    const module = zh
        ? await import('./ai/data/provider-descriptions-zh-CN')
        : await import('./ai/data/provider-descriptions-en-US')
    if (token === descriptionsToken) providerDescriptions.value = module.default
}

watch(descriptionsZh, (zh) => {
    loadProviderDescriptions(zh).catch(handleError)
})

const selectedDefinition = computed<AIProviderDefinition | undefined>(() =>
    catalog.value.find((provider) => provider.id === selectedId.value),
)
const selectedConfig = computed<AIProviderConfig | undefined>(() =>
    state.value.providers.find((provider) => provider.provider_id === selectedId.value),
)
const enabledModels = computed(() =>
    (selectedConfig.value?.models ?? []).filter((model) => model.enabled),
)
const selectedTestModel = ref('')
const modelOptions = computed(() =>
    enabledModels.value.map((model) => ({ value: model.id, label: model.name || model.id })),
)
const filteredModels = computed(() => {
    const query = modelSearch.value.trim().toLocaleLowerCase()
    if (!query) return selectedConfig.value?.models ?? []
    return (selectedConfig.value?.models ?? []).filter((model) =>
        [model.id, model.name, model.source].some((value) =>
            value?.toLocaleLowerCase().includes(query),
        ),
    )
})

const bedrockAuthMode = computed({
    get: () => {
        const configured = selectedConfig.value?.configured_credentials ?? []
        return (
            selectedConfig.value?.settings.auth_mode ||
            (configured.includes('aws-access-key-id') ||
            configured.includes('aws-secret-access-key')
                ? 'aws-credentials'
                : 'api-key')
        )
    },
    set: (value: string | number) => {
        if (selectedConfig.value) selectedConfig.value.settings.auth_mode = String(value)
    },
})
const bedrockAuthTabs = computed(() => [
    { value: 'api-key', label: formatMessage(messages.appAiSettingsBedrockApiKey) },
    { value: 'aws-credentials', label: formatMessage(messages.appAiSettingsBedrockAwsCredentials) },
])
const bedrockCredentialFields = computed(() => [
    { name: 'aws-access-key-id', label: formatMessage(messages.appAiSettingsBedrockAccessKeyId) },
    {
        name: 'aws-secret-access-key',
        label: formatMessage(messages.appAiSettingsBedrockSecretAccessKey),
    },
    { name: 'aws-session-token', label: formatMessage(messages.appAiSettingsBedrockSessionToken) },
])
const settingFieldMessages = {
    deployment: messages.appAiSettingsFieldDeployment,
    api_version: messages.appAiSettingsFieldApiVersion,
    account_id: messages.appAiSettingsFieldAccountId,
    region: messages.appAiSettingsFieldRegion,
    project: messages.appAiSettingsFieldProject,
    location: messages.appAiSettingsFieldLocation,
} as const

function selectProvider(id: string) {
    selectedId.value = id
    apiKey.value = ''
    credentialValues.value = {}
    modelSearch.value = ''
    status.value = ''
    oauthInfo.value = null
    oauthStatus.value = null
    selectedTestModel.value =
        state.value.providers
            .find((provider) => provider.provider_id === id)
            ?.models.find((model) => model.enabled)?.id ?? ''
}

function fieldName(field: string) {
    const message = settingFieldMessages[field as keyof typeof settingFieldMessages]
    return message ? formatMessage(message) : field
}

function providerDescription(providerId: string) {
    return providerDescriptions.value[`${providerId}.description`] ?? ''
}

async function reloadState() {
    await getAIState()
    selectedTestModel.value =
        selectedConfig.value?.models.find((model) => model.enabled)?.id ?? selectedTestModel.value
}

/** `openUrl`: the app hands links to the system browser; here a new tab of the visitor's. */
function openUrl(url: string) {
    window.open(url, '_blank', 'noopener')
}

async function openSponsoredWebsite() {
    if (!selectedDefinition.value?.sponsored) return
    const url = sponsoredProviderWebsites[selectedDefinition.value.id]
    if (!url) return
    try {
        openUrl(url)
    } catch (error) {
        handleError(error)
    }
}

async function setMasterEnabled(enabled: boolean) {
    const previous = state.value.settings.enabled
    state.value.settings.enabled = enabled
    masterBusy.value = true
    try {
        await updateAISettings(enabled)
    } catch (error) {
        state.value.settings.enabled = previous
        handleError(error)
    } finally {
        masterBusy.value = false
    }
}

async function saveProvider() {
    const config = selectedConfig.value
    if (!config) return false
    busy.value = true
    try {
        await updateAIProvider({
            provider_id: config.provider_id,
            custom_name: config.custom_name,
            enabled: config.enabled,
            endpoint: config.endpoint,
            settings: config.settings,
        })
        status.value = formatMessage(messages.appAiSettingsSaved)
        return true
    } catch (error) {
        handleError(error)
        return false
    } finally {
        busy.value = false
    }
}

async function setProviderEnabled(enabled: boolean) {
    if (!selectedConfig.value) return
    await setProviderEnabledById(selectedConfig.value.provider_id, enabled)
}

async function setProviderEnabledById(providerId: string, enabled: boolean) {
    const config = state.value.providers.find((provider) => provider.provider_id === providerId)
    if (!config) return
    const previous = config.enabled
    config.enabled = enabled
    busy.value = true
    try {
        await updateAIProvider({
            provider_id: config.provider_id,
            custom_name: config.custom_name,
            enabled: config.enabled,
            endpoint: config.endpoint,
            settings: config.settings,
        })
    } catch (error) {
        config.enabled = previous
        handleError(error)
    } finally {
        busy.value = false
    }
}

async function saveApiKey() {
    if (!selectedConfig.value || !apiKey.value.trim()) return
    busy.value = true
    try {
        if (!(await saveProvider())) return
        await setAIProviderKey(selectedConfig.value.provider_id, apiKey.value)
        apiKey.value = ''
        await reloadState()
    } catch (error) {
        handleError(error)
    } finally {
        busy.value = false
    }
}

async function clearApiKey() {
    if (!selectedConfig.value) return
    try {
        await setAIProviderKey(selectedConfig.value.provider_id, null)
        await reloadState()
    } catch (error) {
        handleError(error)
    }
}

function credentialConfigured(name: string) {
    return selectedConfig.value?.configured_credentials.includes(name) ?? false
}

async function saveCredential(name: string) {
    if (!selectedConfig.value || !credentialValues.value[name]?.trim()) return
    busy.value = true
    try {
        if (!(await saveProvider())) return
        await setAIProviderCredential(
            selectedConfig.value.provider_id,
            name,
            credentialValues.value[name],
        )
        credentialValues.value[name] = ''
        await reloadState()
    } catch (error) {
        handleError(error)
    } finally {
        busy.value = false
    }
}

async function clearCredential(name: string) {
    if (!selectedConfig.value) return
    try {
        await setAIProviderCredential(selectedConfig.value.provider_id, name, null)
        await reloadState()
    } catch (error) {
        handleError(error)
    }
}

async function saveCredentials(names: string[]) {
    if (!selectedConfig.value) return
    const values = names
        .map((name) => [name, credentialValues.value[name]?.trim()] as const)
        .filter((entry): entry is readonly [string, string] => !!entry[1])
    if (!values.length) return
    busy.value = true
    try {
        if (!(await saveProvider())) return
        await Promise.all(
            values.map(([name, secret]) =>
                setAIProviderCredential(selectedConfig.value!.provider_id, name, secret),
            ),
        )
        for (const [name] of values) credentialValues.value[name] = ''
        await reloadState()
    } catch (error) {
        handleError(error)
    } finally {
        busy.value = false
    }
}

async function clearCredentials(names: string[]) {
    if (!selectedConfig.value) return
    busy.value = true
    try {
        await Promise.all(
            names.map((name) =>
                setAIProviderCredential(selectedConfig.value!.provider_id, name, null),
            ),
        )
        await reloadState()
    } catch (error) {
        handleError(error)
    } finally {
        busy.value = false
    }
}

async function copyOAuthCode() {
    if (!oauthInfo.value) return
    try {
        await navigator.clipboard.writeText(oauthInfo.value.user_code)
    } catch (error) {
        handleError(error)
    }
}

async function openOAuthAuthorization() {
    if (!oauthInfo.value) return
    try {
        openAuthorizationPage(oauthInfo.value.flow_id)
    } catch (error) {
        handleError(error)
    }
}

async function setModelEnabled(modelId: string, enabled: boolean) {
    const config = selectedConfig.value
    const model = config?.models.find((item) => item.id === modelId)
    if (!config || !model) return
    model.enabled = enabled
    try {
        await updateAIModel({
            provider_id: config.provider_id,
            model_id: model.id,
            display_name: model.name,
            enabled,
        })
        if (enabled && !selectedTestModel.value) selectedTestModel.value = model.id
    } catch (error) {
        model.enabled = !enabled
        handleError(error)
    }
}

async function addModel() {
    const config = selectedConfig.value
    if (!config || !customModelId.value.trim()) return
    try {
        await updateAIModel({
            provider_id: config.provider_id,
            model_id: customModelId.value,
            display_name: customModelName.value,
            enabled: true,
        })
        customModelId.value = ''
        customModelName.value = ''
        await reloadState()
    } catch (error) {
        handleError(error)
    }
}

async function removeModel(modelId: string) {
    if (!selectedConfig.value) return
    try {
        await removeAIModel(selectedConfig.value.provider_id, modelId)
        await reloadState()
    } catch (error) {
        handleError(error)
    }
}

async function refreshModels() {
    if (!selectedConfig.value) return
    busy.value = true
    try {
        await fetchAIModels(selectedConfig.value.provider_id)
        await reloadState()
    } catch (error) {
        handleError(error)
    } finally {
        busy.value = false
    }
}

async function testProvider() {
    if (!selectedConfig.value || !selectedTestModel.value) return
    busy.value = true
    status.value = ''
    try {
        if (!(await saveProvider())) return
        const response = await testAIProvider(
            selectedConfig.value.provider_id,
            selectedTestModel.value,
        )
        status.value = formatMessage(messages.appAiSettingsTestSuccess, { response })
    } catch (error) {
        handleError(error)
    } finally {
        busy.value = false
    }
}

function stopOAuthPolling() {
    cancelOAuthTimer?.()
    cancelOAuthTimer = undefined
}

async function pollOAuth(info: OAuthDeviceCode, interval = info.interval) {
    if (oauthChecking.value) return
    oauthChecking.value = true
    try {
        const result = await pollAIOAuth(info.flow_id)
        oauthStatus.value = result
        if (result === 'success') {
            stopOAuthPolling()
            oauthInfo.value = null
            await reloadState()
            return
        }
        if (result === 'expired' || result === 'denied') {
            stopOAuthPolling()
            return
        }
        cancelOAuthTimer = clock.after(
            (result === 'slow_down' ? interval + 5 : interval) * 1000,
            () => void pollOAuth(info, result === 'slow_down' ? interval + 5 : interval),
        )
    } catch (error) {
        stopOAuthPolling()
        handleError(error)
    } finally {
        oauthChecking.value = false
    }
}

async function checkOAuthStatus() {
    if (!oauthInfo.value) return
    const info = oauthInfo.value
    stopOAuthPolling()
    await pollOAuth(info)
}

function checkOAuthOnFocus() {
    if (oauthInfo.value && (oauthStatus.value === 'pending' || oauthStatus.value === 'slow_down')) {
        void checkOAuthStatus()
    }
}

async function connectOAuth() {
    if (!selectedConfig.value) return
    stopOAuthPolling()
    busy.value = true
    try {
        const info = await beginAIOAuth(selectedConfig.value.provider_id)
        oauthInfo.value = info
        oauthStatus.value = 'pending'
        // The app opens the authorization page in the system browser here; the
        // backend's stand-in for it starts with the device flow.
        cancelOAuthTimer = clock.after(2000, () => void pollOAuth(info))
    } catch (error) {
        handleError(error)
    } finally {
        busy.value = false
    }
}

async function disconnectOAuth() {
    if (!selectedConfig.value) return
    try {
        await disconnectAIOAuth(selectedConfig.value.provider_id)
        await reloadState()
    } catch (error) {
        handleError(error)
    }
}

function openLanguageTranslationSettings() {
    void router.push('/settings#language-translation')
}

onUnmounted(() => {
    stopOAuthPolling()
    window.removeEventListener('focus', checkOAuthOnFocus)
})
onMounted(async () => {
    window.addEventListener('focus', checkOAuthOnFocus)
    try {
        const [loadedCatalog] = await Promise.all([
            getAICatalog(),
            getAIState(),
            loadProviderDescriptions(descriptionsZh.value),
        ])
        catalog.value = loadedCatalog
        selectedId.value = 'all'
        selectProvider(selectedId.value)
    } catch (error) {
        handleError(error)
    } finally {
        loading.value = false
    }
})
</script>

<template>
    <div v-if="loading" class="flex h-full min-h-48 items-center justify-center">
        <SpinnerIcon class="size-6 animate-spin text-[var(--color-text-tertiary)]" />
    </div>

    <div v-else id="settings-target-ai-providers" tabindex="-1" class="ai-provider-layout">
        <aside class="ai-provider-sidebar">
            <div class="ai-provider-search">
                <StyledInput
                    v-model="search"
                    :icon="SearchIcon"
                    :placeholder="formatMessage(messages.appAiSettingsSearch)"
                    clearable
                    wrapper-class="w-full"
                />
            </div>
            <div class="ai-provider-list">
                <button
                    v-if="!search.trim()"
                    type="button"
                    class="ai-provider-item mb-1"
                    :class="{ selected: selectedId === 'all' }"
                    @click="selectProvider('all')"
                >
                    <GridIcon class="size-[22px] shrink-0" />
                    <span class="min-w-0 flex-1 truncate text-left text-sm font-semibold">
                        {{ formatMessage(messages.appAiSettingsAllProviders) }}
                    </span>
                </button>

                <div v-if="enabledProviderItems.length" class="flex flex-none flex-col gap-1">
                    <p
                        class="m-0 flex items-center justify-between px-2 pb-1 pt-2 text-xs font-semibold text-[var(--color-text-tertiary)]"
                    >
                        <span>{{ formatMessage(messages.appAiSettingsEnabledProviders) }}</span>
                        <span>{{ enabledProviderItems.length }}</span>
                    </p>
                    <button
                        v-for="{ definition: provider } in enabledProviderItems"
                        :key="provider.id"
                        type="button"
                        class="ai-provider-item"
                        :class="{ selected: selectedId === provider.id }"
                        @click="selectProvider(provider.id)"
                    >
                        <AIIcon kind="provider-avatar" :value="provider.id" :size="22" />
                        <span class="min-w-0 flex-1 truncate text-left text-sm font-semibold">
                            {{ provider.name }}
                        </span>
                        <span class="size-2 shrink-0 rounded-full bg-green" />
                    </button>
                </div>

                <div v-if="sponsoredProviderItems.length" class="flex flex-none flex-col gap-1">
                    <p
                        class="m-0 flex items-center justify-between px-2 pb-1 pt-2 text-xs font-semibold text-[var(--color-text-tertiary)]"
                    >
                        <span>{{ formatMessage(messages.appAiSettingsSponsoredProviders) }}</span>
                        <span>{{ sponsoredProviderItems.length }}</span>
                    </p>
                    <button
                        v-for="{ definition: provider, config } in sponsoredProviderItems"
                        :key="provider.id"
                        type="button"
                        class="ai-provider-item"
                        :class="{ selected: selectedId === provider.id }"
                        @click="selectProvider(provider.id)"
                    >
                        <AIIcon kind="provider-avatar" :value="provider.id" :size="22" />
                        <span class="min-w-0 flex-1 truncate text-left text-sm font-semibold">
                            {{ provider.name }}
                        </span>
                        <span v-if="config.enabled" class="size-2 shrink-0 rounded-full bg-green" />
                    </button>
                </div>

                <div v-if="disabledProviderItems.length" class="flex flex-none flex-col gap-1">
                    <p
                        class="m-0 flex items-center justify-between px-2 pb-1 pt-2 text-xs font-semibold text-[var(--color-text-tertiary)]"
                    >
                        <span>{{ formatMessage(messages.appAiSettingsDisabledProviders) }}</span>
                        <span>{{ disabledProviderItems.length }}</span>
                    </p>
                    <button
                        v-for="{ definition: provider } in disabledProviderItems"
                        :key="provider.id"
                        type="button"
                        class="ai-provider-item"
                        :class="{ selected: selectedId === provider.id }"
                        @click="selectProvider(provider.id)"
                    >
                        <AIIcon kind="provider-avatar" :value="provider.id" :size="22" />
                        <span class="min-w-0 flex-1 truncate text-left text-sm font-semibold">
                            {{ provider.name }}
                        </span>
                    </button>
                </div>
                <p
                    v-if="filteredProviderItems.length === 0"
                    class="m-2 text-sm text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.appAiSettingsNoProviders) }}
                </p>
            </div>
            <div class="ai-master-switch">
                <label
                    for="ai-master-switch"
                    class="min-w-0 text-sm font-semibold text-[var(--color-text-primary)]"
                >
                    {{ formatMessage(messages.appAiSettingsMasterSwitch) }}
                </label>
                <Toggle
                    id="ai-master-switch"
                    :model-value="state.settings.enabled"
                    :disabled="masterBusy"
                    small
                    @update:model-value="setMasterEnabled"
                />
            </div>
        </aside>

        <section v-if="selectedId === 'all'" class="ai-provider-overview">
            <Admonition type="info">
                <div class="flex flex-col gap-2">
                    <strong class="text-[var(--color-text-primary)]">
                        {{ formatMessage(messages.appAiSettingsLookingForTranslation) }}
                    </strong>
                    <span class="text-sm text-[var(--color-text-tertiary)]">
                        {{ formatMessage(messages.appAiSettingsLookingForTranslationDescription) }}
                    </span>
                    <div>
                        <Button type="outlined" @click="openLanguageTranslationSettings">
                            {{ formatMessage(messages.appAiSettingsOpenLanguageTranslation) }}
                        </Button>
                    </div>
                </div>
            </Admonition>

            <div v-if="allEnabledProviderItems.length" class="ai-overview-group">
                <div class="ai-overview-heading">
                    <h2>{{ formatMessage(messages.appAiSettingsEnabledProviders) }}</h2>
                    <span>{{ allEnabledProviderItems.length }}</span>
                </div>
                <div class="ai-provider-grid grid grid-cols-2 gap-3">
                    <article
                        v-for="{ definition, config } in allEnabledProviderItems"
                        :key="definition.id"
                        class="ai-provider-card"
                    >
                        <button
                            type="button"
                            class="ai-provider-card-main"
                            @click="selectProvider(definition.id)"
                        >
                            <span class="ai-provider-card-title">
                                <template v-if="definition.id === 'chatgpt'">
                                    <AIIcon
                                        kind="provider-avatar"
                                        :value="definition.id"
                                        :size="24"
                                    />
                                    <strong>{{ definition.name }}</strong>
                                </template>
                                <AIIcon
                                    v-else
                                    kind="provider-combine"
                                    :value="definition.id"
                                    :size="24"
                                />
                            </span>
                            <span class="ai-provider-card-description">
                                {{ providerDescription(definition.id) }}
                            </span>
                        </button>
                        <div
                            class="mt-auto flex min-h-11 items-center justify-between gap-3 bg-transparent px-4 text-xs font-semibold text-[var(--color-text-tertiary)]"
                        >
                            <span>
                                <span class="capitalize">{{ definition.protocol }}</span>
                                ·
                                {{
                                    formatMessage(messages.appAiSettingsConfiguredModels, {
                                        count: config.models.length,
                                    })
                                }}
                            </span>
                            <Toggle
                                :id="`ai-overview-provider-${definition.id}`"
                                :model-value="config.enabled"
                                :disabled="busy"
                                small
                                @update:model-value="setProviderEnabledById(definition.id, $event)"
                            />
                        </div>
                    </article>
                </div>
            </div>

            <div v-if="allSponsoredProviderItems.length" class="ai-overview-group">
                <div class="ai-overview-heading">
                    <h2>{{ formatMessage(messages.appAiSettingsSponsoredProviders) }}</h2>
                    <span>{{ allSponsoredProviderItems.length }}</span>
                </div>
                <div class="ai-provider-grid grid grid-cols-2 gap-3">
                    <article
                        v-for="{ definition, config } in allSponsoredProviderItems"
                        :key="definition.id"
                        class="ai-provider-card"
                    >
                        <button
                            type="button"
                            class="ai-provider-card-main"
                            @click="selectProvider(definition.id)"
                        >
                            <span class="ai-provider-card-title">
                                <template v-if="definition.sponsored">
                                    <AIIcon
                                        kind="provider-wordmark"
                                        :value="definition.id"
                                        :size="28"
                                    />
                                </template>
                                <template v-else-if="definition.id === 'chatgpt'">
                                    <AIIcon
                                        kind="provider-avatar"
                                        :value="definition.id"
                                        :size="24"
                                    />
                                    <strong>{{ definition.name }}</strong>
                                </template>
                                <AIIcon
                                    v-else
                                    kind="provider-combine"
                                    :value="definition.id"
                                    :size="24"
                                />
                            </span>
                            <span class="ai-provider-card-description">
                                {{ providerDescription(definition.id) }}
                            </span>
                        </button>
                        <div
                            class="mt-auto flex min-h-11 items-center justify-between gap-3 bg-transparent px-4 text-xs font-semibold text-[var(--color-text-tertiary)]"
                        >
                            <span>
                                <span class="capitalize">{{ definition.protocol }}</span>
                                ·
                                {{
                                    formatMessage(messages.appAiSettingsConfiguredModels, {
                                        count: config.models.length,
                                    })
                                }}
                            </span>
                            <Toggle
                                :id="`ai-overview-provider-${definition.id}`"
                                :model-value="config.enabled"
                                :disabled="busy"
                                small
                                @update:model-value="setProviderEnabledById(definition.id, $event)"
                            />
                        </div>
                    </article>
                </div>
            </div>

            <div class="ai-overview-group">
                <div class="ai-overview-heading">
                    <h2>{{ formatMessage(messages.appAiSettingsDisabledProviders) }}</h2>
                    <span>{{ allDisabledProviderItems.length }}</span>
                </div>
                <div class="ai-provider-grid grid grid-cols-2 gap-3">
                    <article
                        v-for="{ definition, config } in allDisabledProviderItems"
                        :key="definition.id"
                        class="ai-provider-card"
                    >
                        <button
                            type="button"
                            class="ai-provider-card-main"
                            @click="selectProvider(definition.id)"
                        >
                            <span class="ai-provider-card-title">
                                <template v-if="definition.id === 'chatgpt'">
                                    <AIIcon
                                        kind="provider-avatar"
                                        :value="definition.id"
                                        :size="24"
                                    />
                                    <strong>{{ definition.name }}</strong>
                                </template>
                                <AIIcon
                                    v-else
                                    kind="provider-combine"
                                    :value="definition.id"
                                    :size="24"
                                />
                            </span>
                            <span class="ai-provider-card-description">
                                {{ providerDescription(definition.id) }}
                            </span>
                        </button>
                        <div
                            class="mt-auto flex min-h-11 items-center justify-between gap-3 bg-transparent px-4 text-xs font-semibold text-[var(--color-text-tertiary)]"
                        >
                            <span>
                                <span class="capitalize">{{ definition.protocol }}</span>
                                ·
                                {{
                                    formatMessage(messages.appAiSettingsConfiguredModels, {
                                        count: config.models.length,
                                    })
                                }}
                            </span>
                            <Toggle
                                :id="`ai-overview-provider-${definition.id}`"
                                :model-value="config.enabled"
                                :disabled="busy"
                                small
                                @update:model-value="setProviderEnabledById(definition.id, $event)"
                            />
                        </div>
                    </article>
                </div>
            </div>
        </section>

        <section v-else-if="selectedDefinition && selectedConfig" class="ai-provider-detail">
            <header class="ai-provider-detail-header">
                <div class="flex min-w-0 items-center gap-3">
                    <AIIcon kind="provider-avatar" :value="selectedDefinition.id" :size="40" />
                    <div class="min-w-0">
                        <h3
                            class="m-0 truncate text-base font-semibold text-[var(--color-text-primary)]"
                        >
                            {{ selectedDefinition.name }}
                        </h3>
                        <p class="m-0 mt-0.5 text-xs text-[var(--color-text-tertiary)]">
                            {{ selectedDefinition.protocol }} · {{ selectedDefinition.id }}
                        </p>
                    </div>
                </div>
                <div
                    class="ai-provider-detail-actions flex shrink-0 items-center gap-2 text-sm font-semibold text-[var(--color-text-tertiary)]"
                >
                    <Button
                        v-if="selectedDefinition.sponsored"
                        type="quiet"
                        @click="openSponsoredWebsite"
                    >
                        <ExternalIcon />{{ formatMessage(messages.appAiSettingsVisitWebsite) }}
                    </Button>
                    {{
                        formatMessage(
                            selectedConfig.enabled
                                ? messages.appAiSettingsEnabled
                                : messages.appAiSettingsDisabled,
                        )
                    }}
                    <Toggle
                        :id="`ai-provider-${selectedDefinition.id}`"
                        :model-value="selectedConfig.enabled"
                        :disabled="busy"
                        @update:model-value="setProviderEnabled"
                    />
                </div>
            </header>

            <div class="ai-provider-detail-scroll">
                <div
                    v-if="
                        !['bedrock', 'vertexai'].includes(selectedDefinition.id) ||
                        selectedDefinition.required_settings.length
                    "
                    class="ai-provider-connection-fields grid grid-cols-2 gap-4"
                >
                    <label
                        v-if="!['bedrock', 'vertexai'].includes(selectedDefinition.id)"
                        class="col-span-2 flex flex-col gap-1.5 text-sm font-semibold text-[var(--color-text-primary)]"
                    >
                        {{ formatMessage(messages.appAiSettingsEndpoint) }}
                        <StyledInput
                            v-model="selectedConfig.endpoint"
                            type="url"
                            wrapper-class="w-full"
                        />
                    </label>
                    <label
                        v-for="field in selectedDefinition.required_settings"
                        :key="field"
                        class="flex flex-col gap-1.5 text-sm font-semibold text-[var(--color-text-primary)]"
                    >
                        {{ fieldName(field) }}
                        <StyledInput
                            v-model="selectedConfig.settings[field]"
                            wrapper-class="w-full"
                        />
                    </label>
                </div>

                <div
                    v-if="
                        selectedDefinition.id === 'bedrock' ||
                        selectedDefinition.id === 'vertexai' ||
                        selectedDefinition.auth_type === 'apiKey' ||
                        selectedDefinition.auth_type === 'oauthDeviceFlow'
                    "
                    class="ai-provider-credentials"
                >
                    <div v-if="selectedDefinition.id === 'bedrock'" class="flex flex-col gap-3">
                        <div
                            class="flex flex-col gap-1.5 text-sm font-semibold text-[var(--color-text-primary)]"
                        >
                            {{ formatMessage(messages.appAiSettingsBedrockAuthentication) }}
                            <Tabs v-model:value="bedrockAuthMode" :tabs="bedrockAuthTabs" />
                        </div>
                        <template v-if="bedrockAuthMode === 'api-key'">
                            <label
                                class="flex flex-col gap-1.5 text-sm font-semibold text-[var(--color-text-primary)]"
                            >
                                {{ formatMessage(messages.appAiSettingsApiKey) }}
                                <StyledInput
                                    v-model="apiKey"
                                    :icon="KeyIcon"
                                    type="password"
                                    autocomplete="off"
                                    wrapper-class="w-full"
                                />
                            </label>
                            <p
                                v-if="selectedConfig.has_api_key"
                                class="m-0 text-xs text-[var(--color-text-tertiary)]"
                            >
                                {{ formatMessage(messages.appAiSettingsCredentialConfigured) }}
                            </p>
                            <div class="flex flex-wrap gap-2">
                                <Button
                                    type="base"
                                    :disabled="busy || !apiKey.trim()"
                                    @click="saveApiKey"
                                >
                                    <CheckIcon />{{ formatMessage(messages.appAiSettingsSaveKey) }}
                                </Button>
                                <Button
                                    v-if="selectedConfig.has_api_key"
                                    type="outlined"
                                    color="red"
                                    :disabled="busy"
                                    @click="clearApiKey"
                                >
                                    <TrashIcon />{{ formatMessage(messages.appAiSettingsClearKey) }}
                                </Button>
                            </div>
                        </template>
                        <template v-else>
                            <label
                                v-for="field in bedrockCredentialFields"
                                :key="field.name"
                                class="flex flex-col gap-1.5 text-sm font-semibold text-[var(--color-text-primary)]"
                            >
                                {{ field.label }}
                                <StyledInput
                                    v-model="credentialValues[field.name]"
                                    :icon="KeyIcon"
                                    type="password"
                                    autocomplete="off"
                                    wrapper-class="w-full"
                                />
                                <span
                                    v-if="credentialConfigured(field.name)"
                                    class="text-xs font-normal text-[var(--color-text-tertiary)]"
                                >
                                    {{ formatMessage(messages.appAiSettingsCredentialConfigured) }}
                                </span>
                            </label>
                            <div class="flex flex-wrap gap-2">
                                <Button
                                    type="base"
                                    :disabled="
                                        busy ||
                                        !bedrockCredentialFields.some((field) =>
                                            credentialValues[field.name]?.trim(),
                                        )
                                    "
                                    @click="
                                        saveCredentials(
                                            bedrockCredentialFields.map((field) => field.name),
                                        )
                                    "
                                >
                                    <CheckIcon />{{
                                        formatMessage(messages.appAiSettingsSaveCredential)
                                    }}
                                </Button>
                                <Button
                                    v-if="
                                        bedrockCredentialFields.some((field) =>
                                            credentialConfigured(field.name),
                                        )
                                    "
                                    type="outlined"
                                    color="red"
                                    :disabled="busy"
                                    @click="
                                        clearCredentials(
                                            bedrockCredentialFields.map((field) => field.name),
                                        )
                                    "
                                >
                                    <TrashIcon />{{
                                        formatMessage(messages.appAiSettingsClearCredential)
                                    }}
                                </Button>
                            </div>
                        </template>
                    </div>

                    <div
                        v-else-if="selectedDefinition.id === 'vertexai'"
                        class="flex flex-col gap-2"
                    >
                        <label
                            class="flex flex-col gap-1.5 text-sm font-semibold text-[var(--color-text-primary)]"
                        >
                            {{ formatMessage(messages.appAiSettingsVertexServiceAccount) }}
                            <StyledInput
                                v-model="credentialValues['vertex-service-account']"
                                :icon="KeyIcon"
                                type="password"
                                autocomplete="off"
                                wrapper-class="w-full"
                            />
                            <span class="text-xs font-normal text-[var(--color-text-tertiary)]">
                                {{
                                    formatMessage(
                                        messages.appAiSettingsVertexServiceAccountDescription,
                                    )
                                }}
                            </span>
                            <span
                                v-if="credentialConfigured('vertex-service-account')"
                                class="text-xs font-normal text-[var(--color-text-tertiary)]"
                            >
                                {{ formatMessage(messages.appAiSettingsCredentialConfigured) }}
                            </span>
                        </label>
                        <div class="flex flex-wrap gap-2">
                            <Button
                                type="base"
                                :disabled="
                                    busy || !credentialValues['vertex-service-account']?.trim()
                                "
                                @click="saveCredential('vertex-service-account')"
                            >
                                <CheckIcon />{{
                                    formatMessage(messages.appAiSettingsSaveCredential)
                                }}
                            </Button>
                            <Button
                                v-if="credentialConfigured('vertex-service-account')"
                                type="outlined"
                                color="red"
                                :disabled="busy"
                                @click="clearCredential('vertex-service-account')"
                            >
                                <TrashIcon />{{
                                    formatMessage(messages.appAiSettingsClearCredential)
                                }}
                            </Button>
                        </div>
                    </div>

                    <div
                        v-else-if="selectedDefinition.auth_type === 'apiKey'"
                        class="flex flex-col gap-2"
                    >
                        <label
                            class="flex flex-col gap-1.5 text-sm font-semibold text-[var(--color-text-primary)]"
                        >
                            {{ formatMessage(messages.appAiSettingsApiKey) }}
                            <StyledInput
                                v-model="apiKey"
                                :icon="KeyIcon"
                                type="password"
                                autocomplete="off"
                                wrapper-class="w-full"
                            />
                        </label>
                        <p class="m-0 text-xs text-[var(--color-text-tertiary)]">
                            {{
                                formatMessage(
                                    selectedConfig.has_api_key
                                        ? messages.appAiSettingsApiKeyConfigured
                                        : messages.appAiSettingsApiKeyOptional,
                                )
                            }}
                        </p>
                        <div class="flex flex-wrap gap-2">
                            <Button
                                type="base"
                                :disabled="busy || !apiKey.trim()"
                                @click="saveApiKey"
                            >
                                <CheckIcon />{{ formatMessage(messages.appAiSettingsSaveKey) }}
                            </Button>
                            <Button
                                v-if="selectedConfig.has_api_key"
                                type="outlined"
                                color="red"
                                :disabled="busy"
                                @click="clearApiKey"
                            >
                                <TrashIcon />{{ formatMessage(messages.appAiSettingsClearKey) }}
                            </Button>
                        </div>
                    </div>

                    <div
                        v-else-if="selectedDefinition.auth_type === 'oauthDeviceFlow'"
                        class="flex flex-col gap-3"
                    >
                        <div class="flex flex-wrap items-center gap-2">
                            <span
                                v-if="selectedConfig.oauth_connected"
                                class="text-sm font-semibold text-green"
                            >
                                {{ formatMessage(messages.appAiSettingsOauthConnected) }}
                            </span>
                            <Button
                                type="colored"
                                color="brand"
                                :disabled="busy"
                                @click="connectOAuth"
                            >
                                <ExternalIcon />
                                {{
                                    formatMessage(
                                        selectedConfig.oauth_connected
                                            ? messages.appAiSettingsOauthReconnect
                                            : messages.appAiSettingsOauthConnect,
                                    )
                                }}
                            </Button>
                            <Button
                                v-if="selectedConfig.oauth_connected"
                                type="outlined"
                                color="red"
                                :disabled="busy"
                                @click="disconnectOAuth"
                            >
                                <LogOutIcon />{{
                                    formatMessage(messages.appAiSettingsOauthDisconnect)
                                }}
                            </Button>
                        </div>
                        <div v-if="oauthInfo" class="oauth-code-row">
                            <div class="min-w-0 flex-1">
                                <p
                                    class="m-0 text-xs font-semibold text-[var(--color-text-tertiary)]"
                                >
                                    {{ formatMessage(messages.appAiSettingsOauthCode) }}
                                </p>
                                <p
                                    class="m-0 mt-1 font-mono text-lg font-semibold text-[var(--color-text-primary)]"
                                >
                                    {{ oauthInfo.user_code }}
                                </p>
                            </div>
                            <Button
                                type="quiet"
                                :title="formatMessage(messages.appAiSettingsOauthCopyCode)"
                                @click="copyOAuthCode"
                            >
                                <CopyIcon />
                            </Button>
                            <Button
                                type="quiet"
                                :disabled="oauthChecking"
                                :title="
                                    formatMessage(
                                        oauthChecking
                                            ? messages.appAiSettingsOauthChecking
                                            : messages.appAiSettingsOauthCheck,
                                    )
                                "
                                @click="checkOAuthStatus"
                            >
                                <RefreshCwIcon :class="{ 'animate-spin': oauthChecking }" />
                            </Button>
                            <Button
                                type="quiet"
                                :title="formatMessage(messages.appAiSettingsOauthOpen)"
                                @click="openOAuthAuthorization"
                            >
                                <ExternalIcon />
                            </Button>
                        </div>
                        <p
                            v-if="oauthStatus === 'pending' || oauthStatus === 'slow_down'"
                            class="m-0 text-sm text-[var(--color-text-tertiary)]"
                        >
                            {{ formatMessage(messages.appAiSettingsOauthPending) }}
                        </p>
                        <p v-else-if="oauthStatus === 'expired'" class="m-0 text-sm text-red">
                            {{ formatMessage(messages.appAiSettingsOauthExpired) }}
                        </p>
                        <p v-else-if="oauthStatus === 'denied'" class="m-0 text-sm text-red">
                            {{ formatMessage(messages.appAiSettingsOauthDenied) }}
                        </p>
                    </div>
                </div>

                <div class="ai-provider-models-section">
                    <div class="flex flex-wrap items-center justify-between gap-2">
                        <h4 class="m-0 text-sm font-semibold text-[var(--color-text-primary)]">
                            {{ formatMessage(messages.appAiSettingsModels) }}
                        </h4>
                        <div class="ai-model-actions">
                            <StyledInput
                                v-model="modelSearch"
                                :icon="SearchIcon"
                                :placeholder="formatMessage(messages.appAiSettingsSearchModels)"
                                clearable
                                wrapper-class="w-64"
                            />
                            <Button
                                v-if="selectedDefinition.show_model_fetcher"
                                type="outlined"
                                :disabled="busy"
                                @click="refreshModels"
                            >
                                <RefreshCwIcon :class="{ 'animate-spin': busy }" />
                                {{
                                    formatMessage(
                                        busy
                                            ? messages.appAiSettingsRefreshingModels
                                            : messages.appAiSettingsRefreshModels,
                                    )
                                }}
                            </Button>
                        </div>
                    </div>
                    <div v-if="filteredModels.length" class="ai-model-list">
                        <div v-for="model in filteredModels" :key="model.id" class="ai-model-row">
                            <AIIcon kind="model" :value="model.id" :size="32" />
                            <div class="min-w-0 flex-1">
                                <p
                                    class="m-0 truncate text-sm font-semibold text-[var(--color-text-primary)]"
                                >
                                    {{ model.name || model.id }}
                                </p>
                                <p
                                    class="m-0 mt-0.5 truncate text-xs text-[var(--color-text-tertiary)]"
                                >
                                    {{ model.id }} · {{ model.source }}
                                </p>
                            </div>
                            <Button
                                v-if="model.source === 'custom'"
                                type="quiet"
                                :title="formatMessage(messages.appAiSettingsClearKey)"
                                @click="removeModel(model.id)"
                            >
                                <TrashIcon />
                            </Button>
                            <Toggle
                                :id="`ai-model-${selectedDefinition.id}-${model.id}`"
                                :model-value="model.enabled"
                                :disabled="busy"
                                small
                                @update:model-value="setModelEnabled(model.id, $event)"
                            />
                        </div>
                    </div>
                    <p v-else class="m-0 text-sm text-[var(--color-text-tertiary)]">
                        {{
                            formatMessage(
                                selectedConfig.models.length
                                    ? messages.appAiSettingsNoMatchingModels
                                    : messages.appAiSettingsNoModels,
                            )
                        }}
                    </p>
                    <div class="grid grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto] gap-2">
                        <StyledInput
                            v-model="customModelId"
                            :placeholder="formatMessage(messages.appAiSettingsModelId)"
                            wrapper-class="w-full"
                        />
                        <StyledInput
                            v-model="customModelName"
                            :placeholder="formatMessage(messages.appAiSettingsModelName)"
                            wrapper-class="w-full"
                        />
                        <Button type="base" :disabled="!customModelId.trim()" @click="addModel">
                            <PlusIcon />{{ formatMessage(messages.appAiSettingsAddModel) }}
                        </Button>
                    </div>
                </div>

                <div class="ai-provider-actions">
                    <label
                        class="flex min-w-48 flex-1 flex-col gap-1.5 text-sm font-semibold text-[var(--color-text-primary)]"
                    >
                        {{ formatMessage(messages.appAiSettingsTestModel) }}
                        <Combobox v-model="selectedTestModel" :options="modelOptions" />
                    </label>
                    <Button type="base" :disabled="busy" @click="saveProvider">
                        <CheckIcon />{{ formatMessage(messages.appAiSettingsSaveProvider) }}
                    </Button>
                    <Button
                        type="colored"
                        color="brand"
                        :disabled="busy || !selectedTestModel"
                        @click="testProvider"
                    >
                        <PlugIcon />{{
                            formatMessage(
                                busy ? messages.appAiSettingsTesting : messages.appAiSettingsTest,
                            )
                        }}
                    </Button>
                </div>
                <p v-if="status" class="m-0 text-sm text-[var(--color-text-tertiary)]">
                    {{ status }}
                </p>
            </div>
        </section>
    </div>
</template>

<style scoped>
.ai-provider-layout {
    display: grid;
    grid-template-columns: minmax(14rem, 16rem) minmax(0, 1fr);
    grid-template-rows: minmax(0, 1fr);
    height: 100%;
    min-height: 0;
    width: 100%;
    overflow: hidden;
    box-sizing: border-box;
    border-radius: var(--radius-lg);
}

.ai-provider-sidebar {
    display: flex;
    height: 100%;
    min-height: 0;
    min-width: 0;
    flex-direction: column;
    overflow: hidden;
    border-right: 1px solid
        var(--settings-divider, color-mix(in srgb, var(--surface-4) 55%, transparent));
}

.ai-provider-search {
    flex: none;
    padding: var(--gap-lg);
    border-bottom: 1px solid
        var(--settings-divider, color-mix(in srgb, var(--surface-4) 55%, transparent));
}

.ai-provider-list,
.ai-provider-overview,
.ai-model-list {
    min-height: 0;
    overflow-y: auto;
}

.ai-provider-list {
    display: flex;
    flex: 1 1 0;
    flex-direction: column;
    gap: var(--gap-sm);
    overscroll-behavior: contain;
    overflow-x: hidden;
    overflow-y: auto;
    padding: 0 var(--gap-md) var(--gap-md);
    scrollbar-gutter: stable;
    touch-action: pan-y;
}

.ai-provider-item {
    display: flex;
    width: 100%;
    min-height: 2.25rem;
    flex: none;
    align-items: center;
    gap: var(--gap-sm);
    box-sizing: border-box;
    padding: 0 var(--gap-sm);
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--color-text-tertiary);
    cursor: pointer;
    transition:
        background-color 120ms ease,
        color 120ms ease;
}

.ai-provider-item:hover {
    background: var(--surface-3);
    color: var(--color-text-primary);
}

.ai-provider-item.selected {
    /* The same pair the settings sidebar uses for its active category, so the
       two selection surfaces agree. Deliberately a brand token rather than a
       surface: a selected item should stay emphasised over the background
       image instead of fading with the panel behind it. */
    background: var(--color-button-bg-selected);
    color: var(--color-button-text-selected);
}

.ai-master-switch {
    display: flex;
    min-height: 3.5rem;
    flex: none;
    align-items: center;
    justify-content: space-between;
    gap: var(--gap-md);
    padding: var(--gap-md) var(--gap-lg);
    border-top: 1px solid
        var(--settings-divider, color-mix(in srgb, var(--surface-4) 55%, transparent));
}

.ai-provider-overview {
    display: flex;
    height: 100%;
    min-width: 0;
    box-sizing: border-box;
    flex-direction: column;
    gap: var(--gap-xl);
    padding: var(--gap-xl);
    scrollbar-gutter: stable;
}

.ai-overview-group {
    display: flex;
    flex-direction: column;
    gap: var(--gap-md);
}

.ai-overview-heading {
    display: flex;
    align-items: center;
    gap: var(--gap-sm);
}

.ai-overview-heading h2 {
    margin: 0;
    color: var(--color-text-primary);
    font-size: 1rem;
    font-weight: 700;
}

.ai-overview-heading span {
    display: inline-flex;
    min-width: 1.5rem;
    height: 1.5rem;
    align-items: center;
    justify-content: center;
    box-sizing: border-box;
    padding: 0 0.4rem;
    border-radius: var(--radius-sm);
    background: var(--surface-3);
    color: var(--color-text-tertiary);
    font-size: 0.75rem;
    font-weight: 700;
}

.ai-provider-card {
    display: flex;
    min-width: 0;
    flex-direction: column;
    overflow: hidden;
    border: 1px solid var(--surface-4);
    border-radius: var(--radius-md);
    /* One rung above the settings content panel this sits on, matching
       `SettingsSection`; at `surface-2` the card would match the panel exactly
       and disappear. */
    background: var(--surface-3);
    transition:
        border-color 150ms ease,
        background-color 150ms ease;
}

.ai-provider-card:hover {
    border-color: var(--surface-5);
    background: var(--surface-4);
}

.ai-provider-card-main {
    display: flex;
    min-height: 8.5rem;
    min-width: 0;
    box-sizing: border-box;
    flex-direction: column;
    align-items: stretch;
    gap: 0.875rem;
    padding: 1rem;
    border: 0;
    background: transparent;
    color: inherit;
    cursor: pointer;
}

.ai-provider-card-title {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 0.75rem;
}

.ai-provider-card-title strong {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.ai-provider-card-title strong {
    color: var(--color-text-primary);
    font-size: 1rem;
    font-weight: 700;
}

.ai-provider-card-description {
    display: -webkit-box;
    overflow: hidden;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 3;
    color: var(--color-text-tertiary);
    font-size: 0.8125rem;
    line-height: 1.45;
    text-align: left;
    white-space: pre-line;
}

.ai-provider-detail {
    display: flex;
    height: 100%;
    min-width: 0;
    flex-direction: column;
    overflow: hidden;
}

.ai-provider-detail-header {
    display: flex;
    min-height: 5.25rem;
    flex: none;
    align-items: center;
    justify-content: space-between;
    gap: var(--gap-lg);
    padding: var(--gap-lg) var(--gap-xl);
    border-bottom: 1px solid
        var(--settings-divider, color-mix(in srgb, var(--surface-4) 55%, transparent));
    background: transparent;
}

.ai-provider-detail-actions {
    flex-wrap: wrap;
    justify-content: flex-end;
}

.ai-provider-detail-scroll {
    display: flex;
    min-height: 0;
    flex: 1;
    flex-direction: column;
    gap: var(--gap-xl);
    overflow-y: auto;
    padding: var(--gap-xl);
    scrollbar-gutter: stable;
}

/* The app's `min-width: 1024px` rule, which the 1280 px window always meets. */
.ai-provider-connection-fields > :first-child:last-child {
    grid-column: span 2 / span 2;
}

.ai-provider-credentials {
    display: flex;
    flex-direction: column;
    gap: 0.875rem;
}

.ai-provider-actions {
    display: flex;
    flex-direction: row;
    flex-wrap: wrap;
    align-items: end;
    gap: 0.5rem;
}

.ai-model-actions {
    display: flex;
    min-width: 0;
    flex-wrap: wrap;
    align-items: center;
    justify-content: flex-end;
    gap: 0.5rem;
    margin-left: auto;
}

.ai-model-list {
    display: flex;
    max-height: 15rem;
    flex-direction: column;
    gap: 0.25rem;
}

.ai-model-row {
    display: flex;
    min-height: 3.25rem;
    align-items: center;
    gap: 0.75rem;
    box-sizing: border-box;
    padding: 0.625rem 0.75rem;
    border-radius: var(--radius-sm);
    transition: background-color 120ms ease;
}

.ai-model-row:hover {
    background: var(--surface-3);
}

.oauth-code-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.75rem;
    border: 1px solid var(--surface-4);
    border-radius: var(--radius-sm);
    background: var(--surface-3);
}
</style>
