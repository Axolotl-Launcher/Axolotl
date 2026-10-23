/**
 * Time source for simulated work (downloads, launches, installs, servers).
 *
 * Nothing runs until the preview mounts: the clock is created by the preview
 * root and disposed with it. Progress is advanced at a coarse cadence and only
 * while some task is subscribed, so an idle preview schedules no timers.
 */
export interface PreviewClock {
    /** Calls `task` with the elapsed milliseconds roughly four times a second until removed. */
    every: (task: (elapsedMs: number) => void) => () => void
    /** Runs `callback` once after `ms`; returns a cancel function. */
    after: (ms: number, callback: () => void) => () => void
    dispose: () => void
}

const TICK_MS = 250

export function createPreviewClock(): PreviewClock {
    const tasks = new Set<(elapsedMs: number) => void>()
    const timeouts = new Set<ReturnType<typeof setTimeout>>()
    let tickTimer: ReturnType<typeof setTimeout> | null = null
    let lastTick = 0
    let disposed = false

    function tick() {
        tickTimer = null
        const now = performance.now()
        const elapsed = now - lastTick
        lastTick = now
        for (const task of [...tasks]) task(elapsed)
        if (tasks.size && !disposed) tickTimer = setTimeout(tick, TICK_MS)
    }

    function every(task: (elapsedMs: number) => void) {
        if (disposed) return () => {}
        tasks.add(task)
        if (tickTimer === null) {
            lastTick = performance.now()
            tickTimer = setTimeout(tick, TICK_MS)
        }
        return () => {
            tasks.delete(task)
            if (!tasks.size && tickTimer !== null) {
                clearTimeout(tickTimer)
                tickTimer = null
            }
        }
    }

    function after(ms: number, callback: () => void) {
        if (disposed) return () => {}
        const handle = setTimeout(() => {
            timeouts.delete(handle)
            callback()
        }, ms)
        timeouts.add(handle)
        return () => {
            clearTimeout(handle)
            timeouts.delete(handle)
        }
    }

    function dispose() {
        disposed = true
        tasks.clear()
        if (tickTimer !== null) clearTimeout(tickTimer)
        tickTimer = null
        for (const handle of timeouts) clearTimeout(handle)
        timeouts.clear()
    }

    return { every, after, dispose }
}
