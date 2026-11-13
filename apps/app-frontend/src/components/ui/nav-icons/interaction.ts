import { inject, type InjectionKey, provide, readonly, type Ref, ref } from 'vue'

/** What a nav button tells the icon in its slot. */
export interface NavIconInteraction {
    /** True while the pointer is over an enabled button. */
    hovered: Readonly<Ref<boolean>>
    /** Increments on each primary-button press. */
    presses: Readonly<Ref<number>>
    /** Increments on each click, including keyboard activation. */
    activations: Readonly<Ref<number>>
}

const navIconInteractionKey: InjectionKey<NavIconInteraction> = Symbol('navIconInteraction')

/**
 * Called by `NavButton`. Returns the writable state; the icon in its slot gets
 * a read-only view through `injectNavIconInteraction`, the same way `NavRail`
 * shares its optimistic selection with the buttons in its slot.
 */
export function provideNavIconInteraction() {
    const state = { hovered: ref(false), presses: ref(0), activations: ref(0) }
    provide(navIconInteractionKey, {
        hovered: readonly(state.hovered),
        presses: readonly(state.presses),
        activations: readonly(state.activations),
    })
    return state
}

/** Called by a nav icon; null outside a `NavButton`, where the icon stays still. */
export function injectNavIconInteraction(): NavIconInteraction | null {
    return inject(navIconInteractionKey, null)
}
