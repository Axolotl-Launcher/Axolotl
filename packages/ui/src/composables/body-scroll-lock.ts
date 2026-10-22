import { onScopeDispose } from 'vue'

interface ScrollLock {
    owners: Set<symbol>
    overflowX: string
    overflowY: string
    priorityX: string
    priorityY: string
}

const locks = new WeakMap<HTMLElement, ScrollLock>()

/** Each scope holds at most one lock and can only release its own ownership. */
export function useBodyScrollLock() {
    const owner = Symbol()
    let body: HTMLElement | undefined
    let disposed = false

    function lock() {
        if (disposed || body || typeof document === 'undefined') return
        body = document.body
        let state = locks.get(body)
        if (!state) {
            state = {
                owners: new Set(),
                overflowX: body.style.getPropertyValue('overflow-x'),
                overflowY: body.style.getPropertyValue('overflow-y'),
                priorityX: body.style.getPropertyPriority('overflow-x'),
                priorityY: body.style.getPropertyPriority('overflow-y'),
            }
            locks.set(body, state)
            body.style.setProperty('overflow-x', 'hidden', state.priorityX)
            body.style.setProperty('overflow-y', 'hidden', state.priorityY)
        }
        state.owners.add(owner)
    }

    function unlock() {
        if (!body) return
        const state = locks.get(body)
        if (state) {
            state.owners.delete(owner)
            if (state.owners.size === 0) {
                body.style.removeProperty('overflow')
                if (state.overflowX)
                    body.style.setProperty('overflow-x', state.overflowX, state.priorityX)
                if (state.overflowY)
                    body.style.setProperty('overflow-y', state.overflowY, state.priorityY)
                locks.delete(body)
            }
        }
        body = undefined
    }

    onScopeDispose(() => {
        disposed = true
        unlock()
    })
    return { lock, unlock }
}
