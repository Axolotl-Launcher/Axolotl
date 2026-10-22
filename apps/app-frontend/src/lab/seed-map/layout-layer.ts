import type { SeedMapStructureDetails, SeedMapStructureQuery } from './advanced.ts'

type Job = {
    key: string
    query: SeedMapStructureQuery
    selected: boolean
    promise: Promise<SeedMapStructureDetails>
    resolve: (result: SeedMapStructureDetails) => void
    reject: (error: unknown) => void
}

/** Shares selected and visible layout requests, with a bounded native work queue. */
export function createSeedMapLayoutLayer(
    load: (query: SeedMapStructureQuery) => Promise<SeedMapStructureDetails>,
    onUpdate: () => void,
) {
    const cache = new Map<string, SeedMapStructureDetails>()
    const jobs = new Map<string, Job>()
    let visible = new Set<string>()
    let queue: Job[] = []
    let active = 0
    let disposed = false
    const key = (query: SeedMapStructureQuery) => JSON.stringify(query)
    function trim() {
        while (cache.size > Math.max(96, visible.size + 1)) {
            const oldest =
                [...cache.keys()].find((entry) => !visible.has(entry)) ?? cache.keys().next().value!
            cache.delete(oldest)
        }
    }
    function prune() {
        queue = queue.filter((job) => {
            if (visible.has(job.key) || job.selected) return true
            jobs.delete(job.key)
            job.reject(new Error('Layout is no longer visible'))
            return false
        })
    }
    function select(query: SeedMapStructureQuery | null) {
        const id = query ? key(query) : undefined
        for (const job of jobs.values()) job.selected = job.key === id
        prune()
    }
    function pump() {
        while (!disposed && active < 2 && queue.length) {
            const job = queue.shift()!
            active++
            void load(job.query)
                .then((result) => {
                    if (!disposed && (visible.has(job.key) || job.selected)) {
                        cache.delete(job.key)
                        cache.set(job.key, result)
                        trim()
                        onUpdate()
                    }
                    job.resolve(result)
                }, job.reject)
                .finally(() => {
                    jobs.delete(job.key)
                    active--
                    pump()
                })
        }
    }
    function request(query: SeedMapStructureQuery, selected: boolean) {
        if (selected) select(query)
        const id = key(query)
        const result = cache.get(id)
        if (result) return Promise.resolve(result)
        const pending = jobs.get(id)
        if (pending) {
            pending.selected ||= selected
            const index = selected ? queue.indexOf(pending) : -1
            if (index > 0) {
                queue.splice(index, 1)
                queue.unshift(pending)
            }
            return pending.promise
        }
        let resolve!: Job['resolve'], reject!: Job['reject']
        const promise = new Promise<SeedMapStructureDetails>((yes, no) => {
            resolve = yes
            reject = no
        })
        const job = { key: id, query: { ...query }, selected, promise, resolve, reject }
        jobs.set(id, job)
        if (selected) queue.unshift(job)
        else queue.push(job)
        pump()
        return promise
    }
    return {
        select,
        load: (query: SeedMapStructureQuery) => request(query, true),
        update(queries: SeedMapStructureQuery[]) {
            visible = new Set(queries.map(key))
            trim()
            prune()
            for (const query of queries) void request(query, false).catch(() => {})
        },
        peek: (query: SeedMapStructureQuery) => cache.get(key(query)),
        dispose() {
            disposed = true
            for (const job of queue) job.reject(new Error('Layout layer disposed'))
            queue = []
            cache.clear()
            visible.clear()
        },
    }
}
