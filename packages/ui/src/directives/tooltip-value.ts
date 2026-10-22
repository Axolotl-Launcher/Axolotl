import type { Placement } from '@floating-ui/vue'

/**
 * The value shapes `v-tooltip` accepts, and how they coerce.
 *
 * Kept apart from the directive itself so the coercion is testable: the
 * directive imports its stylesheet, which `node --test` cannot load, whereas
 * this module has no side effects at all.
 */

export type TooltipOptions = {
    content?: string | null
    html?: boolean
    placement?: Placement
    popperClass?: string
    /** Re-evaluate actual overflow after resize, text changes and font loading. */
    onlyWhenTruncated?: boolean
    /** The text fragment may be inside a larger hover/focus target. */
    overflowTarget?: HTMLElement | null
    /** Only `['hover']` appears in this repo; focus is always honoured. */
    triggers?: string[]
}

export type TooltipValue = string | TooltipOptions | null | undefined

/**
 * Resolves a binding value into what should be shown, or `null` for "no
 * tooltip".
 *
 * The suppression half is load-bearing: roughly thirty call sites pass a falsy
 * value to suppress conditionally. An empty string must mean *no tooltip*
 * rather than an empty bubble.
 */
export function resolveTooltipContent(
    value: TooltipValue,
): { text: string; options: TooltipOptions } | null {
    if (value === null || value === undefined || value === '') return null
    if (typeof value === 'string') return { text: value, options: {} }

    const text = value.content
    if (text === null || text === undefined || text === '') return null

    return { text, options: value }
}
