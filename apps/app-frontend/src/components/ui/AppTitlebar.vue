<script setup lang="ts">
import { LeftArrowIcon, RightArrowIcon } from '@modrinth/assets'
import { commonMessages, defineMessages, useVIntl } from '@modrinth/ui'
import { useRouter } from 'vue-router'

import AxolotlLogo from './AxolotlLogo.vue'
import Breadcrumbs from './Breadcrumbs.vue'

defineProps<{
    isBetaBuild?: boolean
}>()

const router = useRouter()
const { formatMessage } = useVIntl()
const messages = defineMessages({
    betaBuild: { id: 'app.build.beta', defaultMessage: 'Beta' },
    forward: { id: 'app.navigation.forward', defaultMessage: 'Forward' },
})
</script>

<template>
    <div data-tauri-drag-region class="flex min-w-0 items-center">
        <div data-tauri-drag-region class="flex h-full shrink-0 items-center gap-2 p-3">
            <AxolotlLogo class="h-full w-auto shrink-0 pointer-events-none" />
            <span
                v-if="isBetaBuild"
                class="inline-flex shrink-0 rounded-full bg-blue-100 px-2 py-0.5 text-xs font-semibold leading-none text-blue-700"
            >
                {{ formatMessage(messages.betaBuild) }}
            </span>
        </div>
        <div class="flex shrink-0 items-center gap-1" data-tauri-drag-region-exclude>
            <button
                type="button"
                :aria-label="formatMessage(commonMessages.backButton)"
                :title="formatMessage(commonMessages.backButton)"
                class="cursor-pointer p-0 m-0 text-[var(--color-text-primary)] border-none bg-surface-4 rounded-full flex items-center justify-center w-6 h-6 hover:brightness-75 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand transition-all"
                @click="router.back()"
            >
                <LeftArrowIcon aria-hidden="true" />
            </button>
            <button
                type="button"
                :aria-label="formatMessage(messages.forward)"
                :title="formatMessage(messages.forward)"
                class="cursor-pointer p-0 m-0 text-[var(--color-text-primary)] border-none bg-surface-4 rounded-full flex items-center justify-center w-6 h-6 hover:brightness-75 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand transition-all"
                @click="router.forward()"
            >
                <RightArrowIcon aria-hidden="true" />
            </button>
        </div>
        <Breadcrumbs class="flex-1 pt-[2px] pr-3" />
        <section data-tauri-drag-region class="ml-auto flex min-w-0 items-center">
            <slot />
        </section>
    </div>
</template>
