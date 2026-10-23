<script setup lang="ts">
import SparklesIcon from '@modrinth/assets/icons/sparkles.svg?component'
import UpdatedIcon from '@modrinth/assets/icons/updated.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, ref } from 'vue'

import { type ChallengeDifficulty, dailyChallenges } from '../data/daily-challenges'
import { stableGreetingIndex, toDateKey } from '../home-utils'
import { homeMessages } from '../messages'

const { formatMessage, locale } = useVIntl()
const messages = homeMessages

const difficultyMessages = {
    easy: messages.challengeEasy,
    medium: messages.challengeMedium,
    hard: messages.challengeHard,
} as const

const dailyIndex = stableGreetingIndex(
    `daily-challenge:${toDateKey(new Date())}`,
    dailyChallenges.length,
)
const challengeIndex = ref(dailyIndex)

const challenge = computed(() => dailyChallenges[challengeIndex.value])
const challengeText = computed(() => {
    const lowerLocale = locale.value.toLowerCase()
    if (lowerLocale === 'zh-tw') return challenge.value.text['zh-TW']
    return lowerLocale.startsWith('zh')
        ? challenge.value.text['zh-CN']
        : challenge.value.text['en-US']
})

const difficultyDotClass: Record<ChallengeDifficulty, string> = {
    easy: 'bg-brand-green',
    medium: 'bg-orange',
    hard: 'bg-red',
}

function shuffleChallenge() {
    if (dailyChallenges.length < 2) return
    let next = challengeIndex.value
    while (next === challengeIndex.value) {
        next = Math.floor(Math.random() * dailyChallenges.length)
    }
    challengeIndex.value = next
}
</script>

<template>
    <section
        class="flex min-w-0 flex-col gap-3 border-0 border-b-[1px] border-solid border-[--brand-gradient-border] p-4"
    >
        <div class="flex items-center gap-2">
            <SparklesIcon
                class="size-4 shrink-0 text-[var(--color-text-tertiary)]"
                aria-hidden="true"
            />
            <h2 class="m-0 truncate text-lg">{{ formatMessage(messages.challengeTitle) }}</h2>
            <Button
                v-tooltip="formatMessage(messages.challengeShuffle)"
                class="ml-auto"
                type="quiet"
                size="2xs"
                circular
                icon-only
                @click="shuffleChallenge"
                ><UpdatedIcon />
            </Button>
        </div>
        <p class="m-0 text-sm leading-relaxed text-[var(--color-text-default)]">
            {{ challengeText }}
        </p>
        <div class="flex items-center gap-1.5 text-xs text-[var(--color-text-tertiary)]">
            <span
                class="size-2 rounded-full"
                :class="difficultyDotClass[challenge.difficulty]"
                aria-hidden="true"
            />
            {{ formatMessage(difficultyMessages[challenge.difficulty]) }}
        </div>
    </section>
</template>
