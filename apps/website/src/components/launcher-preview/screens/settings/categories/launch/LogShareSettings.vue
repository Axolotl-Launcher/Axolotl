<script setup lang="ts">
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Combobox from '@modrinth/ui/src/components/base/Combobox.vue'
import Toggle from '@modrinth/ui/src/components/base/Toggle.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'

import SettingsRow from '../../components/SettingsRow.vue'
import SettingsSection from '../../components/SettingsSection.vue'
import { messages } from '../../messages'
import { type AIState, useAIBackend } from '../ai/ai-backend'
import LogAgentSourcePicker from './LogAgentSourcePicker.vue'
import LogAgentTour from './LogAgentTour.vue'
import { type AnalysisSettings, type LogShareSettings, useLaunchBackend } from './state'

/**
 * Crash analysis and diagnostic sharing (apps/app-frontend/src/components/ui/
 * settings/LogShareSettings.vue): the default analysis source, the local LogAgent
 * model or the cloud options, the sharing service and the progress display.
 * Every change saves on its own; the controls wait while one is saving.
 */
const props = defineProps<{ localSetup?: boolean }>()
const emit = defineEmits<{ openProviders: []; savingChange: [saving: boolean] }>()

const { formatMessage } = useVIntl()
const router = useRouter()
const { handleError } = injectNotificationManager()
const { sharedAIState, getAIState } = useAIBackend()
const {
    get_crash_analysis_ai_settings,
    get_log_share_settings,
    update_crash_analysis_ai_settings,
    update_log_share_settings,
} = useLaunchBackend()

/** The app's onboarding overlay in its LogAgent replay mode. */
const tourVisible = ref(false)

/** `replayOnboarding('logagent')` (App.vue): reopens this page, then starts the tour. */
async function replayOnboarding(mode: 'logagent') {
    if (mode === 'logagent') await router.replace('/settings#launch-defaults')
    tourVisible.value = true
}

const settings = ref<LogShareSettings>({
    share_provider: 'logshare',
    ai_source: 'logshare',
    auto_upload: true,
    multi_file: true,
    no_storage: false,
    show_progress: true,
})
const aiSettings = ref<AnalysisSettings>({
    enabled: false,
    provider_id: '',
    model_id: '',
    ai_source: 'logshare',
})
const loading = ref(true)
const saving = ref(false)

const emptyState: AIState = { settings: { enabled: false }, catalog_source: '', providers: [] }
const aiState = computed(() => sharedAIState.value ?? emptyState)
const providers = computed(() =>
    aiState.value.providers.filter(
        (provider) => provider.enabled && provider.models.some((model) => model.enabled),
    ),
)
const providerOptions = computed(() =>
    providers.value.map((provider) => ({
        value: provider.provider_id,
        label: provider.custom_name || provider.provider_id,
    })),
)
const modelOptions = computed(
    () =>
        providers.value
            .find((provider) => provider.provider_id === aiSettings.value.provider_id)
            ?.models.filter((model) => model.enabled)
            .map((model) => ({ value: model.id, label: model.name || model.id })) ?? [],
)

const isCustom = computed(() => aiSettings.value.ai_source === 'custom')

const shareProviderOptions = [
    { value: 'logshare', label: 'LogShare' },
    { value: 'mclogs', label: 'mclo.gs' },
]

async function saveShare(next: LogShareSettings): Promise<void> {
    if (saving.value) return
    saving.value = true
    emit('savingChange', true)
    try {
        await update_log_share_settings(next)
        settings.value = next
    } catch (error) {
        handleError(error)
    } finally {
        saving.value = false
        emit('savingChange', false)
    }
}

async function saveAI(next: AnalysisSettings): Promise<void> {
    if (saving.value) return
    saving.value = true
    emit('savingChange', true)
    try {
        await update_crash_analysis_ai_settings(next)
        aiSettings.value = next
        settings.value = { ...settings.value, ai_source: next.ai_source }
    } catch (error) {
        handleError(error)
    } finally {
        saving.value = false
        emit('savingChange', false)
    }
}

function updateShare(patch: Partial<LogShareSettings>): void {
    void saveShare({ ...settings.value, ...patch })
}

function selectShareProvider(value: unknown): void {
    const provider = value === 'mclogs' ? 'mclogs' : 'logshare'
    updateShare({ share_provider: provider, ai_source: aiSettings.value.ai_source })
}

function selectAiSource(value: unknown): void {
    const source = value === 'custom' ? 'custom' : 'logshare'
    if (source === aiSettings.value.ai_source) return
    void saveAI({ ...aiSettings.value, ai_source: source })
}

function updateEnabled(enabled: boolean): void {
    void saveAI({ ...aiSettings.value, enabled })
}

function updateProvider(providerId: string): void {
    const provider = providers.value.find((item) => item.provider_id === providerId)
    const modelId = provider?.models.find((model) => model.enabled)?.id ?? ''
    void saveAI({ ...aiSettings.value, provider_id: providerId, model_id: modelId })
}

function updateModel(modelId: string): void {
    void saveAI({ ...aiSettings.value, model_id: modelId })
}

function openProviders(): void {
    if (props.localSetup) emit('openProviders')
    else void router.push('/settings#ai')
}

onMounted(async () => {
    try {
        const [nextShare, nextAI] = await Promise.all([
            get_log_share_settings(),
            get_crash_analysis_ai_settings(),
            getAIState().catch(handleError),
        ])
        settings.value = { ...nextShare, ai_source: nextAI.ai_source }
        aiSettings.value = nextAI
    } catch (error) {
        handleError(error)
    } finally {
        loading.value = false
    }
})
</script>

<template>
    <div v-if="!loading" class="flex min-w-0 flex-col gap-6">
        <SettingsSection
            :title="
                formatMessage(
                    localSetup
                        ? messages.appLogagentSettingsLocalSetupTitle
                        : messages.appLogagentSettingsAnalysisTitle,
                )
            "
            :description="
                formatMessage(
                    localSetup
                        ? messages.appLogagentSettingsLocalSetupDescription
                        : messages.appLogagentSettingsAnalysisDescription,
                )
            "
        >
            <template #extra
                ><Button v-if="!localSetup" type="quiet" @click="replayOnboarding('logagent')">{{
                    formatMessage(messages.appLogagentSettingsTour)
                }}</Button></template
            >
            <div
                v-if="!localSetup"
                id="settings-target-log-share"
                data-onboarding-id="logagent-settings-source"
                class="p-4"
            >
                <LogAgentSourcePicker
                    :model-value="aiSettings.ai_source"
                    :disabled="saving"
                    @update:model-value="selectAiSource"
                />
            </div>
            <template v-if="isCustom || localSetup">
                <SettingsRow inline-control>
                    <template #label>{{
                        formatMessage(messages.appCrashAnalysisAiSettingsEnabled)
                    }}</template>
                    <template #description>{{
                        formatMessage(messages.appCrashAnalysisAiSettingsEnabledDescription)
                    }}</template>
                    <template #control
                        ><Toggle
                            id="crash-analysis-ai-enabled"
                            :model-value="aiSettings.enabled"
                            :disabled="saving || !aiState.settings.enabled || !providers.length"
                            @update:model-value="updateEnabled(!!$event)"
                    /></template>
                </SettingsRow>
                <template v-if="providers.length">
                    <SettingsRow>
                        <template #label>{{
                            formatMessage(messages.appCrashAnalysisAiSettingsProvider)
                        }}</template>
                        <template #control
                            ><Combobox
                                id="crash-analysis-ai-provider"
                                :model-value="aiSettings.provider_id"
                                :options="providerOptions"
                                :disabled="saving"
                                @update:model-value="updateProvider(String($event))"
                        /></template>
                    </SettingsRow>
                    <SettingsRow>
                        <template #label>{{
                            formatMessage(messages.appCrashAnalysisAiSettingsModel)
                        }}</template>
                        <template #control
                            ><Combobox
                                id="crash-analysis-ai-model"
                                :model-value="aiSettings.model_id"
                                :options="modelOptions"
                                :disabled="saving || !aiSettings.provider_id"
                                @update:model-value="updateModel(String($event))"
                        /></template>
                    </SettingsRow>
                    <details class="px-4 py-3 text-sm text-[var(--color-text-tertiary)]">
                        <summary class="cursor-pointer">
                            {{ formatMessage(messages.appLogagentSettingsModelHelp) }}
                        </summary>
                        <p class="m-0 mt-2 leading-relaxed">
                            {{ formatMessage(messages.appLogagentSettingsSupportedModels) }}
                        </p>
                    </details>
                </template>
                <div
                    v-if="!providers.length || !aiState.settings.enabled"
                    class="flex flex-col items-start gap-2 px-4 py-3"
                >
                    <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                        {{ formatMessage(messages.appCrashAnalysisAiSettingsNoProviders) }}
                    </p>
                    <Button type="outlined" @click="openProviders">{{
                        formatMessage(messages.appLogagentSettingsManageModels)
                    }}</Button>
                </div>
            </template>
            <div v-else data-onboarding-id="logagent-settings-cloud">
                <SettingsRow inline-control>
                    <template #label>{{
                        formatMessage(messages.appLogShareSettingsNoStorage)
                    }}</template>
                    <template #description>{{
                        formatMessage(messages.appLogShareSettingsNoStorageDescription)
                    }}</template>
                    <template #control
                        ><Toggle
                            id="log-share-no-storage"
                            :model-value="settings.no_storage"
                            :disabled="saving"
                            @update:model-value="updateShare({ no_storage: !!$event })"
                    /></template>
                </SettingsRow>
                <SettingsRow inline-control>
                    <template #label>{{
                        formatMessage(messages.appLogShareSettingsAutoUpload)
                    }}</template>
                    <template #description>{{
                        formatMessage(messages.appLogShareSettingsAutoUploadDescription)
                    }}</template>
                    <template #control
                        ><Toggle
                            id="log-share-auto-upload"
                            :model-value="settings.auto_upload"
                            :disabled="saving || settings.no_storage"
                            @update:model-value="updateShare({ auto_upload: !!$event })"
                    /></template>
                </SettingsRow>
            </div>
        </SettingsSection>
        <SettingsSection
            v-if="!localSetup"
            data-onboarding-id="logagent-settings-sharing"
            :title="formatMessage(messages.appLogagentSettingsSharingTitle)"
            :description="formatMessage(messages.appLogagentSettingsSharingDescription)"
        >
            <SettingsRow>
                <template #label>{{
                    formatMessage(messages.appLogShareSettingsShareProvider)
                }}</template>
                <template #control
                    ><Combobox
                        id="log-share-provider"
                        :model-value="settings.share_provider"
                        :options="shareProviderOptions"
                        :disabled="saving"
                        @update:model-value="selectShareProvider"
                /></template>
            </SettingsRow>
            <SettingsRow inline-control>
                <template #label>{{
                    formatMessage(messages.appLogShareSettingsMultiFile)
                }}</template>
                <template #description>{{
                    formatMessage(messages.appLogShareSettingsMultiFileDescription)
                }}</template>
                <template #control
                    ><Toggle
                        id="log-share-multi-file"
                        :model-value="settings.multi_file"
                        :disabled="saving"
                        @update:model-value="updateShare({ multi_file: !!$event })"
                /></template>
            </SettingsRow>
        </SettingsSection>
        <SettingsSection
            v-if="!localSetup"
            :title="formatMessage(messages.appLogagentSettingsDisplayTitle)"
        >
            <SettingsRow inline-control>
                <template #label>{{
                    formatMessage(messages.appLogShareSettingsShowProgress)
                }}</template>
                <template #description>{{
                    formatMessage(messages.appLogShareSettingsShowProgressDescription)
                }}</template>
                <template #control
                    ><Toggle
                        id="log-share-show-progress"
                        :model-value="settings.show_progress"
                        :disabled="saving"
                        @update:model-value="updateShare({ show_progress: !!$event })"
                /></template>
            </SettingsRow>
        </SettingsSection>
        <LogAgentTour
            v-if="!localSetup"
            :visible="tourVisible"
            @complete="tourVisible = false"
            @skip="tourVisible = false"
        />
    </div>
</template>
