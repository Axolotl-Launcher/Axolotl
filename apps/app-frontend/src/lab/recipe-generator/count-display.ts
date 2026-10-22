import type { SlotValue } from './types.ts'

export const RESULT_COUNT_MAX = 64

export type ResultCountValue = Extract<SlotValue, { kind: 'item' } | { kind: 'custom_item' }>

export function isResultValue(value: SlotValue | undefined): value is ResultCountValue {
    if (!value) return false
    return value.kind === 'item' || value.kind === 'custom_item'
}

export function nextResultCount(current: number, deltaY: number, maximum = RESULT_COUNT_MAX) {
    if (deltaY === 0) return current
    const next = current + (deltaY < 0 ? 1 : -1)
    return Math.min(maximum, Math.max(1, next))
}

export function countFontSize(iconSize: number) {
    return Math.max(8, Math.round(iconSize * 0.32))
}

export function countInset(iconSize: number) {
    return Math.max(2, Math.round(iconSize * 0.02))
}

export function countShadow(iconSize: number) {
    const offset = Math.max(1, Math.round(iconSize * 0.02))
    return `${offset}px ${offset}px 0 #000`
}

export function drawCountOnCanvas(
    context: CanvasRenderingContext2D,
    count: number,
    iconSize: number,
    iconX: number,
    iconY: number,
) {
    context.fillStyle = '#fff'
    context.font = `bold ${countFontSize(iconSize)}px sans-serif`
    context.textAlign = 'right'
    context.textBaseline = 'bottom'
    context.shadowColor = '#000'
    context.shadowOffsetX = Math.max(1, Math.round(iconSize * 0.01))
    context.shadowOffsetY = Math.max(1, Math.round(iconSize * 0.01))
    context.fillText(
        String(count),
        iconX + iconSize - countInset(iconSize),
        iconY + iconSize - countInset(iconSize),
    )
    context.shadowColor = 'transparent'
}
