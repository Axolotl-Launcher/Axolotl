<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'

import { steveHead, useOobeAccounts } from './accounts'
import {
    CARD_SLOTS,
    type CardLayout,
    type CardSlot,
    layoutCards,
    moveOf,
    slotOf,
    withPickOnHiddenCard,
} from './card-layout'
import {
    cardClock,
    type CardMotion,
    type CardPose,
    cardTransform,
    emergingMotion,
    enteringMotion,
    frontAmount,
    headCovered,
    layerOf,
    LEAVING_POSE,
    restingMotion,
    settle,
    SIDE,
    SLOT_POSES,
    startMove,
    stepMotion,
} from './card-physics'

/**
 * The selected account's card in front, with Steve's head while there is none. With two
 * or more accounts the cards behind carry the others, and picking one of them slides its
 * card up to the front while the old front card sinks into its place. From four accounts a
 * faint card hides behind the stack; an account without a visible card is drawn out of it.
 *
 * Every account card moves on springs, so a pick made while cards are still moving just
 * steers them from where they are, at the speed they have: they bend their path, or turn
 * back when a pick is taken back. Cards whose place did not change carry on as they were.
 */
const { accounts, selected, headOf } = useOobeAccounts()

const layout = ref<CardLayout>({})
/**
 * Accounts whose card lost its place while it could still be seen, as when a pick takes the
 * hidden place from a card still on its way into the deck. They go on into the deck, fading.
 */
const leaving = ref<string[]>([])
/** Each account card's element, written to directly every frame while cards move. */
const elements = new Map<string, HTMLElement>()
/**
 * Each account card's motion; its slot in `layout` is where it is heading, and a card
 * without one is leaving.
 */
const motions = new Map<string, CardMotion>()
let frame = 0
let lastFrame = 0

const reducedMotion = () => window.matchMedia?.('(prefers-reduced-motion: reduce)').matches

function draw(id: string) {
    const element = elements.get(id)
    const motion = motions.get(id)
    if (!element || !motion) return
    const pose: CardPose = motion.pose
    element.style.transform = cardTransform(pose)
    element.style.opacity = String(pose.o)
    element.style.zIndex = String(layerOf(pose))
    const front = String(frontAmount(pose))
    element.querySelectorAll<SVGElement>('.front-only').forEach((part) => {
        part.style.opacity = front
    })
}

function tick() {
    const now = cardClock.now()
    const dt = Math.min(Math.max(now - lastFrame, 0) / 1000, 1)
    lastFrame = now
    let moving = false
    for (const [id, motion] of motions) {
        const slot = slotOf(layout.value, id)
        if (!slot && reducedMotion()) {
            drop(id)
            continue
        }
        const target = slot ? SLOT_POSES[slot] : LEAVING_POSE
        const stepped =
            slot && reducedMotion() ? restingMotion(slot) : stepMotion(motion, target, dt, now)
        const rest = settle(stepped, target)
        if (rest && !slot) {
            // In the deck and faded out: the card is gone.
            drop(id)
            continue
        }
        motions.set(id, rest ?? stepped)
        if (!rest) moving = true
        draw(id)
    }
    frame = moving ? requestAnimationFrame(tick) : 0
}

function drop(id: string) {
    motions.delete(id)
    leaving.value = leaving.value.filter((other) => other !== id)
}

function wake() {
    if (frame) return
    lastFrame = cardClock.now()
    frame = requestAnimationFrame(tick)
}

onBeforeUnmount(() => cancelAnimationFrame(frame))

function bindCard(id: string | undefined, element: unknown) {
    if (!id) return
    if (element instanceof HTMLElement) {
        elements.set(id, element)
        // Placed before the first paint, so a new card never shows at the front card's place.
        draw(id)
    } else elements.delete(id)
}

/**
 * Gives a pick put on the hidden card a card to come out on. While the hidden card's head is
 * out of sight that is the hidden card itself, so it seems to have carried the pick's head
 * all along. While it can be seen, say still on its way into the deck, it keeps its head and
 * goes on, and the pick comes out of the deck on a card of its own, fading in. A pick whose
 * card is still in view on its way out just turns back.
 */
function takeHiddenCard(pick: string, hidden: string | undefined) {
    if (motions.has(pick)) return
    const card = hidden === undefined ? undefined : motions.get(hidden)
    const frontId = layout.value.front
    // With no account in front, the placeholder in front covers it.
    const front = frontId === undefined ? SLOT_POSES.front : motions.get(frontId)?.pose
    if (hidden !== undefined && card && front && headCovered(card.pose, front)) {
        motions.set(pick, card)
        motions.delete(hidden)
    } else motions.set(pick, emergingMotion())
}

watch(
    () => [accounts.value.map((account) => account.account_id), selected.value] as const,
    ([ids, pick]) => {
        const shown = layout.value
        const before = withPickOnHiddenCard(shown, ids, pick)
        if (pick && before.hidden === pick && shown.hidden !== pick)
            takeHiddenCard(pick, shown.hidden)
        const next = layoutCards(before, ids, pick)
        const now = cardClock.now()
        for (const slot of CARD_SLOTS) {
            const id = next[slot]
            const move = id ? moveOf(before, next, id) : undefined
            if (!id || !move) continue
            const from = slotOf(before, id)
            const current =
                motions.get(id) ??
                (move === 'enter' ? enteringMotion(slot) : restingMotion(from ?? slot))
            // A card coming forward swings toward the side it came from; one going back toward its new side.
            const side = SIDE[move === 'rise' || move === 'draw' ? (from ?? slot) : slot]
            motions.set(id, startMove(current, move, side, now))
        }
        // A card that lost its place goes on into the deck; one whose account is gone, at once.
        for (const id of [...motions.keys()]) {
            if (!slotOf(next, id) && (!ids.includes(id) || reducedMotion())) motions.delete(id)
        }
        leaving.value = [...motions.keys()].filter((id) => !slotOf(next, id))
        layout.value = next
        wake()
    },
    { immediate: true },
)

const headOfId = (id: string) => headOf(accounts.value.find((account) => account.account_id === id))

const cards = computed(() => [
    // The hidden card exists only while some account has no visible card.
    ...CARD_SLOTS.filter((slot) => slot !== 'hidden' || layout.value.hidden).map((slot) => {
        const id = layout.value[slot]
        const pose = SLOT_POSES[slot]
        return {
            key: id ?? `empty-${slot}`,
            id,
            slot: slot as CardSlot | 'leaving',
            head: id ? headOfId(id) : slot === 'front' ? steveHead : undefined,
            // Placeholders stay put; account cards are placed by their motion.
            style: id
                ? undefined
                : { transform: cardTransform(pose), opacity: pose.o, zIndex: layerOf(pose) },
        }
    }),
    ...leaving.value.map((id) => ({
        key: id,
        id,
        slot: 'leaving' as const,
        head: headOfId(id),
        style: undefined,
    })),
])
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

.slot-left,
.slot-right,
.slot-hidden,
.slot-leaving {
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
}
</style>
