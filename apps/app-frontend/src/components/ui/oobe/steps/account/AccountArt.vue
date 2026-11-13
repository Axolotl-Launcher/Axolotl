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

/**
 * The selected account's card in front, with Steve's head while there is none. With two
 * or more accounts the cards behind carry the others, and picking one of them slides its
 * card up to the front while the old front card sinks into its place. From four accounts a
 * faint card hides behind the stack; an account without a visible card is drawn out of it.
 */
const { accounts, defaultAccount, headOf } = useOobeAccounts()

/** Each slot's pose, relative to the front card; every pose uses the same functions so they interpolate. */
const POSES: Record<CardSlot, string> = {
    front: 'translate(0px, 0px) rotate(0deg) scale(1)',
    // Fanned out far enough that most of each head shows beside the front card.
    left: 'translate(-100px, 8px) rotate(-10deg) scale(0.88)',
    right: 'translate(100px, 8px) rotate(10deg) scale(0.88)',
    // Only its top edge clears the front card; its head stays covered.
    hidden: 'translate(0px, -46px) rotate(0deg) scale(0.8)',
}
/** The side a card swings toward on its way; the hidden card is drawn out to the right. */
const SIDE: Record<CardSlot, number> = { front: 0, left: -1, right: 1, hidden: 1 }

const layout = ref<CardLayout>({})
/** The last move of each account's card; `run` gives the card a new element so it plays again. */
const moves = ref(new Map<string, { move: CardMove; from?: CardSlot; run: number }>())
let runs = 0

watch(
    () => [accounts.value.map((account) => account.account_id), defaultAccount.value] as const,
    ([ids, selected]) => {
        const before = withPickOnHiddenCard(layout.value, ids, selected)
        const next = layoutCards(before, ids, selected)
        const updated = new Map(moves.value)
        for (const slot of CARD_SLOTS) {
            const id = next[slot]
            const move = id ? moveOf(before, next, id) : undefined
            if (id && move) updated.set(id, { move, from: slotOf(before, id), run: ++runs })
        }
        moves.value = updated
        layout.value = next
    },
    { immediate: true },
)

const cards = computed(() =>
    // The hidden card exists only while some account has no visible card.
    CARD_SLOTS.filter((slot) => slot !== 'hidden' || layout.value.hidden).map((slot) => {
        const id = layout.value[slot]
        const motion = id ? moves.value.get(id) : undefined
        const from = motion?.from ?? slot
        const leaving = motion?.move === 'rise' || motion?.move === 'draw'
        return {
            key: id ? `${id}#${motion?.run ?? 0}` : `empty-${slot}`,
            slot,
            head: id
                ? headOf(accounts.value.find((account) => account.account_id === id))
                : slot === 'front'
                  ? steveHead
                  : undefined,
            move: motion?.move,
            style: {
                '--oobe-card-pose': POSES[slot],
                '--oobe-card-from': POSES[from],
                // A card coming forward swings toward the side it came from; one going back toward its new side.
                '--oobe-card-side': SIDE[leaving ? from : slot],
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
                            class="oobe-account-card"
                            :class="[`slot-${card.slot}`, card.move && `move-${card.move}`]"
                            :style="card.style"
                            data-testid="oobe-account-card"
                            :data-slot="card.slot"
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
                                    :key="card.head"
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
    transform: var(--oobe-card-pose);
    transform-origin: 50% 50%;
}

.slot-front {
    z-index: 3;
}

.slot-left,
.slot-right {
    z-index: 1;
    opacity: 0.7;
}

// Above the side cards, which never reach the strip of it that shows, so it can come out over them.
.slot-hidden {
    z-index: 2;
    opacity: 0.4;
}

.slot-left,
.slot-right,
.slot-hidden {
    .front-only {
        opacity: 0;
    }
}

.move-rise {
    animation: oobe-account-rise 720ms cubic-bezier(0.45, 0, 0.2, 1) both;

    .front-only {
        animation: oobe-account-front-in 720ms ease-in-out both;
    }
}

.move-sink {
    animation: oobe-account-sink 720ms cubic-bezier(0.45, 0, 0.2, 1) both;

    .front-only {
        animation: oobe-account-front-out 720ms ease-in-out both;
    }
}

.move-draw {
    animation: oobe-account-draw 820ms cubic-bezier(0.45, 0, 0.2, 1) both;

    .front-only {
        animation: oobe-account-front-in 820ms ease-in-out both;
    }
}

.move-stow {
    animation: oobe-account-stow 820ms cubic-bezier(0.45, 0, 0.2, 1) both;

    .front-only {
        animation: oobe-account-front-out 820ms ease-in-out both;
    }
}

.move-enter {
    animation: oobe-account-enter 420ms cubic-bezier(0.22, 1, 0.36, 1) both;
}

.oobe-account-head {
    image-rendering: pixelated;
    // A head that finishes rendering fades in over Steve's.
    animation: oobe-account-head-in 320ms ease-out both;
}

// A moving card is a new element but carries its head from the start.
.move-rise,
.move-sink,
.move-draw,
.move-stow {
    .oobe-account-head {
        animation: none;
    }
}

.oobe-account-badge {
    transform-origin: 290px 110px;
    animation: oobe-account-pop 700ms 300ms cubic-bezier(0.34, 1.56, 0.64, 1) both;
}

// Up and out from behind the front card, in front of it from halfway, then into place.
@keyframes oobe-account-rise {
    0% {
        transform: var(--oobe-card-from);
        opacity: 0.7;
        z-index: 1;
    }
    50% {
        transform: translate(calc(var(--oobe-card-side) * 80px), -84px)
            rotate(calc(var(--oobe-card-side) * 5deg)) scale(0.96);
        z-index: 1;
    }
    51% {
        z-index: 3;
    }
    100% {
        transform: var(--oobe-card-pose);
        opacity: 1;
        z-index: 3;
    }
}

// Down and aside, behind the rising card from halfway, then into the slot it left.
@keyframes oobe-account-sink {
    0% {
        transform: var(--oobe-card-from);
        opacity: 1;
        z-index: 3;
    }
    50% {
        transform: translate(calc(var(--oobe-card-side) * 40px), 34px)
            rotate(calc(var(--oobe-card-side) * 4deg)) scale(0.9);
        z-index: 3;
    }
    51% {
        z-index: 1;
    }
    100% {
        transform: var(--oobe-card-pose);
        opacity: 0.7;
        z-index: 1;
    }
}

// Pulled up and out of the deck to the side, in front of the old card from halfway, then into place.
@keyframes oobe-account-draw {
    0% {
        transform: var(--oobe-card-from);
        opacity: 0.4;
        z-index: 2;
    }
    50% {
        transform: translate(calc(var(--oobe-card-side) * 70px), -100px)
            rotate(calc(var(--oobe-card-side) * 6deg)) scale(0.88);
        opacity: 1;
        z-index: 2;
    }
    51% {
        z-index: 3;
    }
    100% {
        transform: var(--oobe-card-pose);
        opacity: 1;
        z-index: 3;
    }
}

// Down and away from the drawn card, behind it from halfway, then tucked into the deck.
@keyframes oobe-account-stow {
    0% {
        transform: var(--oobe-card-from);
        opacity: 1;
        z-index: 3;
    }
    50% {
        transform: translate(calc(var(--oobe-card-side) * -28px), 26px)
            rotate(calc(var(--oobe-card-side) * -3deg)) scale(0.9);
        opacity: 0.9;
        z-index: 3;
    }
    51% {
        z-index: 2;
    }
    100% {
        transform: var(--oobe-card-pose);
        opacity: 0.4;
        z-index: 2;
    }
}

@keyframes oobe-account-front-in {
    0%,
    40% {
        opacity: 0;
    }
    100% {
        opacity: 1;
    }
}

@keyframes oobe-account-front-out {
    0% {
        opacity: 1;
    }
    60%,
    100% {
        opacity: 0;
    }
}

@keyframes oobe-account-enter {
    from {
        opacity: 0;
        transform: translateY(16px) var(--oobe-card-pose);
    }
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

@keyframes oobe-account-head-in {
    from {
        opacity: 0;
    }
    to {
        opacity: 1;
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
    .oobe-account-head,
    .oobe-account-badge,
    .move-rise,
    .move-rise .front-only,
    .move-sink,
    .move-sink .front-only,
    .move-draw,
    .move-draw .front-only,
    .move-stow,
    .move-stow .front-only,
    .move-enter {
        animation: none;
    }
}
</style>
