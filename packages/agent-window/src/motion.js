// Motion helpers. Every animation here explains a state change (where the composer
// went, what just arrived) and is skipped when the system asks for reduced motion.
export const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)')

let transition = null

// Morphs the page between two layouts with the View Transitions API. Elements
// that keep a view-transition-name (the composer) glide to their new place; the
// rest cross-fades. Falls back to an instant update.
export function morph(change, kind = 'page') {
    if (reducedMotion.matches || !document.startViewTransition || document.hidden) {
        return Promise.resolve(change())
    }
    transition?.skipTransition()
    document.documentElement.dataset.morph = kind
    let result
    transition = document.startViewTransition(() => {
        result = change()
        return result
    })
    const current = transition
    // A transition cut short by the next one, as when Ctrl ] steps through
    // tasks quickly, rejects `ready`; that is expected, not an error.
    current.ready.catch(() => {})
    current.finished.finally(() => {
        if (transition === current) {
            transition = null
            delete document.documentElement.dataset.morph
        }
    })
    return current.updateCallbackDone.then(() => result)
}

// Content that is shown again rather than arriving, such as a task opened from
// the list, skips the entrances CSS gives its parts (rows rising in, ticks
// drawing, icons popping): they jump to their end. Loops keep running, and the
// axolotl keeps its own moods.
export function finishEntrances(node) {
    for (const animation of node.getAnimations({ subtree: true })) {
        if (!(animation instanceof CSSAnimation)) continue
        if (animation.effect?.getComputedTiming().endTime === Infinity) continue
        if (animation.effect?.target?.closest('.axo')) continue
        animation.finish()
    }
}

export function animate(node, frames, options = {}) {
    if (reducedMotion.matches || !node?.isConnected || !node.getClientRects().length) return null
    const animation = node.animate(frames, {
        duration: 220,
        easing: 'cubic-bezier(0.23, 1, 0.32, 1)',
        ...options,
    })
    animation.finished.catch(() => {})
    return animation
}

// Keeps the transcript pinned to the newest content while the user is at the
// bottom, and leaves it alone once they scroll up to read.
export class ScrollFollower {
    constructor(scroller, onChange) {
        this.scroller = scroller
        this.onChange = onChange
        this.pinned = true
        this.frame = 0
        scroller.addEventListener(
            'scroll',
            () => {
                const pinned = this.distance() < 48
                if (pinned !== this.pinned) {
                    this.pinned = pinned
                    this.onChange?.(pinned)
                }
            },
            { passive: true },
        )
    }

    distance() {
        const { scrollHeight, scrollTop, clientHeight } = this.scroller
        return scrollHeight - scrollTop - clientHeight
    }

    follow() {
        if (!this.pinned || this.frame) return
        this.frame = requestAnimationFrame(() => {
            this.frame = 0
            this.scroller.scrollTop = this.scroller.scrollHeight
        })
    }

    toBottom(smooth = true) {
        this.pinned = true
        this.onChange?.(true)
        this.scroller.scrollTo({
            top: this.scroller.scrollHeight,
            behavior: smooth && !reducedMotion.matches ? 'smooth' : 'instant',
        })
    }
}
