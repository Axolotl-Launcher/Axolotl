export function createThumbnailQueue(limit: number) {
    const queue: Array<{ start: () => void }> = []
    let active = 0
    function drain() {
        while (active < limit && queue.length) {
            active++
            queue.shift()!.start()
        }
    }
    return {
        enqueue<T>(task: () => Promise<T>) {
            let started = false
            let rejectQueued!: (reason: Error) => void
            let job!: { start: () => void }
            const promise = new Promise<T>((resolve, reject) => {
                rejectQueued = reject
                job = {
                    start() {
                        started = true
                        void Promise.resolve()
                            .then(task)
                            .then(resolve, reject)
                            .finally(() => {
                                active--
                                drain()
                            })
                    },
                }
                queue.push(job)
                drain()
            })
            return {
                promise,
                cancel() {
                    if (started) return false
                    const index = queue.indexOf(job)
                    if (index < 0) return false
                    queue.splice(index, 1)
                    rejectQueued(new Error('Thumbnail request released before starting'))
                    return true
                },
            }
        },
    }
}
