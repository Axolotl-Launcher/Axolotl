<script setup lang="ts">
import { PlugIcon, SpinnerIcon, TrashIcon } from '@modrinth/assets'
import {
    Button,
    Combobox,
    defineMessages,
    injectNotificationManager,
    LOCALES,
    StyledInput,
    Toggle,
    useVIntl,
} from '@modrinth/ui'
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'

import { type AIProviderDefinition, getAICatalog, getAIState, sharedAIState } from '@/helpers/ai'
import { errorText } from '@/helpers/error-text'
import {
    clearTranslationCache,
    getGoogleIpPoolSize,
    getTranslationErrorKind,
    getTranslationSettings,
    testTranslationProvider,
    type TranslationProvider,
    type TranslationSettings as TranslationSettingsState,
    type TranslationStyle,
    updateTranslationSettings,
} from '@/helpers/translation'

import AIIcon from './AIIcon.vue'
import SettingsRow from './SettingsRow.vue'
import SettingsSection from './SettingsSection.vue'

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const settings = ref<TranslationSettingsState>({
    provider: 'bili-index-translate',
    target_language: '',
    mode: 'bilingual',
    auto_translate: false,
    style: 'weakened',
    ai_provider_id: '',
    ai_model_id: '',
    ai_system_prompt: '',
    deepl_api_endpoint: 'https://api-free.deepl.com/v2/translate',
    deepl_api_key: null,
})

// Debug logging helper
function debugLog(area: string, message: string, data?: unknown) {
    const timestamp = new Date().toISOString()
    const prefix = `[Translation Debug ${timestamp}] [${area}]`
    if (data !== undefined) {
        console.log(prefix, message, data)
    } else {
        console.log(prefix, message)
    }
}
const aiCatalog = ref<AIProviderDefinition[]>([])
const loading = ref(true)
const cacheStatus = ref('')
const testing = ref(false)
const testStatus = ref('')
const googleIpPoolSize = ref(0)
let saveTimer: ReturnType<typeof setTimeout> | undefined
let poolTimer: ReturnType<typeof setInterval> | undefined

const messages = defineMessages({
    title: { id: 'app.translation-settings.title', defaultMessage: 'Translation' },
    displaySectionTitle: {
        id: 'app.translation-settings.display-section-title',
        defaultMessage: 'Translation display',
    },
    description: {
        id: 'app.translation-settings.description',
        defaultMessage:
            'Translate Modrinth project titles, summaries, and descriptions while browsing content.',
    },
    provider: { id: 'app.translation-settings.provider', defaultMessage: 'Translation service' },
    google: {
        id: 'app.translation-settings.provider.google',
        defaultMessage: 'Google Translate (free)',
    },
    biliIndexTranslate: {
        id: 'app.translation-settings.provider.bili-index-translate',
        defaultMessage: 'Bili Index Translate',
    },
    deepl: {
        id: 'app.translation-settings.provider.deepl',
        defaultMessage: 'DeepL',
    },
    deeplApiEndpoint: {
        id: 'app.translation-settings.deepl-api-endpoint',
        defaultMessage: 'API endpoint',
    },
    deeplApiEndpointPlaceholder: {
        id: 'app.translation-settings.deepl-api-endpoint-placeholder',
        defaultMessage: 'https://api-free.deepl.com/v2/translate',
    },
    deeplApiKey: {
        id: 'app.translation-settings.deepl-api-key',
        defaultMessage: 'API key',
    },
    deeplApiKeyPlaceholder: {
        id: 'app.translation-settings.deepl-api-key-placeholder',
        defaultMessage: 'Enter your DeepL API key',
    },
    deeplApiKeyHint: {
        id: 'app.translation-settings.deepl-api-key-hint',
        defaultMessage: 'Get a free key at deepl.com/pro-api',
    },
    googleIpPool: {
        id: 'app.translation-settings.google-ip-pool',
        defaultMessage: 'IP pool {count}',
    },
    ai: { id: 'app.translation-settings.provider.ai', defaultMessage: 'AI model' },
    aiProvider: { id: 'app.translation-settings.ai-provider', defaultMessage: 'AI provider' },
    aiModel: { id: 'app.translation-settings.ai-model', defaultMessage: 'Text model' },
    targetLanguage: {
        id: 'app.translation-settings.target-language',
        defaultMessage: 'Target language',
    },
    followApp: {
        id: 'app.translation-settings.target-language.follow-app',
        defaultMessage: 'Follow launcher language',
    },
    displayMode: {
        id: 'app.translation-settings.display-mode',
        defaultMessage: 'Display mode',
    },
    bilingual: {
        id: 'app.translation-settings.display-mode.bilingual',
        defaultMessage: 'Original and translation',
    },
    translationOnly: {
        id: 'app.translation-settings.display-mode.translation-only',
        defaultMessage: 'Translation only',
    },
    autoTranslate: {
        id: 'app.translation-settings.auto-translate',
        defaultMessage: 'Translate project pages automatically',
    },
    autoTranslateDescription: {
        id: 'app.translation-settings.auto-translate-description',
        defaultMessage: 'Start translating as soon as a Modrinth project page is opened.',
    },
    style: { id: 'app.translation-settings.style', defaultMessage: 'Translation style' },
    styleDefault: { id: 'app.translation-settings.style.default', defaultMessage: 'Default' },
    styleBlur: { id: 'app.translation-settings.style.blur', defaultMessage: 'Blur' },
    styleBlockquote: {
        id: 'app.translation-settings.style.blockquote',
        defaultMessage: 'Block quote',
    },
    styleWeakened: { id: 'app.translation-settings.style.weakened', defaultMessage: 'Muted' },
    styleDashedLine: {
        id: 'app.translation-settings.style.dashed-line',
        defaultMessage: 'Dashed underline',
    },
    styleBorder: { id: 'app.translation-settings.style.border', defaultMessage: 'Border' },
    styleTextColor: {
        id: 'app.translation-settings.style.text-color',
        defaultMessage: 'Text color',
    },
    styleBackground: {
        id: 'app.translation-settings.style.background',
        defaultMessage: 'Background',
    },
    stylePreview: { id: 'app.translation-settings.style.preview', defaultMessage: 'Preview' },
    stylePreviewOriginalText: {
        id: 'app.translation-settings.style.preview-original-text',
        defaultMessage: 'Explore high-quality Minecraft content on Modrinth.',
    },
    stylePreviewText: {
        id: 'app.translation-settings.style.preview-text',
        defaultMessage: 'Discover high-quality Minecraft content on Modrinth.',
    },
    systemPrompt: {
        id: 'app.translation-settings.system-prompt',
        defaultMessage: 'Translation instructions',
    },
    systemPromptDescription: {
        id: 'app.translation-settings.system-prompt-description',
        defaultMessage:
            'Optional feature-specific instructions. The launcher always appends its structured translation contract.',
    },
    test: { id: 'app.translation-settings.test', defaultMessage: 'Test service' },
    testing: { id: 'app.translation-settings.testing', defaultMessage: 'Testing…' },
    testSuccess: {
        id: 'app.translation-settings.test-success',
        defaultMessage: 'Connection succeeded: {translation}',
    },
    cache: { id: 'app.translation-settings.cache', defaultMessage: 'Translation cache' },
    cacheDescription: {
        id: 'app.translation-settings.cache-description',
        defaultMessage: 'Successful translations are cached for seven days to reduce requests.',
    },
    clearCache: {
        id: 'app.translation-settings.clear-cache',
        defaultMessage: 'Clear translation cache',
    },
    cacheCleared: {
        id: 'app.translation-settings.cache-cleared',
        defaultMessage: 'Translation cache cleared.',
    },
    operationFailed: {
        id: 'app.translation-settings.operation-failed',
        defaultMessage: 'The translation operation failed. Check the configuration and try again.',
    },
    rateLimited: {
        id: 'app.translation.error.rate-limited',
        defaultMessage:
            'The translation service is temporarily rate limited. Please try again later.',
    },
    authenticationFailed: {
        id: 'app.translation.error.authentication',
        defaultMessage: 'The translation service could not authenticate. Please try again later.',
    },
    contentTooLong: {
        id: 'app.translation.error.content-too-long',
        defaultMessage: 'This content is too long for the selected translation service.',
    },
    networkFailed: {
        id: 'app.translation.error.network',
        defaultMessage:
            'The translation service could not be reached. Check your network or proxy.',
    },
})

const configuredAIProviders = computed(() =>
    (sharedAIState.value?.providers ?? []).filter(
        (provider) => provider.enabled && provider.models.some((model) => model.enabled),
    ),
)
const aiAvailable = computed(
    () => !!sharedAIState.value?.settings.enabled && configuredAIProviders.value.length > 0,
)

const modes = ['bilingual', 'translation-only'] as const
const styles: TranslationStyle[] = [
    'default',
    'blur',
    'blockquote',
    'weakened',
    'dashed-line',
    'border',
    'text-color',
    'background',
]
const languages = ['follow-app', ...LOCALES.map((locale) => locale.code)]

const targetLanguage = computed({
    get: () => settings.value.target_language || 'follow-app',
    set: (value: string) => {
        settings.value.target_language = value === 'follow-app' ? '' : value
    },
})

function providerName(provider: TranslationProvider) {
    return formatMessage(
        {
            google: messages.google,
            deepl: messages.deepl,
            ai: messages.ai,
            'bili-index-translate': messages.biliIndexTranslate,
        }[provider],
    )
}

function languageName(code: string) {
    if (code === 'follow-app') return formatMessage(messages.followApp)
    const locale = LOCALES.find((item) => item.code === code)
    return locale ? `${locale.name} — ${formatMessage(locale.translatedName)}` : code
}

function styleName(style: TranslationStyle) {
    return formatMessage(
        {
            default: messages.styleDefault,
            blur: messages.styleBlur,
            blockquote: messages.styleBlockquote,
            weakened: messages.styleWeakened,
            'dashed-line': messages.styleDashedLine,
            border: messages.styleBorder,
            'text-color': messages.styleTextColor,
            background: messages.styleBackground,
        }[style],
    )
}

const translationProviders = computed<TranslationProvider[]>(() => [
    'bili-index-translate',
    'google',
    ...(aiAvailable.value ? (['ai'] as const) : []),
    'deepl',
])
const providerOptions = computed(() =>
    translationProviders.value.map((provider) => ({
        value: provider,
        label: providerName(provider),
    })),
)
const languageOptions = computed(() =>
    languages.map((language) => ({ value: language, label: languageName(language) })),
)
const modeOptions = computed(() =>
    modes.map((mode) => ({
        value: mode,
        label: formatMessage(mode === 'bilingual' ? messages.bilingual : messages.translationOnly),
    })),
)
const styleOptions = computed(() =>
    styles.map((style) => ({ value: style, label: styleName(style) })),
)
const aiProviderOptions = computed(() =>
    configuredAIProviders.value.map((provider) => ({
        value: provider.provider_id,
        label:
            provider.custom_name ||
            aiCatalog.value.find((definition) => definition.id === provider.provider_id)?.name ||
            provider.provider_id,
    })),
)
const deeplApiKey = computed({
    get: () => settings.value.deepl_api_key ?? '',
    set: (value: string) => {
        settings.value.deepl_api_key = value
    },
})
const selectedAIProvider = computed({
    get: () => settings.value.ai_provider_id,
    set: (providerId: string) => {
        settings.value.ai_provider_id = providerId
        settings.value.ai_model_id =
            configuredAIProviders.value
                .find((provider) => provider.provider_id === providerId)
                ?.models.find((model) => model.enabled)?.id ?? ''
    },
})
const aiModelOptions = computed(() =>
    (
        configuredAIProviders.value.find(
            (provider) => provider.provider_id === settings.value.ai_provider_id,
        )?.models ?? []
    )
        .filter((model) => model.enabled)
        .map((model) => ({ value: model.id, label: model.name || model.id })),
)
const stylePreviewClass = computed(() => `translation-style-preview-${settings.value.style}`)

watch(
    [aiAvailable, configuredAIProviders],
    () => {
        if (!aiAvailable.value) {
            if (settings.value.provider === 'ai') settings.value.provider = 'google'
            return
        }
        if (
            !configuredAIProviders.value.some(
                (provider) => provider.provider_id === settings.value.ai_provider_id,
            )
        ) {
            selectedAIProvider.value = configuredAIProviders.value[0]?.provider_id ?? ''
        }
        if (!aiModelOptions.value.some((model) => model.value === settings.value.ai_model_id)) {
            settings.value.ai_model_id = aiModelOptions.value[0]?.value ?? ''
        }
    },
    { immediate: true, deep: true },
)

function reportOperationError(error?: unknown, context?: string) {
    const errorKind = error ? getTranslationErrorKind(error) : 'provider'
    const errorMessage = errorText(error)

    debugLog('Error', `Operation failed${context ? ` (${context})` : ''}`, {
        kind: errorKind,
        message: errorMessage,
        provider: settings.value.provider,
        deeplApiKeySet: !!settings.value.deepl_api_key?.trim(),
        deeplEndpoint: settings.value.deepl_api_endpoint,
    })

    // Don't show error notifications for DeepL when API key is not configured
    // This prevents spam when user is still configuring
    if (
        settings.value.provider === 'deepl' &&
        !settings.value.deepl_api_key?.trim() &&
        errorMessage.includes('DeepL API key is not configured')
    ) {
        debugLog(
            'Error',
            'Suppressing DeepL API key not configured error - user is still configuring',
        )
        return
    }

    const message = error
        ? {
              'rate-limited': messages.rateLimited,
              authentication: messages.authenticationFailed,
              'content-too-long': messages.contentTooLong,
              network: messages.networkFailed,
              provider: messages.operationFailed,
          }[errorKind]
        : messages.operationFailed
    // Surface the underlying provider error (e.g. DeepL HTTP status, quota
    // or endpoint mistakes) instead of a generic message, so users can fix
    // the configuration themselves.
    const displayMessage =
        errorKind === 'provider' && errorMessage.includes('DeepL API error')
            ? errorMessage
            : formatMessage(message)
    handleError(new Error(displayMessage))
}

watch(
    settings,
    (newSettings) => {
        if (loading.value) {
            debugLog('Watch', 'Skipping save - still loading')
            return
        }

        // Deep watchers receive the same object for old and new values when a
        // nested field is mutated in place. Always schedule a save for a loaded
        // settings change instead of comparing those references.
        debugLog('Watch', 'Settings changed', {
            provider: newSettings.provider,
            mode: newSettings.mode,
            style: newSettings.style,
            autoTranslate: newSettings.auto_translate,
        })

        clearTimeout(saveTimer)
        saveTimer = setTimeout(() => {
            saveTimer = undefined
            debugLog('Save', 'Saving settings to backend', {
                provider: newSettings.provider,
                deeplApiKeySet: !!newSettings.deepl_api_key?.trim(),
                deeplEndpoint: newSettings.deepl_api_endpoint,
                aiProviderId: newSettings.ai_provider_id,
                aiModelId: newSettings.ai_model_id,
            })

            // Only save settings, don't show error notifications
            // Errors during save should be silent - only test button shows errors
            void updateTranslationSettings({ ...settings.value })
                .then(() => {
                    debugLog('Save', 'Settings saved successfully')
                })
                .catch((error) => {
                    // Log error but don't show notification to user
                    // Only the "Test" button should show errors
                    const errorMessage = errorText(error)
                    debugLog('Save', 'Settings save failed (silent)', {
                        error: errorMessage,
                        provider: newSettings.provider,
                    })
                })
        }, 300)
    },
    { deep: true },
)

async function refreshGoogleIpPool() {
    try {
        googleIpPoolSize.value = await getGoogleIpPoolSize()
    } catch (error) {
        reportOperationError(error)
    }
}

watch(
    () => settings.value.provider,
    (provider) => {
        clearInterval(poolTimer)
        if (provider !== 'google') return
        void refreshGoogleIpPool()
        poolTimer = setInterval(() => void refreshGoogleIpPool(), 5000)
    },
    { immediate: true },
)

onUnmounted(() => {
    clearInterval(poolTimer)
    if (loading.value || !saveTimer) return
    clearTimeout(saveTimer)
    saveTimer = undefined
    void updateTranslationSettings({ ...settings.value }).catch(reportOperationError)
})

onMounted(async () => {
    debugLog('Init', 'Loading translation settings...')
    try {
        const [loadedSettings, , loadedCatalog] = await Promise.all([
            getTranslationSettings(),
            getAIState(),
            getAICatalog(),
        ])
        debugLog('Init', 'Settings loaded from backend', {
            provider: loadedSettings.provider,
            deeplApiKeySet: !!loadedSettings.deepl_api_key?.trim(),
            deeplEndpoint: loadedSettings.deepl_api_endpoint,
            aiProviderId: loadedSettings.ai_provider_id,
            aiModelId: loadedSettings.ai_model_id,
            targetLanguage: loadedSettings.target_language,
            mode: loadedSettings.mode,
            autoTranslate: loadedSettings.auto_translate,
        })
        settings.value = loadedSettings
        aiCatalog.value = loadedCatalog
    } catch (error) {
        debugLog('Init', 'Failed to load settings', error)
        reportOperationError(error, 'load-settings')
    } finally {
        loading.value = false
        debugLog('Init', 'Loading complete')
    }
})

async function testProvider() {
    debugLog('Test', 'Starting provider test', {
        provider: settings.value.provider,
        deeplApiKeySet: !!settings.value.deepl_api_key?.trim(),
        deeplEndpoint: settings.value.deepl_api_endpoint,
        aiProviderId: settings.value.ai_provider_id,
        aiModelId: settings.value.ai_model_id,
    })

    testing.value = true
    testStatus.value = ''

    // Validate DeepL configuration before testing
    if (settings.value.provider === 'deepl') {
        if (!settings.value.deepl_api_key?.trim()) {
            debugLog('Test', 'DeepL API key is not configured')
            reportOperationError(
                new Error('DeepL API key is not configured. Please enter your API key first.'),
                'deepl-validation',
            )
            testing.value = false
            return
        }
        if (!settings.value.deepl_api_endpoint?.trim()) {
            debugLog('Test', 'DeepL API endpoint is not configured, using default')
            settings.value.deepl_api_endpoint = 'https://api-free.deepl.com/v2/translate'
        }
    }

    try {
        debugLog('Test', 'Saving settings before test...')
        await updateTranslationSettings({ ...settings.value })
        debugLog('Test', 'Settings saved, now testing provider...')

        const result = await testTranslationProvider(settings.value.provider)
        debugLog('Test', 'Test succeeded', { result })
        testStatus.value = formatMessage(messages.testSuccess, { translation: result })
    } catch (error) {
        debugLog('Test', 'Test failed', error)
        reportOperationError(error, 'test-provider')
    } finally {
        testing.value = false
        debugLog('Test', 'Test complete')
    }
}

async function clearCache() {
    try {
        await clearTranslationCache()
        cacheStatus.value = formatMessage(messages.cacheCleared)
    } catch (error) {
        reportOperationError(error)
    }
}
</script>

<template>
    <div v-if="loading" class="flex min-h-48 items-center justify-center">
        <SpinnerIcon class="size-6 animate-spin text-[var(--color-text-tertiary)]" />
    </div>
    <div v-else class="flex flex-col gap-6">
        <SettingsSection>
            <template #header>
                <h2
                    id="settings-target-translation-service"
                    tabindex="-1"
                    class="m-0 text-lg font-semibold text-[var(--color-text-primary)]"
                >
                    {{ formatMessage(messages.title) }}
                </h2>
                <p class="m-0 mt-1 text-sm leading-relaxed text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.description) }}
                </p>
            </template>
            <template #extra>
                <div class="flex flex-wrap items-center justify-end gap-2">
                    <span v-if="testStatus" class="text-sm text-[var(--color-text-tertiary)]">{{
                        testStatus
                    }}</span>
                    <Button type="base" :disabled="testing" @click="testProvider">
                        <PlugIcon />{{ formatMessage(testing ? messages.testing : messages.test) }}
                    </Button>
                </div>
            </template>
            <SettingsRow class="[--settings-control-width:16rem]">
                <template #label>{{ formatMessage(messages.provider) }}</template>
                <template #description>
                    <span v-if="settings.provider === 'google'">
                        {{ formatMessage(messages.googleIpPool, { count: googleIpPoolSize }) }}
                    </span>
                </template>
                <template #control>
                    <div class="w-full">
                        <Combobox v-model="settings.provider" :options="providerOptions" />
                    </div>
                </template>
            </SettingsRow>
            <SettingsRow v-if="settings.provider === 'deepl'" stacked>
                <template #label>{{ formatMessage(messages.deeplApiEndpoint) }}</template>
                <template #control>
                    <StyledInput
                        v-model="settings.deepl_api_endpoint"
                        :placeholder="formatMessage(messages.deeplApiEndpointPlaceholder)"
                        wrapper-class="w-full"
                    />
                </template>
            </SettingsRow>
            <SettingsRow v-if="settings.provider === 'deepl'" stacked>
                <template #label>{{ formatMessage(messages.deeplApiKey) }}</template>
                <template #description>{{ formatMessage(messages.deeplApiKeyHint) }}</template>
                <template #control>
                    <StyledInput
                        v-model="deeplApiKey"
                        type="password"
                        :placeholder="formatMessage(messages.deeplApiKeyPlaceholder)"
                        wrapper-class="w-full"
                    />
                </template>
            </SettingsRow>
            <SettingsRow v-if="settings.provider === 'ai' && aiAvailable">
                <template #label>{{ formatMessage(messages.aiProvider) }}</template>
                <template #control>
                    <div class="w-full">
                        <Combobox v-model="selectedAIProvider" :options="aiProviderOptions">
                            <template #selected="{ label }">
                                <span class="inline-flex min-w-0 items-center gap-2">
                                    <AIIcon
                                        kind="provider-avatar"
                                        :value="selectedAIProvider"
                                        :size="20"
                                    />
                                    <span class="truncate">{{ label }}</span>
                                </span>
                            </template>
                            <template #option="{ item, isSelected }">
                                <div class="flex min-w-0 items-center gap-2.5">
                                    <AIIcon
                                        kind="provider-avatar"
                                        :value="String(item.value)"
                                        :size="22"
                                    />
                                    <span
                                        class="truncate font-semibold leading-tight"
                                        :class="
                                            isSelected
                                                ? 'text-brand'
                                                : 'text-[var(--color-text-default)]'
                                        "
                                    >
                                        {{ item.label }}
                                    </span>
                                </div>
                            </template>
                        </Combobox>
                    </div>
                </template>
            </SettingsRow>
            <SettingsRow v-if="settings.provider === 'ai' && aiAvailable" stacked>
                <template #label>{{ formatMessage(messages.aiModel) }}</template>
                <template #control>
                    <div
                        class="translation-model-combobox relative w-full"
                        :class="{ 'has-model-icon': settings.ai_model_id }"
                    >
                        <AIIcon
                            v-if="settings.ai_model_id"
                            class="pointer-events-none absolute left-3 top-1/2 z-[2] -translate-y-1/2"
                            kind="model"
                            :value="settings.ai_model_id"
                            :size="20"
                        />
                        <Combobox
                            v-model="settings.ai_model_id"
                            :options="aiModelOptions"
                            searchable
                        >
                            <template #option="{ item, isSelected }">
                                <div class="flex min-w-0 items-center gap-2.5">
                                    <AIIcon kind="model" :value="String(item.value)" :size="22" />
                                    <span
                                        class="truncate font-semibold leading-tight"
                                        :class="
                                            isSelected
                                                ? 'text-brand'
                                                : 'text-[var(--color-text-default)]'
                                        "
                                    >
                                        {{ item.label }}
                                    </span>
                                </div>
                            </template>
                        </Combobox>
                    </div>
                </template>
            </SettingsRow>
            <SettingsRow v-if="settings.provider === 'ai' && aiAvailable" stacked>
                <template #label>{{ formatMessage(messages.systemPrompt) }}</template>
                <template #description>{{
                    formatMessage(messages.systemPromptDescription)
                }}</template>
                <template #control>
                    <StyledInput
                        v-model="settings.ai_system_prompt"
                        multiline
                        :rows="3"
                        resize="vertical"
                        wrapper-class="w-full"
                    />
                </template>
            </SettingsRow>
        </SettingsSection>

        <SettingsSection
            :title="formatMessage(messages.displaySectionTitle)"
            title-id="settings-target-translation-display"
        >
            <SettingsRow>
                <template #label>{{ formatMessage(messages.targetLanguage) }}</template>
                <template #control>
                    <div class="w-full">
                        <Combobox v-model="targetLanguage" :options="languageOptions" searchable />
                    </div>
                </template>
            </SettingsRow>
            <SettingsRow>
                <template #label>{{ formatMessage(messages.displayMode) }}</template>
                <template #control>
                    <div class="w-full">
                        <Combobox v-model="settings.mode" :options="modeOptions" />
                    </div>
                </template>
            </SettingsRow>
            <SettingsRow>
                <template #label>{{ formatMessage(messages.style) }}</template>
                <template #control>
                    <div class="w-full">
                        <Combobox v-model="settings.style" :options="styleOptions" />
                    </div>
                </template>
            </SettingsRow>
            <SettingsRow stacked>
                <template #label>{{ formatMessage(messages.stylePreview) }}</template>
                <template #control>
                    <div class="translation-style-preview-container">
                        <p
                            v-if="settings.mode === 'bilingual'"
                            class="translation-style-preview-original m-0"
                        >
                            {{ formatMessage(messages.stylePreviewOriginalText) }}
                        </p>
                        <p class="translation-style-preview m-0" :class="stylePreviewClass">
                            {{ formatMessage(messages.stylePreviewText) }}
                        </p>
                    </div>
                </template>
            </SettingsRow>
            <SettingsRow>
                <template #label>
                    <span id="settings-target-translation-auto-translate" tabindex="-1">
                        {{ formatMessage(messages.autoTranslate) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.autoTranslateDescription)
                }}</template>
                <template #control
                    ><Toggle id="translation-auto" v-model="settings.auto_translate"
                /></template>
            </SettingsRow>
            <SettingsRow>
                <template #label>
                    <span id="settings-target-translation-cache" tabindex="-1">
                        {{ formatMessage(messages.cache) }}
                    </span>
                </template>
                <template #description>{{ formatMessage(messages.cacheDescription) }}</template>
                <template #control>
                    <div class="flex flex-wrap items-center justify-end gap-2">
                        <span
                            v-if="cacheStatus"
                            class="text-sm text-[var(--color-text-tertiary)]"
                            >{{ cacheStatus }}</span
                        >
                        <Button type="base" @click="clearCache">
                            <TrashIcon />{{ formatMessage(messages.clearCache) }}
                        </Button>
                    </div>
                </template>
            </SettingsRow>
        </SettingsSection>
    </div>
</template>

<style scoped>
.translation-model-combobox.has-model-icon :deep(input) {
    padding-left: 2.75rem !important;
}

.translation-style-preview-container {
    display: flex;
    width: 100%;
    min-height: 6.5rem;
    flex-direction: column;
    box-sizing: border-box;
    gap: 0.75rem;
    padding: 1rem;
    border: 1px solid var(--color-divider);
    border-radius: var(--radius-lg);
}

.translation-style-preview-original,
.translation-style-preview {
    font-weight: 400;
}

.translation-style-preview-original,
.translation-style-preview-default {
    color: var(--color-text-primary);
}

.translation-style-preview-weakened {
    color: var(--color-text-tertiary) !important;
}

.translation-style-preview-blur {
    filter: blur(4px);
    opacity: 0.75;
    transition:
        filter 0.1s ease-in-out,
        opacity 0.1s ease-in-out;
}

.translation-style-preview-blur:hover {
    filter: blur(0);
    opacity: 1;
}

.translation-style-preview-blockquote {
    padding: 4px 0 4px 8px;
    border-left: 4px solid var(--color-brand);
}

.translation-style-preview-dashed-line {
    text-decoration: underline dashed var(--color-brand) !important;
    text-underline-offset: 5px;
}

.translation-style-preview-border {
    padding: 2px 4px;
    border: 1px solid var(--color-brand);
    border-radius: 4px;
}

.translation-style-preview-text-color {
    color: oklch(0.693 0.17 162.48) !important;
}

.translation-style-preview-background {
    padding: 2px 4px;
    border-radius: 4px;
    background-color: color-mix(in srgb, var(--color-brand) 15%, transparent);
}
</style>
