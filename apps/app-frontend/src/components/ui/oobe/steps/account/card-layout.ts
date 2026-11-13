export type CardSlot = 'front' | 'left' | 'right' | 'hidden'

/** Which account's card sits in each slot; an empty slot shows a placeholder card. */
export type CardLayout = Partial<Record<CardSlot, string>>

/**
 * How a card reached its slot: from behind to the front (`rise`) and back (`sink`), drawn
 * out of the hidden card's place (`draw`) and tucked into it (`stow`), from one place
 * behind to another (`shift`), or new (`enter`).
 */
export type CardMove = 'rise' | 'sink' | 'draw' | 'stow' | 'shift' | 'enter'

export const CARD_SLOTS: readonly CardSlot[] = ['front', 'left', 'right', 'hidden']
const BEHIND: readonly CardSlot[] = ['left', 'right']

/** From this many accounts a card hides behind the stack for those that do not fit on it. */
export const HIDDEN_CARD_FROM = 4

export function slotOf(layout: CardLayout, id: string): CardSlot | undefined {
    return CARD_SLOTS.find((slot) => layout[slot] === id)
}

/**
 * Lays out the account cards: the selected account in front and, once there are two or
 * more accounts, up to two others behind it, plus a hidden card from four. Cards keep their
 * slots between updates, and the front card takes the slot of the card that replaces it,
 * so picking an account reads as the two cards trading places.
 */
export function layoutCards(
    previous: CardLayout,
    ids: readonly string[],
    selected: string | undefined,
): CardLayout {
    const front = selected && ids.includes(selected) ? selected : undefined
    const next: CardLayout = front ? { front } : {}
    if (ids.length < 2) return next

    const deck = ids.length >= HIDDEN_CARD_FROM
    const slots = deck ? [...BEHIND, 'hidden' as const] : BEHIND
    const behind = ids.filter((id) => id !== front)
    for (const slot of slots) {
        const id = previous[slot]
        if (id && behind.includes(id)) next[slot] = id
    }
    const replaced = previous.front
    if (replaced && replaced !== front && behind.includes(replaced)) {
        const vacated = slots.find((slot) => previous[slot] === front)
        next[vacated ?? BEHIND.find((slot) => !next[slot]) ?? (deck ? 'hidden' : 'left')] = replaced
    }
    for (const slot of slots) {
        if (next[slot]) continue
        const id = behind.find((candidate) => !slotOf(next, candidate))
        if (id) next[slot] = id
    }
    return next
}

/**
 * The layout as it stood, but with a picked account that has no visible card put on the
 * hidden card. That card's head is covered by the front card, so the swap cannot be seen,
 * and the card is drawn out as if it had carried that head all along.
 */
export function withPickOnHiddenCard(
    previous: CardLayout,
    ids: readonly string[],
    selected: string | undefined,
): CardLayout {
    if (!selected || !ids.includes(selected) || ids.length < HIDDEN_CARD_FROM) return previous
    // Nothing is on screen yet, so the first layout just appears.
    if (Object.keys(previous).length === 0) return previous
    const slot = slotOf(previous, selected)
    if (slot && slot !== 'hidden') return previous
    return { ...previous, hidden: selected }
}

/** How the card of account `id` moved between two layouts, if it moved at all. */
export function moveOf(previous: CardLayout, next: CardLayout, id: string): CardMove | undefined {
    const from = slotOf(previous, id)
    const to = slotOf(next, id)
    if (!to || from === to) return undefined
    if (!from) return 'enter'
    if (to === 'front') return from === 'hidden' ? 'draw' : 'rise'
    if (from === 'front') return to === 'hidden' ? 'stow' : 'sink'
    return 'shift'
}
