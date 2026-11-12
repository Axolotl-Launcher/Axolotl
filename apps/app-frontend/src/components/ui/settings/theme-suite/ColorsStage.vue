<script setup lang="ts">
import { Button, ColorPicker } from '@modrinth/ui'
import { computed } from 'vue'

import { injectThemeSuiteContext } from './context'
import { themeSuiteMessages } from './messages'

const { formatMessage, colorsDraft, colorsBucket, error, installing, resetColors } =
    injectThemeSuiteContext()

const bucketLabel = computed(() =>
    formatMessage(
        colorsBucket.value === 'light'
            ? themeSuiteMessages.themeLight
            : themeSuiteMessages.themeDark,
    ),
)
</script>

<template>
    <div class="flex min-w-0 flex-col gap-4">
        <div class="flex flex-col gap-1">
            <h3 class="m-0 text-base font-semibold text-[var(--color-text-primary)]">
                {{ formatMessage(themeSuiteMessages.colorsTitle) }}
            </h3>
            <p class="m-0 text-sm text-secondary">
                {{ formatMessage(themeSuiteMessages.colorsDescription, { theme: bucketLabel }) }}
            </p>
        </div>

        <div class="flex flex-col gap-3 rounded-xl border border-solid border-surface-4 p-4">
            <ColorPicker
                v-model="colorsDraft.background"
                :label="formatMessage(themeSuiteMessages.background)"
                :disabled="installing"
            />
            <ColorPicker
                v-model="colorsDraft.surface"
                :label="formatMessage(themeSuiteMessages.surface)"
                :disabled="installing"
            />
            <ColorPicker
                v-model="colorsDraft.text"
                :label="formatMessage(themeSuiteMessages.text)"
                :disabled="installing"
            />
            <Button type="outlined" @click="resetColors">
                {{ formatMessage(themeSuiteMessages.resetColors) }}
            </Button>
        </div>

        <div class="flex flex-col gap-2">
            <span
                class="text-xs font-semibold uppercase tracking-wide text-[var(--color-text-tertiary)]"
            >
                {{ formatMessage(themeSuiteMessages.colorsPreview) }}
            </span>
            <div
                class="rounded-xl border border-solid border-surface-4 p-4"
                :style="{ backgroundColor: colorsDraft.background }"
            >
                <div
                    class="rounded-lg p-4 text-sm"
                    :style="{
                        backgroundColor: colorsDraft.surface,
                        color: colorsDraft.text,
                    }"
                >
                    {{ formatMessage(themeSuiteMessages.colorsPreviewText) }}
                </div>
            </div>
        </div>

        <div v-if="error" class="flex flex-col gap-1" role="alert">
            <p class="m-0 break-all text-sm text-[var(--color-red)]">{{ error }}</p>
        </div>
    </div>
</template>
