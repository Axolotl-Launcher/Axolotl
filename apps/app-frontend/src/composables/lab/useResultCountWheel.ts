import { defineMessages, useVIntl } from '@modrinth/ui'
import { computed } from 'vue'

import { isResultValue, nextResultCount } from '@/lab/recipe-generator/count-display'
import type { RecipeSlot, SlotValue } from '@/lab/recipe-generator/types'

const messages = defineMessages({
    scrollToAdjust: {
        id: 'app.lab.recipe-generator.scroll-to-adjust',
        defaultMessage: 'Scroll to adjust quantity',
    },
})

export function useResultCountWheel(options: {
    getSlot: () => RecipeSlot | null
    getValue: () => SlotValue | undefined
    getCount: () => number
    setCount: (count: number) => void
    getMaximum?: () => number
}) {
    const { formatMessage } = useVIntl()
    const canAdjust = computed(() =>
        Boolean(options.getSlot() && isResultValue(options.getValue())),
    )
    const hint = computed(() =>
        canAdjust.value ? formatMessage(messages.scrollToAdjust) : undefined,
    )

    function onWheel(slot: RecipeSlot, event: WheelEvent) {
        if (slot !== options.getSlot()) return
        const value = options.getValue()
        if (!isResultValue(value)) return
        if (event.deltaY === 0) return
        event.preventDefault()
        event.stopPropagation()
        const current = options.getCount()
        const next = nextResultCount(current, event.deltaY, options.getMaximum?.())
        if (next !== current) options.setCount(next)
    }

    return { canAdjust, hint, onWheel }
}
