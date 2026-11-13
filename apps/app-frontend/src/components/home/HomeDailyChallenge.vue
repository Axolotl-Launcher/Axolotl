<script setup lang="ts">
import { SparklesIcon, UpdatedIcon } from '@modrinth/assets'
import { Button, defineMessages, useVIntl } from '@modrinth/ui'
import { computed, ref } from 'vue'

import { type ChallengeDifficulty, dailyChallenges } from '@/data/daily-challenges'

import { stableGreetingIndex, toDateKey } from './home-utils'
import HomeWidgetFrame from './HomeWidgetFrame.vue'

const { formatMessage, locale } = useVIntl()

const messages = defineMessages({
    dailyChallenge: { id: 'app.home.challenge.title', defaultMessage: 'Daily challenge' },
    shuffle: { id: 'app.home.challenge.shuffle', defaultMessage: 'Try another' },
    easy: { id: 'app.home.challenge.easy', defaultMessage: 'Easy' },
    medium: { id: 'app.home.challenge.medium', defaultMessage: 'Medium' },
    hard: { id: 'app.home.challenge.hard', defaultMessage: 'Hard' },
    emptyTitle: { id: 'app.home.challenge.empty-title', defaultMessage: 'No challenge today' },
    emptyHint: {
        id: 'app.home.challenge.empty-hint',
        defaultMessage: 'Pick your own goal for this session.',
    },
})

const difficultyMessages = {
    easy: messages.easy,
    medium: messages.medium,
    hard: messages.hard,
} as const

const dailyIndex = stableGreetingIndex(
    `daily-challenge:${toDateKey(new Date())}`,
    dailyChallenges.length,
)
const challengeIndex = ref(dailyIndex)

/** Undefined when the challenge list is empty; the card then shows its empty state. */
const challenge = computed(() => dailyChallenges[challengeIndex.value] ?? null)
const challengeText = computed(() => {
    if (!challenge.value) return ''
    const lowerLocale = locale.value.toLowerCase()
    if (lowerLocale == 'zh-tw') {
        return challenge.value.text['zh-TW']
    } else {
        return lowerLocale.startsWith('zh')
            ? challenge.value.text['zh-CN']
            : challenge.value.text['en-US']
    }
})

const difficultyPillClass: Record<ChallengeDifficulty, string> = {
    easy: 'bg-highlight-green text-brand-green',
    medium: 'bg-highlight-orange text-brand-orange',
    hard: 'bg-highlight-red text-brand-red',
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
    <HomeWidgetFrame
        class="home-sidebar-section home-surface"
        :title="formatMessage(messages.dailyChallenge)"
        :icon="SparklesIcon"
        :empty-text="challenge ? null : formatMessage(messages.emptyTitle)"
        :empty-hint="formatMessage(messages.emptyHint)"
        :empty-icon="SparklesIcon"
    >
        <template v-if="dailyChallenges.length > 1" #actions>
            <Button
                v-tooltip="formatMessage(messages.shuffle)"
                type="quiet"
                size="2xs"
                circular
                icon-only
                :aria-label="formatMessage(messages.shuffle)"
                @click="shuffleChallenge"
                ><UpdatedIcon />
            </Button>
        </template>
        <figure v-if="challenge" class="home-challenge m-0 flex min-w-0 flex-col gap-2.5">
            <blockquote
                class="m-0 text-sm font-medium leading-relaxed text-[var(--color-text-primary)]"
                aria-live="polite"
            >
                {{ challengeText }}
            </blockquote>
            <figcaption>
                <span
                    class="inline-flex items-center gap-1.5 rounded-full px-2 text-xs font-bold leading-5"
                    :class="difficultyPillClass[challenge.difficulty]"
                >
                    <span class="size-1.5 rounded-full bg-current" aria-hidden="true" />
                    {{ formatMessage(difficultyMessages[challenge.difficulty]) }}
                </span>
            </figcaption>
        </figure>
    </HomeWidgetFrame>
</template>

<style scoped>
/* The challenge reads as a quote: an accent rule marks it as today's prompt. */
.home-challenge {
    @apply pl-3;
    border-left: 3px solid var(--color-brand);
}
</style>
