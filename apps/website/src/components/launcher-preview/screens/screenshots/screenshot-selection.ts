import { ref } from 'vue'

export type SelectionModifiers = { shiftKey?: boolean; ctrlKey?: boolean; metaKey?: boolean }
export type SelectionRect = { left: number; top: number; width: number; height: number }
export type SelectionTarget = SelectionRect & { key: string }

/** Range order is the visible grouped list, including rows outside the virtualization window. */
export function createScreenshotSelection() {
    const selectedKeys = ref(new Set<string>())
    let anchor: string | undefined
    return {
        selectedKeys,
        markAnchor(key: string) {
            anchor = key
        },
        clear() {
            selectedKeys.value = new Set()
            anchor = undefined
        },
        reconcile(order: readonly string[]) {
            const visible = new Set(order)
            selectedKeys.value = new Set([...selectedKeys.value].filter((key) => visible.has(key)))
            if (anchor && !visible.has(anchor)) anchor = undefined
        },
        select(key: string, order: readonly string[], modifiers: SelectionModifiers = {}) {
            if (!order.includes(key)) return
            const next = new Set(selectedKeys.value)
            if (modifiers.shiftKey) {
                const start = anchor ? order.indexOf(anchor) : -1
                const end = order.indexOf(key)
                if (end < 0) return
                if (!modifiers.ctrlKey && !modifiers.metaKey) next.clear()
                for (const item of order.slice(
                    start < 0 ? end : Math.min(start, end),
                    start < 0 ? end + 1 : Math.max(start, end) + 1,
                ))
                    next.add(item)
                if (start < 0) anchor = key
            } else {
                if (next.has(key)) next.delete(key)
                else next.add(key)
                anchor = key
            }
            selectedKeys.value = next
        },
        toggleGroup(keys: readonly string[]) {
            const next = new Set(selectedKeys.value)
            const remove = keys.length > 0 && keys.every((key) => next.has(key))
            for (const key of keys) {
                if (remove) next.delete(key)
                else next.add(key)
            }
            selectedKeys.value = next
            anchor = keys[0]
        },
        box(keys: readonly string[], base: ReadonlySet<string>) {
            selectedKeys.value = new Set([...base, ...keys])
        },
    }
}

export function screenshotGridTargets(
    groups: readonly { keys: readonly string[]; gridTop: number; isOpen: boolean }[],
    columns: number,
    cardWidth: number,
    cardHeight: number,
    gap: number,
): SelectionTarget[] {
    return groups.flatMap((group) =>
        !group.isOpen
            ? []
            : group.keys.map((key, index) => ({
                  key,
                  left: (index % columns) * (cardWidth + gap),
                  top: group.gridTop + Math.floor(index / columns) * (cardHeight + gap),
                  width: cardWidth,
                  height: cardHeight,
              })),
    )
}

export function rectangleSelection(
    rect: SelectionRect,
    targets: readonly SelectionTarget[],
): string[] {
    return targets
        .filter(
            (target) =>
                target.left < rect.left + rect.width &&
                target.left + target.width > rect.left &&
                target.top < rect.top + rect.height &&
                target.top + target.height > rect.top,
        )
        .map((target) => target.key)
}

export function isSelectionModifier(event: SelectionModifiers) {
    return !!(event.shiftKey || event.ctrlKey || event.metaKey)
}
