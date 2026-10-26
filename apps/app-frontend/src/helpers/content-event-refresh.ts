export function createContentEventRefresh(options: {
    busy: () => boolean
    revision: () => number | undefined
    refresh: (filesystem: boolean) => void
    delay: number
}) {
    let filesystem = false
    let changed = false
    let timer: ReturnType<typeof setTimeout> | undefined

    function clear() {
        if (timer) clearTimeout(timer)
        timer = undefined
        filesystem = false
        changed = false
    }

    function resume() {
        if (timer || (!filesystem && !changed) || options.busy()) return
        timer = setTimeout(() => {
            timer = undefined
            if (options.busy()) return
            const scan = filesystem
            clear()
            options.refresh(scan)
        }, options.delay)
    }

    function enqueue(event: { event: string; revision?: number }) {
        if (event.event === 'synced') filesystem = true
        else if (event.event === 'content_changed') {
            if (event.revision != null && event.revision <= (options.revision() ?? -1)) return
            changed = true
        } else return
        resume()
    }

    return { enqueue, resume, clear }
}
