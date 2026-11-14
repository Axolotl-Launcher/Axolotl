import type { Ref } from 'vue'

/** Controls inside a modal that can receive keyboard focus, in document order. */
export const MODAL_FOCUSABLE_SELECTOR =
    'a[href], area[href], button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), iframe, summary, audio[controls], video[controls], [contenteditable]:not([contenteditable="false"]):not([tabindex="-1"]), [tabindex]:not([tabindex="-1"])'

export interface ModalFocusOptions {
    /** Returns whether this modal is the one that should own focus. */
    isTopmost?: () => boolean
}

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
export function useModalFocus(
    container: Ref<HTMLElement | null>,
    options: ModalFocusOptions = {},
): ModalFocusController {
    let previouslyFocused: HTMLElement | null = null
    let pendingFocusFrame: number | null = null

    function cancelPendingFocus(): void {
        if (pendingFocusFrame === null) return
        cancelAnimationFrame(pendingFocusFrame)
        pendingFocusFrame = null
    }

    /**
     * Hidden inputs, inert subtrees and CSS-hidden controls still match the
     * selector above, but focusing them silently does nothing. Filtering them
     * out keeps focus inside the modal instead of leaving it on the trigger.
     */
    function isFocusVisible(element: HTMLElement): boolean {
        if (element.matches('input[type="hidden"], :disabled')) return false
        if (element.closest('[hidden], [inert]')) return false
        const style = getComputedStyle(element)
        return (
            style.display !== 'none' &&
            style.visibility !== 'hidden' &&
            element.getClientRects().length > 0
        )
    }

    function getFocusableElements(): HTMLElement[] {
        if (!container.value) return []
        return Array.from(
            container.value.querySelectorAll<HTMLElement>(MODAL_FOCUSABLE_SELECTOR),
        ).filter(isFocusVisible)
    }

    function captureFocus(): void {
        if (typeof document === 'undefined') return
        cancelPendingFocus()
        previouslyFocused =
            document.activeElement instanceof HTMLElement ? document.activeElement : null
    }

    function focusInitial(attempt = 0): void {
        cancelPendingFocus()
        if (options.isTopmost && !options.isTopmost()) return
        const focusable = getFocusableElements()
        const target = focusable[0] ?? container.value
        if (!target) return
        target.focus()
        // The opening transition starts with the body hidden, which makes the
        // first focus attempt a no-op. Retry briefly once it becomes visible.
        if (document.activeElement === target || attempt >= 2) return
        pendingFocusFrame = requestAnimationFrame(() => {
            pendingFocusFrame = null
            if (options.isTopmost && !options.isTopmost()) return
            focusInitial(attempt + 1)
        })
    }

    function restoreFocus(): void {
        cancelPendingFocus()
        if (previouslyFocused) {
            previouslyFocused.focus()
        }
        previouslyFocused = null
    }

    function trapTab(event: KeyboardEvent): void {
        if (event.key !== 'Tab') return
        const focusable = getFocusableElements()
        if (focusable.length === 0) {
            event.preventDefault()
            // Keep focus inside the dialog; the body has tabindex="-1".
            const fallback = container.value ?? document.body
            fallback.focus()
            return
        }

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
