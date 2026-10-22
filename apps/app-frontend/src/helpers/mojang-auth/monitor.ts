export type MojangAuthSource = 'auto' | 'official_only' | 'mirror_preferred' | 'official_preferred'

export const mojangAuthKeys = { status: ['mojang', 'auth', 'reachability'] as const }

export interface MojangAuthConfiguration {
    source: MojangAuthSource
    key: string
}

export interface MojangServiceStatus {
    service: string
    reachable: boolean
}

export function officialMojangServicesReachable(statuses: MojangServiceStatus[]): boolean {
    return ['session', 'services'].every((service) =>
        statuses.some((status) => status.service === service && status.reachable),
    )
}

interface Dependencies {
    configuration: () => Promise<MojangAuthConfiguration>
    probeOfficial: () => Promise<MojangServiceStatus[]>
    probeMirror: () => Promise<MojangServiceStatus[]>
    applySource: (source: MojangAuthSource, useMirror: boolean) => Promise<boolean>
    probeSelected: () => Promise<void>
    enabled: () => boolean
    publish: (reachable: boolean) => void
}

/** One application-owned worker for startup, periodic, reconnect and settings checks. */
export function createMojangAuthMonitor(deps: Dependencies) {
    let worker: Promise<boolean | null> | undefined
    let generation = 0
    let pending = false
    let disposed = false
    let configurationKey: string | undefined

    const active = () => !disposed && deps.enabled()

    async function check(): Promise<boolean | null> {
        const current = generation
        const isCurrent = () => active() && generation === current
        const configuration = await deps.configuration()
        if (!isCurrent()) return null
        configurationKey = configuration.key
        const automatic =
            configuration.source === 'auto' || configuration.source === 'official_preferred'
        let useMirror = configuration.source === 'mirror_preferred'
        if (automatic) {
            const official = await deps.probeOfficial()
            if (!isCurrent()) return null
            // A failed auxiliary endpoint must not replace working core
            // authentication services. Probe a fallback before applying it to authentication.
            if (!officialMojangServicesReachable(official)) {
                const mirror = await deps.probeMirror()
                if (!isCurrent()) return null
                useMirror = officialMojangServicesReachable(mirror)
            }
        }
        if (!isCurrent()) return null

        // Recheck after the slow network request, before applying its decision.
        const latest = await deps.configuration()
        if (!isCurrent()) return null
        if (latest.key !== configuration.key) {
            generation++
            pending = true
            return null
        }
        if (!(await deps.applySource(configuration.source, useMirror))) {
            if (isCurrent()) {
                generation++
                pending = true
            }
            return null
        }
        if (!isCurrent()) return null
        let reachable = true
        try {
            await deps.probeSelected()
        } catch {
            reachable = false
        }
        if (!isCurrent()) return null
        deps.publish(reachable)
        return reachable
    }

    return {
        run(): Promise<boolean | null> {
            if (!active()) return Promise.resolve(null)
            if (worker) return worker
            pending = true
            worker = Promise.resolve().then(async () => {
                let result: boolean | null = null
                try {
                    while (pending && active()) {
                        pending = false
                        const current = generation
                        try {
                            result = await check()
                        } catch (error) {
                            if (current === generation || !pending || !active()) throw error
                        }
                    }
                    return result
                } finally {
                    worker = undefined
                }
            })
            return worker
        },
        invalidate() {
            generation++
            pending = !!worker
        },
        async configurationChanged() {
            const configuration = await deps.configuration()
            return active() && configuration.key !== configurationKey
        },
        dispose() {
            disposed = true
            generation++
            pending = false
        },
    }
}
