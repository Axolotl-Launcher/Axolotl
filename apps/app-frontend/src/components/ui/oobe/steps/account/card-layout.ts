export type CardSlot = 'front' | 'left' | 'right'

/** Which account's card sits in each slot; an empty slot shows a placeholder card. */
export type CardLayout = Partial<Record<CardSlot, string>>

/** How a card reached its slot: from behind to the front, the other way, or new. */
export type CardMove = 'rise' | 'sink' | 'enter'

export const CARD_SLOTS: readonly CardSlot[] = ['front', 'left', 'right']
const BEHIND: readonly CardSlot[] = ['left', 'right']

export function slotOf(layout: CardLayout, id: string): CardSlot | undefined {
    return CARD_SLOTS.find((slot) => layout[slot] === id)
}

/**
 * Lays out the account cards: the selected account in front and, once there are two or
 * more accounts, up to two others behind it. Cards keep their slots between updates, and
 * the front card takes the slot of the card that replaces it, so picking an account reads
 * as the two cards trading places.
 */
export function layoutCards(
    previous: CardLayout,
    ids: readonly string[],
    selected: string | undefined,
): CardLayout {
    const front = selected && ids.includes(selected) ? selected : undefined
    const next: CardLayout = front ? { front } : {}
    if (ids.length < 2) return next

    const behind = ids.filter((id) => id !== front)
    for (const slot of BEHIND) {
        const id = previous[slot]
        if (id && behind.includes(id)) next[slot] = id
    }
    const replaced = previous.front
    if (replaced && replaced !== front && behind.includes(replaced)) {
        const vacated = BEHIND.find((slot) => previous[slot] === front)
        next[vacated ?? BEHIND.find((slot) => !next[slot]) ?? 'left'] = replaced
    }
    for (const slot of BEHIND) {
        if (next[slot]) continue
        const id = behind.find((candidate) => !slotOf(next, candidate))
        if (id) next[slot] = id
    }
    return next
}

/** How the card of account `id` moved between two layouts, if it moved at all. */
export function moveOf(previous: CardLayout, next: CardLayout, id: string): CardMove | undefined {
    const from = slotOf(previous, id)
    const to = slotOf(next, id)
    if (!to || from === to) return undefined
    if (!from) return 'enter'
    if (to === 'front') return 'rise'
    if (from === 'front') return 'sink'
    return undefined
}
