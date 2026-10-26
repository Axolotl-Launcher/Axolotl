<script setup lang="ts">
import { defineMessages, injectNotificationManager, Toggle, useVIntl } from '@modrinth/ui'
import { ref, watch } from 'vue'

import { get, update } from '@/helpers/settings.ts'
import { createSettingsPatchSaver } from '@/helpers/settings-patch'

import SettingsRow from './SettingsRow.vue'
import SettingsSection from './SettingsSection.vue'

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()

const messages = defineMessages({
    sectionTitle: {
        id: 'app.settings.content-display.title',
        defaultMessage: 'Content display',
    },
    chineseTitles: {
        id: 'app.settings.content-display.chinese-titles',
        defaultMessage: 'Hide Chinese names in content lists',
    },
    chineseTitlesDescription: {
        id: 'app.settings.content-display.chinese-titles-description',
        defaultMessage: 'Show the original project title instead of the known Chinese name.',
    },
})

const settings = ref(await get())
const saveDraft = createSettingsPatchSaver(settings.value, update)

watch(
    settings,
    () => {
        // The saver snapshots the draft itself; only the changed keys are sent.
        void saveDraft(settings.value).catch(handleError)
    },
    { deep: true },
)
</script>

<template>
    <SettingsSection
        :title="formatMessage(messages.sectionTitle)"
        title-id="settings-target-content-display"
    >
        <SettingsRow>
            <template #label>
                <span id="settings-target-content-titles" tabindex="-1">{{
                    formatMessage(messages.chineseTitles)
                }}</span>
            </template>
            <template #description>{{ formatMessage(messages.chineseTitlesDescription) }}</template>
            <template #control>
                <Toggle
                    id="hide-chinese-content-titles"
                    v-model="settings.hide_chinese_content_titles"
                />
            </template>
        </SettingsRow>
    </SettingsSection>
</template>
