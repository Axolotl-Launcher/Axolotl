import { inject, type InjectionKey, provide, ref, type ShallowRef } from 'vue'

export interface NavRailOptimisticSelection {
    /** Resolved route path of the item currently pressed, or null. */
    href: ShallowRef<string | null>
}

const navRailOptimisticKey: InjectionKey<NavRailOptimisticSelection> = Symbol(
    'navRailOptimisticSelection',
)

export function provideNavRailOptimisticSelection(): NavRailOptimisticSelection {
    const selection: NavRailOptimisticSelection = { href: ref<string | null>(null) }
    provide(navRailOptimisticKey, selection)
    return selection
}

export function injectNavRailOptimisticSelection(): NavRailOptimisticSelection | null {
    return inject(navRailOptimisticKey, null)
}
