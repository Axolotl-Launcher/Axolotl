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
    difficulty: { id: 'app.home.challenge.difficulty', defaultMessage: 'Difficulty: {level}' },
    number: { id: 'app.home.challenge.number', defaultMessage: 'No. {number}' },
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

/** How many of the three ticks a difficulty fills. */
const difficultyLevel: Record<ChallengeDifficulty, number> = { easy: 1, medium: 2, hard: 3 }

/** The challenge's number in the list, beside the title: "No. 007". */
const challengeNumber = computed(() =>
    challenge.value
        ? formatMessage(messages.number, {
              number: String(challengeIndex.value + 1).padStart(3, '0'),
          })
        : null,
)

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
        class="home-challenge-card home-sidebar-section home-surface"
        :title="formatMessage(messages.dailyChallenge)"
        :icon="SparklesIcon"
        header="eyebrow"
        :badge="challengeNumber"
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
        <Transition name="home-challenge-flip" mode="out-in">
            <figure
                v-if="challenge"
                :key="challengeIndex"
                class="home-challenge m-0 flex min-w-0 flex-col gap-3"
            >
                <blockquote class="home-challenge-text" aria-live="polite">
                    {{ challengeText }}
                </blockquote>
                <figcaption
                    class="home-challenge-difficulty"
                    :data-difficulty="challenge.difficulty"
                    :aria-label="
                        formatMessage(messages.difficulty, {
                            level: formatMessage(difficultyMessages[challenge.difficulty]),
                        })
                    "
                >
                    <span class="home-challenge-ticks" aria-hidden="true">
                        <span
                            v-for="tick in 3"
                            :key="tick"
                            :class="{ 'is-on': tick <= difficultyLevel[challenge.difficulty] }"
                        />
                    </span>
                    <span aria-hidden="true">{{
                        formatMessage(difficultyMessages[challenge.difficulty])
                    }}</span>
                </figcaption>
            </figure>
        </Transition>
    </HomeWidgetFrame>
</template>

<style scoped>
/*
 * Today's challenge reads as a task card: its number is a small badge beside
 * the label, the task is the headline, and the difficulty is a row of three
 * ticks filled in its colour.
 */
.home-challenge-card {
    @apply relative;
}

.home-challenge {
    @apply relative;
}

.home-challenge-card :deep(.home-widget-frame-badge) {
    @apply tabular-nums;
}

.home-challenge-text {
    @apply relative m-0 text-[0.9375rem] font-semibold leading-relaxed text-[var(--color-text-primary)];
}

.home-challenge-difficulty {
    @apply flex items-center gap-2 text-xs font-bold;
}

.home-challenge-ticks {
    @apply flex items-center gap-[3px];
}

.home-challenge-ticks > span {
    @apply block h-2 w-4 rounded-full;
    background: var(--home-widget-tint-strong);
}

/* The word takes the darker shade of its colour; the ticks keep the plain one. */
.home-challenge-difficulty[data-difficulty='easy'] {
    --home-challenge-tick: var(--color-green);
    color: var(--home-status-green-text);
}

.home-challenge-difficulty[data-difficulty='medium'] {
    --home-challenge-tick: var(--color-orange);
    color: var(--home-status-orange-text);
}

.home-challenge-difficulty[data-difficulty='hard'] {
    --home-challenge-tick: var(--color-red);
    color: var(--home-status-red-text);
}

.home-challenge-ticks > span.is-on {
    background: var(--home-challenge-tick, currentColor);
}

/* A new challenge turns over like a card. */
.home-challenge-flip-enter-active,
.home-challenge-flip-leave-active {
    transition:
        transform 160ms ease,
        opacity 160ms ease;
    transform-origin: 50% 50%;
}

.home-challenge-flip-leave-to {
    opacity: 0;
    transform: perspective(600px) rotateX(70deg);
}

.home-challenge-flip-enter-from {
    opacity: 0;
    transform: perspective(600px) rotateX(-70deg);
}

@media (prefers-reduced-motion: reduce) {
    .home-challenge-flip-enter-active,
    .home-challenge-flip-leave-active {
        transition: none;
    }

    .home-challenge-flip-leave-to,
    .home-challenge-flip-enter-from {
        transform: none;
    }
}
</style>
