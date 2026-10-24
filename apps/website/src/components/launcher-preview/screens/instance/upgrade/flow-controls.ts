import type { MaybeRef } from 'vue'
import { toValue } from 'vue'

export function upgradeControlEnabled(value: MaybeRef<boolean> | undefined): boolean {
    return toValue(value ?? false)
}

export const UPGRADE_ACTIVE_STEPS = [
    'upgrade',
    'compatibility',
    'customize',
    'confirm',
    'progress',
] as const

export interface UpgradeProgressModel {
    currentIndex: number
    complete: boolean
    steps: typeof UPGRADE_ACTIVE_STEPS
}

export function upgradeProgressModel(path: string): UpgradeProgressModel {
    const routeStep = path.split('/').filter(Boolean).at(-1) ?? 'upgrade'
    const complete = routeStep === 'result'
    const index = UPGRADE_ACTIVE_STEPS.indexOf(routeStep as (typeof UPGRADE_ACTIVE_STEPS)[number])
    return {
        currentIndex: complete ? UPGRADE_ACTIVE_STEPS.length - 1 : Math.max(index, 0),
        complete,
        steps: UPGRADE_ACTIVE_STEPS,
    }
}

export function initialCustomizeStrategy<T>(
    flowStrategy: T | null | undefined,
    selectedStrategy: T | null | undefined,
    defaultStrategy: T,
): T {
    return flowStrategy ?? selectedStrategy ?? defaultStrategy
}

export function bulkResolutionAction(
    actions: Array<'upgrade' | 'keep' | 'disable'>,
): 'keep' | 'disable' | null {
    if (!actions.length) return null
    const unique = new Set(actions)
    return unique.size === 1 && (unique.has('keep') || unique.has('disable'))
        ? ([...unique][0] as 'keep' | 'disable')
        : null
}

export function filterBulkResolutionIds(
    items: Array<{ contentId: string; action: 'upgrade' | 'keep' | 'disable' }>,
    action: 'keep' | 'disable',
): string[] {
    return items.filter((item) => item.action !== action).map((item) => item.contentId)
}
