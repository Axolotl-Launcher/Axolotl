/** Coalesces bursts and permits one trailing refresh while a read is in flight. */
export function createHomeRefresh(read: () => Promise<void>, disposed: () => boolean) {
    let pending: Promise<void> | undefined
    let dirty = false
    return function refresh(): Promise<void> {
        if (disposed()) return Promise.resolve()
        dirty = true
        if (!pending) {
            pending = (async () => {
                while (dirty && !disposed()) {
                    await new Promise((resolve) => setTimeout(resolve, 100))
                    if (disposed()) break
                    dirty = false
                    await read()
                }
            })().finally(() => {
                pending = undefined
            })
        }
        return pending
    }
}

export function homeWorldsChanged(event: string) {
    return [
        'created',
        'added',
        'removed',
        'edited',
        'world_updated',
        'server_joined',
        'servers_updated',
    ].includes(event)
}
