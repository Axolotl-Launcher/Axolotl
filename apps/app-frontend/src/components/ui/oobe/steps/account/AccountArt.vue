<script setup lang="ts">
import { computed, ref, watch } from 'vue'

import { steveHead, useOobeAccounts } from './accounts'
import {
    CARD_SLOTS,
    type CardLayout,
    type CardMove,
    type CardSlot,
    layoutCards,
    moveOf,
    slotOf,
    withPickOnHiddenCard,
} from './card-layout'
import {
    CARD_MOTION_MS,
    cardKeyframes,
    type CardPose,
    LAYER,
    OPACITY,
    POSES,
    restingPose,
    SIDE,
} from './card-motion'

/**
 * The selected account's card in front, with Steve's head while there is none. With two
 * or more accounts the cards behind carry the others, and picking one of them slides its
 * card up to the front while the old front card sinks into its place. From four accounts a
 * faint card hides behind the stack; an account without a visible card is drawn out of it.
 *
 * Moves can be interrupted: a pick made while cards are still moving sends every card whose
 * place changed on from wherever it is, and leaves the others to finish their way.
 */
const { accounts, defaultAccount, headOf } = useOobeAccounts()

const CARD_MOTION = 'oobe-account-card-motion'

const layout = ref<CardLayout>({})
/** Each account card's element, so a move can start from where the card is on screen. */
const elements = new Map<string, HTMLElement>()
/** Moves decided before the cards' new slots are rendered, played right after. */
let queued: { id: string; move: CardMove; from?: CardSlot; start?: CardPose }[] = []
/** Cards a new move caught mid-flight; their move eases on instead of starting from rest. */
const movingIds = new Set<string>()

function bindCard(id: string | undefined, element: unknown) {
    if (!id) return
    if (element instanceof HTMLElement) elements.set(id, element)
    else elements.delete(id)
}

/** Where a card is right now, part way through a move or at rest. */
function currentPose(element: HTMLElement): CardPose {
    const style = getComputedStyle(element)
    return {
        transform: style.transform === 'none' ? POSES.front : style.transform,
        opacity: Number(style.opacity),
        layer: Number(style.zIndex) || 0,
    }
}

function isMoving(element: HTMLElement) {
    return element.getAnimations().some((animation) => animation.id === CARD_MOTION)
}

watch(
    () => [accounts.value.map((account) => account.account_id), defaultAccount.value] as const,
    ([ids, selected]) => {
        const shown = layout.value
        const before = withPickOnHiddenCard(shown, ids, selected)
        const next = layoutCards(before, ids, selected)
        // Read before the new slots render, so each move starts where its card is now.
        const now = new Map<string, { pose: CardPose; moving: boolean }>()
        for (const slot of CARD_SLOTS) {
            const id = shown[slot]
            const element = id ? elements.get(id) : undefined
            if (id && element)
                now.set(id, { pose: currentPose(element), moving: isMoving(element) })
        }
        // The pick took over the hidden card out of sight, so it starts where that card is.
        if (shown.hidden && before.hidden !== shown.hidden) {
            const hidden = now.get(shown.hidden)
            if (hidden && before.hidden) now.set(before.hidden, hidden)
        }
        queued = []
        for (const slot of CARD_SLOTS) {
            const id = next[slot]
            const move = id ? moveOf(before, next, id) : undefined
            if (id && move) {
                const from = slotOf(before, id)
                const start = now.get(id)
                queued.push({ id, move, from, start: start?.pose })
                if (start?.moving) movingIds.add(id)
                else movingIds.delete(id)
            }
        }
        layout.value = next
    },
    { immediate: true },
)

watch(
    layout,
    () => {
        const reduced = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches
        for (const { id, move, from, start } of queued) {
            const element = elements.get(id)
            const to = slotOf(layout.value, id)
            if (!element || !to) continue
            element.getAnimations().forEach((animation) => {
                if (animation.id === CARD_MOTION) animation.cancel()
            })
            if (reduced) continue
            // A card coming forward swings toward the side it came from; one going back toward its new side.
            const side = SIDE[move === 'rise' || move === 'draw' ? (from ?? to) : to]
            const keyframes = cardKeyframes(
                start ?? restingPose(from ?? to),
                to,
                move,
                side,
                movingIds.has(id),
            )
            element.animate(keyframes, { id: CARD_MOTION, duration: CARD_MOTION_MS[move] })
        }
        queued = []
    },
    { flush: 'post' },
)

const cards = computed(() =>
    // The hidden card exists only while some account has no visible card.
    CARD_SLOTS.filter((slot) => slot !== 'hidden' || layout.value.hidden).map((slot) => {
        const id = layout.value[slot]
        return {
            key: id ?? `empty-${slot}`,
            id,
            slot,
            head: id
                ? headOf(accounts.value.find((account) => account.account_id === id))
                : slot === 'front'
                  ? steveHead
                  : undefined,
            // At rest in its slot; a move animates from wherever the card was to here.
            style: {
                transform: POSES[slot],
                opacity: OPACITY[slot],
                zIndex: LAYER[slot],
            },
        }
    }),
)
</script>

<template>
    <div class="flex items-center justify-center p-10">
        <svg
            class="h-full max-h-[26rem] w-full"
            viewBox="0 0 400 440"
            fill="none"
            preserveAspectRatio="xMidYMid meet"
        >
            <ellipse cx="200" cy="384" rx="120" ry="12" fill="var(--surface-3)" opacity="0.7" />

            <g class="oobe-account-stack">
                <!-- HTML, so the cards can trade stacking order halfway through a swap. -->
                <foreignObject x="0" y="0" width="400" height="440">
                    <div class="oobe-account-stage">
                        <div
                            v-for="card in cards"
                            :key="card.key"
                            :ref="(element) => bindCard(card.id, element)"
                            class="oobe-account-card"
                            :class="`slot-${card.slot}`"
                            :style="card.style"
                            data-testid="oobe-account-card"
                            :data-slot="card.slot"
                            :data-account="card.id"
                        >
                            <svg class="block h-full w-full" viewBox="0 0 192 252" fill="none">
                                <rect width="192" height="252" rx="26" fill="var(--surface-3)" />
                                <g class="front-only">
                                    <rect
                                        x="1"
                                        y="1"
                                        width="190"
                                        height="250"
                                        rx="25"
                                        fill="var(--surface-4)"
                                        stroke="var(--color-brand)"
                                        stroke-opacity="0.5"
                                        stroke-width="2"
                                    />
                                </g>
                                <image
                                    v-if="card.head"
                                    class="oobe-account-head"
                                    data-testid="oobe-account-head"
                                    :href="card.head"
                                    x="48"
                                    y="36"
                                    width="96"
                                    height="96"
                                    preserveAspectRatio="none"
                                />
                                <rect
                                    v-else
                                    x="48"
                                    y="36"
                                    width="96"
                                    height="96"
                                    rx="14"
                                    fill="var(--surface-4)"
                                />
                                <rect
                                    x="36"
                                    y="156"
                                    width="120"
                                    height="13"
                                    rx="6.5"
                                    fill="var(--surface-5)"
                                />
                                <rect
                                    x="56"
                                    y="180"
                                    width="80"
                                    height="10"
                                    rx="5"
                                    fill="var(--surface-5)"
                                    opacity="0.6"
                                />
                                <g class="front-only">
                                    <rect
                                        x="36"
                                        y="208"
                                        width="120"
                                        height="24"
                                        rx="12"
                                        fill="var(--color-brand)"
                                        opacity="0.9"
                                    />
                                </g>
                            </svg>
                        </div>
                    </div>
                </foreignObject>

                <!-- Keyed by the front account, so it pops again on the card that took its place. -->
                <g :key="layout.front ?? 'none'" class="oobe-account-badge">
                    <circle cx="290" cy="110" r="28" fill="var(--color-brand)" />
                    <path
                        d="M277 111 l9 9 l17 -19"
                        stroke="var(--color-accent-contrast)"
                        stroke-width="5"
                        stroke-linecap="round"
                        stroke-linejoin="round"
                    />
                </g>
            </g>
        </svg>
    </div>
</template>

<style scoped lang="scss">
.oobe-account-stack {
    animation: oobe-account-float 6s ease-in-out infinite;
}

.oobe-account-stage {
    position: relative;
    width: 400px;
    height: 440px;
}

// Every card is drawn at the front card's size and place, then posed into its slot.
.oobe-account-card {
    position: absolute;
    left: 104px;
    top: 96px;
    width: 192px;
    height: 252px;
    transform-origin: 50% 50%;
}

// The front card's outline and button; a transition, so a reversed pick fades them back smoothly.
.front-only {
    transition: opacity 360ms ease 180ms;
}

.slot-left,
.slot-right,
.slot-hidden {
    .front-only {
        opacity: 0;
    }
}

.oobe-account-head {
    image-rendering: pixelated;
}

.oobe-account-badge {
    transform-origin: 290px 110px;
    animation: oobe-account-pop 700ms 300ms cubic-bezier(0.34, 1.56, 0.64, 1) both;
}

@keyframes oobe-account-float {
    0%,
    100% {
        transform: translateY(0);
    }
    50% {
        transform: translateY(-6px);
    }
}

@keyframes oobe-account-pop {
    from {
        opacity: 0;
        scale: 0.4;
    }
    to {
        opacity: 1;
        scale: 1;
    }
}

@media (prefers-reduced-motion: reduce) {
    .oobe-account-stack,
    .oobe-account-badge {
        animation: none;
    }

    .front-only {
        transition: none;
    }
}
</style>
