import type { Ref } from 'vue'

/** Controls inside a modal that can receive keyboard focus, in document order. */
export const MODAL_FOCUSABLE_SELECTOR =
    'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])'

export interface ModalFocusController {
    /** Remembers the element that had focus before the modal opened. */
    captureFocus: () => void
    /** Focuses the first focusable child, falling back to the modal body itself. */
    focusInitial: () => void
    /** Returns focus to the element captured by `captureFocus`. */
    restoreFocus: () => void
    /** Wraps Tab and Shift+Tab between the first and last focusable children. */
    trapTab: (event: KeyboardEvent) => void
    /** Returns the focusable children of the modal body in document order. */
    getFocusableElements: () => HTMLElement[]
}

/**
 * Owns the focus lifecycle of a modal: capturing the previously focused
 * element, moving focus into the modal, cycling Tab within it and restoring
 * focus when the modal closes. Open and close timing stays with the modal.
 */
export function useModalFocus(container: Ref<HTMLElement | null>): ModalFocusController {
    let previouslyFocused: HTMLElement | null = null

    function getFocusableElements(): HTMLElement[] {
        if (!container.value) return []
        return Array.from(container.value.querySelectorAll<HTMLElement>(MODAL_FOCUSABLE_SELECTOR))
    }

    function captureFocus(): void {
        if (typeof document === 'undefined') return
        previouslyFocused =
            document.activeElement instanceof HTMLElement ? document.activeElement : null
    }

    function focusInitial(attempt = 0): void {
        const focusable = getFocusableElements()
        const target = focusable[0] ?? container.value
        if (!target) return
        target.focus()
        // The opening transition starts with the body hidden, which makes the
        // first focus attempt a no-op. Retry briefly once it becomes visible.
        if (document.activeElement === target || attempt >= 2) return
        requestAnimationFrame(() => focusInitial(attempt + 1))
    }

    function restoreFocus(): void {
        if (previouslyFocused) {
            previouslyFocused.focus()
        }
        previouslyFocused = null
    }

    function trapTab(event: KeyboardEvent): void {
        if (event.key !== 'Tab') return
        const focusable = getFocusableElements()
        if (focusable.length === 0) return

        const first = focusable[0]
        const last = focusable[focusable.length - 1]
        const active = document.activeElement

        if (event.shiftKey) {
            // The body itself receives fallback focus, so treat it as the
            // boundary too; otherwise Shift+Tab would reach the page behind.
            if (active === first || active === container.value) {
                event.preventDefault()
                last.focus()
            }
        } else if (active === last) {
            event.preventDefault()
            first.focus()
        }
    }

    return { captureFocus, focusInitial, restoreFocus, trapTab, getFocusableElements }
}
