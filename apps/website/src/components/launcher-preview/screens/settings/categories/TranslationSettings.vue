<script setup lang="ts">
import PlugIcon from '@modrinth/assets/icons/plug.svg?component'
import SpinnerIcon from '@modrinth/assets/icons/spinner.svg?component'
import TrashIcon from '@modrinth/assets/icons/trash.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Combobox from '@modrinth/ui/src/components/base/Combobox.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import Toggle from '@modrinth/ui/src/components/base/Toggle.vue'
import { LOCALES, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'

import { usePreview } from '../../../lib/state/index'
import SettingsRow from '../components/SettingsRow.vue'
import SettingsSection from '../components/SettingsSection.vue'
import { messages } from '../messages'
import type { AIProviderDefinition } from './ai/ai-backend'
import AIIcon from './ai/AIIcon.vue'
import {
    getTranslationErrorKind,
    type TranslationProvider,
    type TranslationSettings as TranslationSettingsState,
    type TranslationStyle,
    useTranslationBackend,
} from './translation/translation-backend'

/**
 * Translation settings (apps/app-frontend/src/components/ui/settings/TranslationSettings.vue),
 * the second half of Language & translation. The settings row, the provider
 * test, the cache and the Google IP pool are the launcher's translation
 * backend simulated in the window's memory (translation/translation-backend.ts);
 * the AI provider and model lists come from the AI page's state.
 */
const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const { clock } = usePreview()
const {
    ai,
    clearTranslationCache,
    getGoogleIpPoolSize,
    getTranslationSettings,
    testTranslationProvider,
    updateTranslationSettings,
} = useTranslationBackend()
const { getAICatalog, getAIState, sharedAIState } = ai

const settings = ref<TranslationSettingsState>({
    provider: 'google',
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

const aiCatalog = ref<AIProviderDefinition[]>([])
const loading = ref(true)
const cacheStatus = ref('')
const testing = ref(false)
const testStatus = ref('')
const googleIpPoolSize = ref(0)
/* The app's `setTimeout` / `setInterval` handles, on the window clock. */
let saveTimer: (() => void) | undefined
let poolTimer: (() => void) | undefined

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
            google: messages.appTranslationSettingsProviderGoogle,
            deepl: messages.appTranslationSettingsProviderDeepl,
            ai: messages.appTranslationSettingsProviderAi,
        }[provider],
    )
}

function languageName(code: string) {
    if (code === 'follow-app')
        return formatMessage(messages.appTranslationSettingsTargetLanguageFollowApp)
    const locale = LOCALES.find((item) => item.code === code)
    return locale ? `${locale.name} — ${formatMessage(locale.translatedName)}` : code
}

function styleName(style: TranslationStyle) {
    return formatMessage(
        {
            default: messages.appTranslationSettingsStyleDefault,
            blur: messages.appTranslationSettingsStyleBlur,
            blockquote: messages.appTranslationSettingsStyleBlockquote,
            weakened: messages.appTranslationSettingsStyleWeakened,
            'dashed-line': messages.appTranslationSettingsStyleDashedLine,
            border: messages.appTranslationSettingsStyleBorder,
            'text-color': messages.appTranslationSettingsStyleTextColor,
            background: messages.appTranslationSettingsStyleBackground,
        }[style],
    )
}

const translationProviders = computed<TranslationProvider[]>(() => [
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
        label: formatMessage(
            mode === 'bilingual'
                ? messages.appTranslationSettingsDisplayModeBilingual
                : messages.appTranslationSettingsDisplayModeTranslationOnly,
        ),
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

function reportOperationError(error?: unknown) {
    const errorKind = error ? getTranslationErrorKind(error) : 'provider'
    const errorMessage = error instanceof Error ? error.message : String(error)

    // Don't show error notifications for DeepL when API key is not configured
    // This prevents spam when user is still configuring
    if (
        settings.value.provider === 'deepl' &&
        !settings.value.deepl_api_key?.trim() &&
        errorMessage.includes('DeepL API key is not configured')
    ) {
        return
    }

    const message = error
        ? {
              'rate-limited': messages.appTranslationErrorRateLimited,
              authentication: messages.appTranslationErrorAuthentication,
              'content-too-long': messages.appTranslationErrorContentTooLong,
              network: messages.appTranslationErrorNetwork,
              provider: messages.appTranslationSettingsOperationFailed,
          }[errorKind]
        : messages.appTranslationSettingsOperationFailed
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
    () => {
        if (loading.value) return

        // Deep watchers receive the same object for old and new values when a
        // nested field is mutated in place. Always schedule a save for a loaded
        // settings change instead of comparing those references.
        saveTimer?.()
        saveTimer = clock.after(300, () => {
            saveTimer = undefined
            // Only save settings, don't show error notifications
            // Errors during save should be silent - only test button shows errors
            void updateTranslationSettings({ ...settings.value }).catch(() => {})
        })
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

/** `setInterval(refreshGoogleIpPool, 5000)`; returns the matching `clearInterval`. */
function startPoolInterval() {
    let cancel = () => {}
    const schedule = () => {
        cancel = clock.after(5000, () => {
            void refreshGoogleIpPool()
            schedule()
        })
    }
    schedule()
    return () => cancel()
}

watch(
    () => settings.value.provider,
    (provider) => {
        poolTimer?.()
        poolTimer = undefined
        if (provider !== 'google') return
        void refreshGoogleIpPool()
        poolTimer = startPoolInterval()
    },
    { immediate: true },
)

onUnmounted(() => {
    poolTimer?.()
    if (loading.value || !saveTimer) return
    saveTimer()
    saveTimer = undefined
    void updateTranslationSettings({ ...settings.value }).catch(reportOperationError)
})

onMounted(async () => {
    try {
        const [loadedSettings, , loadedCatalog] = await Promise.all([
            getTranslationSettings(),
            getAIState(),
            getAICatalog(),
        ])
        settings.value = loadedSettings
        aiCatalog.value = loadedCatalog
    } catch (error) {
        reportOperationError(error)
    } finally {
        loading.value = false
    }
})

async function testProvider() {
    testing.value = true
    testStatus.value = ''

    // Validate DeepL configuration before testing
    if (settings.value.provider === 'deepl') {
        if (!settings.value.deepl_api_key?.trim()) {
            reportOperationError(
                new Error('DeepL API key is not configured. Please enter your API key first.'),
            )
            testing.value = false
            return
        }
        if (!settings.value.deepl_api_endpoint?.trim()) {
            settings.value.deepl_api_endpoint = 'https://api-free.deepl.com/v2/translate'
        }
    }

    try {
        await updateTranslationSettings({ ...settings.value })
        const result = await testTranslationProvider(settings.value.provider)
        testStatus.value = formatMessage(messages.appTranslationSettingsTestSuccess, {
            translation: result,
        })
    } catch (error) {
        reportOperationError(error)
    } finally {
        testing.value = false
    }
}

async function clearCache() {
    try {
        await clearTranslationCache()
        cacheStatus.value = formatMessage(messages.appTranslationSettingsCacheCleared)
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
                    {{ formatMessage(messages.appTranslationSettingsTitle) }}
                </h2>
                <p class="m-0 mt-1 text-sm leading-relaxed text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.appTranslationSettingsDescription) }}
                </p>
            </template>
            <template #extra>
                <div class="flex flex-wrap items-center justify-end gap-2">
                    <span v-if="testStatus" class="text-sm text-[var(--color-text-tertiary)]">{{
                        testStatus
                    }}</span>
                    <Button type="base" :disabled="testing" @click="testProvider">
                        <PlugIcon />{{
                            formatMessage(
                                testing
                                    ? messages.appTranslationSettingsTesting
                                    : messages.appTranslationSettingsTest,
                            )
                        }}
                    </Button>
                </div>
            </template>
            <SettingsRow>
                <template #label>{{
                    formatMessage(messages.appTranslationSettingsProvider)
                }}</template>
                <template #description>
                    <span v-if="settings.provider === 'google'">
                        {{
                            formatMessage(messages.appTranslationSettingsGoogleIpPool, {
                                count: googleIpPoolSize,
                            })
                        }}
                    </span>
                </template>
                <template #control>
                    <div class="w-full">
                        <Combobox v-model="settings.provider" :options="providerOptions" />
                    </div>
                </template>
            </SettingsRow>
            <SettingsRow v-if="settings.provider === 'deepl'" stacked>
                <template #label>{{
                    formatMessage(messages.appTranslationSettingsDeeplApiEndpoint)
                }}</template>
                <template #control>
                    <StyledInput
                        v-model="settings.deepl_api_endpoint"
                        :placeholder="
                            formatMessage(
                                messages.appTranslationSettingsDeeplApiEndpointPlaceholder,
                            )
                        "
                        wrapper-class="w-full"
                    />
                </template>
            </SettingsRow>
            <SettingsRow v-if="settings.provider === 'deepl'" stacked>
                <template #label>{{
                    formatMessage(messages.appTranslationSettingsDeeplApiKey)
                }}</template>
                <template #description>{{
                    formatMessage(messages.appTranslationSettingsDeeplApiKeyHint)
                }}</template>
                <template #control>
                    <StyledInput
                        v-model="settings.deepl_api_key"
                        type="password"
                        :placeholder="
                            formatMessage(messages.appTranslationSettingsDeeplApiKeyPlaceholder)
                        "
                        wrapper-class="w-full"
                    />
                </template>
            </SettingsRow>
            <SettingsRow v-if="settings.provider === 'ai' && aiAvailable">
                <template #label>{{
                    formatMessage(messages.appTranslationSettingsAiProvider)
                }}</template>
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
            <SettingsRow v-if="settings.provider === 'ai' && aiAvailable">
                <template #label>{{
                    formatMessage(messages.appTranslationSettingsAiModel)
                }}</template>
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
                <template #label>{{
                    formatMessage(messages.appTranslationSettingsSystemPrompt)
                }}</template>
                <template #description>{{
                    formatMessage(messages.appTranslationSettingsSystemPromptDescription)
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
            :title="formatMessage(messages.appTranslationSettingsDisplaySectionTitle)"
            title-id="settings-target-translation-display"
        >
            <SettingsRow>
                <template #label>{{
                    formatMessage(messages.appTranslationSettingsTargetLanguage)
                }}</template>
                <template #control>
                    <div class="w-full">
                        <Combobox v-model="targetLanguage" :options="languageOptions" searchable />
                    </div>
                </template>
            </SettingsRow>
            <SettingsRow>
                <template #label>{{
                    formatMessage(messages.appTranslationSettingsDisplayMode)
                }}</template>
                <template #control>
                    <div class="w-full">
                        <Combobox v-model="settings.mode" :options="modeOptions" />
                    </div>
                </template>
            </SettingsRow>
            <SettingsRow>
                <template #label>{{
                    formatMessage(messages.appTranslationSettingsStyle)
                }}</template>
                <template #control>
                    <div class="w-full">
                        <Combobox v-model="settings.style" :options="styleOptions" />
                    </div>
                </template>
            </SettingsRow>
            <SettingsRow stacked>
                <template #label>{{
                    formatMessage(messages.appTranslationSettingsStylePreview)
                }}</template>
                <template #control>
                    <div class="translation-style-preview-container">
                        <p
                            v-if="settings.mode === 'bilingual'"
                            class="translation-style-preview-original m-0"
                        >
                            {{
                                formatMessage(
                                    messages.appTranslationSettingsStylePreviewOriginalText,
                                )
                            }}
                        </p>
                        <p class="translation-style-preview m-0" :class="stylePreviewClass">
                            {{ formatMessage(messages.appTranslationSettingsStylePreviewText) }}
                        </p>
                    </div>
                </template>
            </SettingsRow>
            <SettingsRow>
                <template #label>
                    <span id="settings-target-translation-auto-translate" tabindex="-1">
                        {{ formatMessage(messages.appTranslationSettingsAutoTranslate) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.appTranslationSettingsAutoTranslateDescription)
                }}</template>
                <template #control
                    ><Toggle id="translation-auto" v-model="settings.auto_translate"
                /></template>
            </SettingsRow>
            <SettingsRow>
                <template #label>
                    <span id="settings-target-translation-cache" tabindex="-1">
                        {{ formatMessage(messages.appTranslationSettingsCache) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.appTranslationSettingsCacheDescription)
                }}</template>
                <template #control>
                    <div class="flex flex-wrap items-center justify-end gap-2">
                        <span
                            v-if="cacheStatus"
                            class="text-sm text-[var(--color-text-tertiary)]"
                            >{{ cacheStatus }}</span
                        >
                        <Button type="base" @click="clearCache">
                            <TrashIcon />{{
                                formatMessage(messages.appTranslationSettingsClearCache)
                            }}
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
