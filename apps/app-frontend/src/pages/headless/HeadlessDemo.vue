<script setup lang="ts">
import { CheckIcon } from '@modrinth/assets'
import { defineMessages, useVIntl } from '@modrinth/ui'
import { CheckboxIndicator, CheckboxRoot } from 'reka-ui'
import { ref } from 'vue'

import HeadlessButton from '@/components/ui/headless/HeadlessButton.vue'
import HeadlessDialog from '@/components/ui/headless/HeadlessDialog.vue'
import HeadlessSelect from '@/components/ui/headless/HeadlessSelect.vue'
import HeadlessTooltip from '@/components/ui/headless/HeadlessTooltip.vue'
import { headlessTokenClasses } from '@/components/ui/headless/token-classes'

const { formatMessage } = useVIntl()

const acceptTerms = ref(false)
const selectedLoader = ref('fabric')
const dialogOpen = ref(false)

const loaderOptions = [
    { value: 'vanilla', labelKey: 'loaderVanilla' },
    { value: 'fabric', labelKey: 'loaderFabric' },
    { value: 'forge', labelKey: 'loaderForge' },
    { value: 'neoforge', labelKey: 'loaderNeoForge' },
] as const

const surfaces = [
    { token: 'surface-1', className: 'bg-surface-1' },
    { token: 'surface-2', className: 'bg-surface-2' },
    { token: 'surface-3', className: 'bg-surface-3' },
    { token: 'surface-4', className: 'bg-surface-4' },
    { token: 'surface-5', className: 'bg-surface-5' },
] as const

const messages = defineMessages({
    title: { id: 'app.headless-demo.title', defaultMessage: 'Headless token demo' },
    description: {
        id: 'app.headless-demo.description',
        defaultMessage:
            'Isolated reka-ui primitives styled only with existing app tokens. Switch theme in Settings → Appearance to verify light, dark, and OLED.',
    },
    themeHint: {
        id: 'app.headless-demo.theme-hint',
        defaultMessage:
            'No second theme system is introduced here — surfaces and brand colors follow the active theme.',
    },
    surfacesHeading: { id: 'app.headless-demo.surfaces', defaultMessage: 'Surfaces' },
    controlsHeading: { id: 'app.headless-demo.controls', defaultMessage: 'Controls' },
    openDialog: { id: 'app.headless-demo.open-dialog', defaultMessage: 'Open dialog' },
    dialogTitle: { id: 'app.headless-demo.dialog-title', defaultMessage: 'Token-mapped dialog' },
    dialogDescription: {
        id: 'app.headless-demo.dialog-description',
        defaultMessage:
            'This dialog content uses bg-surface-3, text-[var(--color-text-primary)], and border-surface-5. Overlay and focus ring stay readable in every theme.',
    },
    dialogClose: { id: 'app.headless-demo.dialog-close', defaultMessage: 'Close' },
    tooltipTrigger: { id: 'app.headless-demo.tooltip-trigger', defaultMessage: 'Hover tooltip' },
    tooltipContent: {
        id: 'app.headless-demo.tooltip-content',
        defaultMessage: 'Tooltip body mapped to surface-4 / text-[var(--color-text-primary)].',
    },
    selectLabel: { id: 'app.headless-demo.select-label', defaultMessage: 'Loader' },
    selectPlaceholder: {
        id: 'app.headless-demo.select-placeholder',
        defaultMessage: 'Pick a loader',
    },
    loaderVanilla: { id: 'app.headless-demo.loader-vanilla', defaultMessage: 'Vanilla' },
    loaderFabric: { id: 'app.headless-demo.loader-fabric', defaultMessage: 'Fabric' },
    loaderForge: { id: 'app.headless-demo.loader-forge', defaultMessage: 'Forge' },
    loaderNeoForge: { id: 'app.headless-demo.loader-neoforge', defaultMessage: 'NeoForge' },
    checkboxLabel: {
        id: 'app.headless-demo.checkbox-label',
        defaultMessage: 'Accept demo checkbox (brand fill when checked)',
    },
    brandButton: { id: 'app.headless-demo.brand-button', defaultMessage: 'Brand button' },
    standardButton: { id: 'app.headless-demo.standard-button', defaultMessage: 'Standard button' },
})
</script>

<template>
    <main class="flex w-full flex-col gap-6 p-6">
        <header class="flex min-w-0 flex-col gap-1">
            <h1 class="m-0 text-2xl font-bold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.title) }}
            </h1>
            <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.description) }}
            </p>
            <p class="m-0 text-xs text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.themeHint) }}
            </p>
        </header>

        <section class="flex flex-col gap-3" :aria-label="formatMessage(messages.surfacesHeading)">
            <h2 class="m-0 text-base font-bold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.surfacesHeading) }}
            </h2>
            <div class="flex flex-wrap gap-2">
                <div
                    v-for="surface in surfaces"
                    :key="surface.token"
                    class="flex h-16 min-w-24 flex-1 items-end rounded-[var(--radius-md)] border border-surface-5 p-2"
                    :class="surface.className"
                >
                    <span class="font-mono text-xs text-[var(--color-text-default)]">{{
                        surface.token
                    }}</span>
                </div>
            </div>
        </section>

        <section class="flex flex-col gap-4" :aria-label="formatMessage(messages.controlsHeading)">
            <h2 class="m-0 text-base font-bold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.controlsHeading) }}
            </h2>

            <div class="flex flex-wrap items-center gap-3">
                <HeadlessDialog
                    v-model:open="dialogOpen"
                    :close-label="formatMessage(messages.dialogClose)"
                >
                    <HeadlessButton color="brand">
                        {{ formatMessage(messages.openDialog) }}
                    </HeadlessButton>
                    <template #title>
                        {{ formatMessage(messages.dialogTitle) }}
                    </template>
                    <template #description>
                        {{ formatMessage(messages.dialogDescription) }}
                    </template>
                    <template #body>
                        <div class="mt-4 flex justify-end">
                            <HeadlessButton @click="dialogOpen = false">
                                {{ formatMessage(messages.dialogClose) }}
                            </HeadlessButton>
                        </div>
                    </template>
                </HeadlessDialog>

                <HeadlessTooltip>
                    <HeadlessButton>{{ formatMessage(messages.tooltipTrigger) }}</HeadlessButton>
                    <template #content>
                        {{ formatMessage(messages.tooltipContent) }}
                    </template>
                </HeadlessTooltip>
            </div>

            <div class="flex flex-wrap items-end gap-4">
                <label
                    class="flex min-w-40 flex-col gap-1 text-sm text-[var(--color-text-tertiary)]"
                >
                    <span>{{ formatMessage(messages.selectLabel) }}</span>
                    <HeadlessSelect
                        id="headless-demo-loader"
                        v-model="selectedLoader"
                        :name="formatMessage(messages.selectLabel)"
                        :placeholder="formatMessage(messages.selectPlaceholder)"
                        :options="
                            loaderOptions.map((option) => ({
                                value: option.value,
                                label: formatMessage(messages[option.labelKey]),
                            }))
                        "
                    />
                </label>

                <label
                    class="flex cursor-pointer items-center gap-2 text-sm text-[var(--color-text-primary)]"
                >
                    <CheckboxRoot v-model="acceptTerms" :class="headlessTokenClasses.checkboxRoot">
                        <CheckboxIndicator class="flex items-center justify-center">
                            <CheckIcon class="size-3.5" />
                        </CheckboxIndicator>
                    </CheckboxRoot>
                    <span>{{ formatMessage(messages.checkboxLabel) }}</span>
                </label>
            </div>

            <div class="flex flex-wrap gap-3">
                <HeadlessButton color="brand">
                    {{ formatMessage(messages.brandButton) }}
                </HeadlessButton>
                <HeadlessButton>
                    {{ formatMessage(messages.standardButton) }}
                </HeadlessButton>
            </div>
        </section>
    </main>
</template>
