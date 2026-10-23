<script setup lang="ts">
import Combobox, { type ComboboxOption } from '@modrinth/ui/src/components/base/Combobox.vue'
import Toggle from '@modrinth/ui/src/components/base/Toggle.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, ref, watchEffect } from 'vue'

import { type AIProviderDefinition, useAIBackend } from '../../settings/categories/ai/ai-backend'
import AIIcon from '../../settings/categories/ai/AIIcon.vue'
import { useTranslationBackend } from '../../settings/categories/translation/translation-backend'
import type { ModTranslationOptions } from './types'

/** Port of `components/lab/mod-translation/ModTranslationSettingsPanel.vue` over the window's AI settings. */

const props = defineProps<{
    modelValue: ModTranslationOptions
    providerId: string
    modelId: string
}>()

const emit = defineEmits<{
    'update:modelValue': [value: ModTranslationOptions]
    'update:providerId': [value: string]
    'update:modelId': [value: string]
}>()

const { formatMessage } = useVIntl()
const { sharedAIState, getAICatalog, getAIState } = useAIBackend()
const { getTranslationSettings } = useTranslationBackend()
const aiCatalog = ref<AIProviderDefinition[]>([])
const loading = ref(true)
const aiLoadFailed = ref(false)

const messages = defineMessages({
    provider: { id: 'app.lab.mod-translation.provider', defaultMessage: 'AI provider' },
    model: { id: 'app.lab.mod-translation.model', defaultMessage: 'Text model' },
    aiNotConfigured: {
        id: 'app.lab.mod-translation.ai-not-configured',
        defaultMessage:
            'AI is not configured. Open the AI settings to enable a provider and model.',
    },
    aiLoadError: {
        id: 'app.lab.mod-translation.ai-load-error',
        defaultMessage: 'AI settings could not be loaded. Check your connection and try again.',
    },
    options: { id: 'app.lab.mod-translation.options', defaultMessage: 'Options' },
    batchSize: { id: 'app.lab.mod-translation.batch-size', defaultMessage: 'Batch size' },
    generateModName: {
        id: 'app.lab.mod-translation.generate-mod-name',
        defaultMessage: 'AI-generate a Chinese mod name',
    },
    repairEnabled: {
        id: 'app.lab.mod-translation.repair-enabled',
        defaultMessage: 'Repair difficult translations after verification',
    },
    classTextEnabled: {
        id: 'app.lab.mod-translation.class-text-enabled',
        defaultMessage: 'Rewrite advanced .class text candidates',
    },
})

const configuredProviders = computed(() =>
    (sharedAIState.value?.providers ?? []).filter(
        (provider) => provider.enabled && provider.models.some((model) => model.enabled),
    ),
)
const aiAvailable = computed(
    () => !!sharedAIState.value?.settings.enabled && configuredProviders.value.length > 0,
)

const providerOptions = computed<ComboboxOption<string>[]>(() =>
    configuredProviders.value.map((provider) => ({
        value: provider.provider_id,
        label:
            provider.custom_name ||
            aiCatalog.value.find((definition) => definition.id === provider.provider_id)?.name ||
            provider.provider_id,
    })),
)

const modelOptions = computed<ComboboxOption<string>[]>(() =>
    (
        configuredProviders.value.find((provider) => provider.provider_id === props.providerId)
            ?.models ?? []
    )
        .filter((model) => model.enabled)
        .map((model) => ({ value: model.id, label: model.name || model.id })),
)

const batchSizeOptions: ComboboxOption<string>[] = [
    { value: '20', label: '20' },
    { value: '40', label: '40' },
    { value: '80', label: '80' },
]

watchEffect(() => {
    const providers = configuredProviders.value
    if (!providers.length) return
    if (!providers.some((provider) => provider.provider_id === props.providerId)) {
        const first = providers[0]
        emit('update:providerId', first.provider_id)
        emit('update:modelId', first.models.find((model) => model.enabled)?.id ?? '')
    }
})

function selectProvider(value: string) {
    emit('update:providerId', value)
    const provider = configuredProviders.value.find((item) => item.provider_id === value)
    emit('update:modelId', provider?.models.find((model) => model.enabled)?.id ?? '')
}

async function loadDefaults() {
    try {
        const [settingsResult, stateResult, catalogResult] = await Promise.allSettled([
            getTranslationSettings(),
            getAIState(),
            getAICatalog(),
        ])
        if (catalogResult.status === 'fulfilled') aiCatalog.value = catalogResult.value
        aiLoadFailed.value = stateResult.status === 'rejected' && !sharedAIState.value
        const settings = settingsResult.status === 'fulfilled' ? settingsResult.value : null
        if (
            settings?.ai_provider_id &&
            settings.ai_model_id &&
            (!props.providerId || !props.modelId)
        ) {
            emit('update:providerId', settings.ai_provider_id)
            emit('update:modelId', settings.ai_model_id)
        }
    } finally {
        loading.value = false
    }
}

loadDefaults()
</script>

<template>
    <div class="flex flex-col gap-4">
        <div v-if="loading" class="text-sm text-[var(--color-text-tertiary)]">…</div>
        <template v-else>
            <div v-if="aiLoadFailed" class="text-sm text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.aiLoadError) }}
            </div>
            <div v-else-if="!aiAvailable" class="text-sm text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.aiNotConfigured) }}
            </div>
            <div v-else class="grid grid-cols-1 gap-4 md:grid-cols-2">
                <label class="flex flex-col gap-2 font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.provider) }}
                    <Combobox
                        :model-value="providerId"
                        :options="providerOptions"
                        @update:model-value="selectProvider"
                    >
                        <template #selected="{ label }">
                            <span class="inline-flex min-w-0 items-center gap-2">
                                <AIIcon kind="provider-avatar" :value="providerId" :size="20" />
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
                </label>
                <label class="flex flex-col gap-2 font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.model) }}
                    <div class="model-combobox relative" :class="{ 'has-model-icon': modelId }">
                        <AIIcon
                            v-if="modelId"
                            class="pointer-events-none absolute left-3 top-1/2 z-[2] -translate-y-1/2"
                            kind="model"
                            :value="modelId"
                            :size="20"
                        />
                        <Combobox
                            :model-value="modelId"
                            :options="modelOptions"
                            searchable
                            @update:model-value="emit('update:modelId', String($event))"
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
                </label>
            </div>
        </template>

        <div class="flex flex-col gap-3">
            <h3 class="m-0 text-sm font-semibold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.options) }}
            </h3>
            <label
                class="flex items-center justify-between gap-3 text-sm text-[var(--color-text-default)]"
            >
                <span>{{ formatMessage(messages.batchSize) }}</span>
                <div class="w-36">
                    <Combobox
                        :model-value="String(modelValue.batchSize)"
                        :options="batchSizeOptions"
                        @update:model-value="
                            emit('update:modelValue', { ...modelValue, batchSize: Number($event) })
                        "
                    />
                </div>
            </label>
            <label
                class="flex items-center justify-between gap-3 text-sm text-[var(--color-text-default)]"
            >
                <span>{{ formatMessage(messages.generateModName) }}</span>
                <Toggle
                    :model-value="modelValue.generateModName"
                    @update:model-value="
                        emit('update:modelValue', {
                            ...modelValue,
                            generateModName: Boolean($event),
                        })
                    "
                />
            </label>
            <label
                class="flex items-center justify-between gap-3 text-sm text-[var(--color-text-default)]"
            >
                <span>{{ formatMessage(messages.repairEnabled) }}</span>
                <Toggle
                    :model-value="modelValue.repairEnabled"
                    @update:model-value="
                        emit('update:modelValue', { ...modelValue, repairEnabled: Boolean($event) })
                    "
                />
            </label>
            <label
                class="flex items-center justify-between gap-3 text-sm text-[var(--color-text-default)]"
            >
                <span>{{ formatMessage(messages.classTextEnabled) }}</span>
                <Toggle
                    :model-value="modelValue.classTextEnabled"
                    @update:model-value="
                        emit('update:modelValue', {
                            ...modelValue,
                            classTextEnabled: Boolean($event),
                        })
                    "
                />
            </label>
        </div>
    </div>
</template>

<style scoped>
.model-combobox.has-model-icon :deep(input) {
    padding-left: 2.75rem !important;
}
</style>
