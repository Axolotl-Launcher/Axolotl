<script setup lang="ts">
import { RefreshCwIcon, SettingsIcon, TagCategoryWifiOffIcon } from '@modrinth/assets'
import { Button, useVIntl } from '@modrinth/ui'
import { openUrl } from '@tauri-apps/plugin-opener'
import { platform } from '@tauri-apps/plugin-os'
import { ref } from 'vue'

import { useNetworkStatus } from '@/composables/useNetworkStatus'

import { networkMessages as messages } from './messages'

const { formatMessage } = useVIntl()
const { retrying, retryConnection } = useNetworkStatus()
const isWindows = platform() === 'windows'
const openingSettings = ref(false)
const settingsFailed = ref(false)

async function openNetworkSettings() {
    if (!isWindows || openingSettings.value) return
    openingSettings.value = true
    settingsFailed.value = false
    try {
        await openUrl('ms-settings:network-status')
    } catch {
        settingsFailed.value = true
    } finally {
        openingSettings.value = false
    }
}
</script>

<template>
    <section
        class="flex min-h-full flex-col items-center justify-center gap-4 p-6 text-center"
        role="status"
    >
        <TagCategoryWifiOffIcon
            class="size-16 text-[var(--color-text-tertiary)]"
            aria-hidden="true"
        />
        <h1 class="m-0 text-3xl font-bold text-[var(--color-text-primary)]">
            {{ formatMessage(messages.title) }}
        </h1>
        <p class="m-0 max-w-lg text-base leading-relaxed text-[var(--color-text-tertiary)]">
            {{ formatMessage(messages.description) }}
        </p>
        <div class="flex flex-wrap items-center justify-center gap-3">
            <Button
                v-if="isWindows"
                type="base"
                :loading="openingSettings"
                @click="openNetworkSettings"
            >
                <SettingsIcon aria-hidden="true" />
                {{ formatMessage(messages.settings) }}
            </Button>
            <Button type="colored" color="brand" :loading="retrying" @click="retryConnection">
                <RefreshCwIcon aria-hidden="true" />
                {{ formatMessage(messages.retry) }}
            </Button>
        </div>
        <p v-if="settingsFailed" class="m-0 text-sm text-[var(--color-red)]" role="alert">
            {{ formatMessage(messages.settingsFailed) }}
        </p>
    </section>
</template>
