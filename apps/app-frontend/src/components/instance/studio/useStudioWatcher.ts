import { onScopeDispose } from 'vue'

import type { StudioFilesChangedEvent } from '@/helpers/studio'

type Source = { kind: 'instance'; id: string } | { kind: 'workspace'; path: string }
interface WatcherServices {
    listen: (handler: (event: StudioFilesChangedEvent) => void) => Promise<() => void>
    register: (id: string) => Promise<string>
    unregister: (id: string, registration: string) => Promise<void>
    watch: (path: string, handler: (paths: string[]) => void) => Promise<() => void>
}

/** Owns registrations and event batches for one Studio workspace at a time. */
export function useStudioWatcher(
    services: WatcherServices,
    onChange: (paths: string[]) => Promise<void>,
    onError: (error: unknown) => void,
) {
    let generation = 0
    let disposed = false
    let release: (() => Promise<void>) | undefined
    let timer: ReturnType<typeof setTimeout> | undefined
    let changedPaths = new Set<string>()

    async function stop() {
        generation++
        clearTimeout(timer)
        changedPaths.clear()
        const previous = release
        release = undefined
        await previous?.()
    }

    async function start(source: Source) {
        const stopping = stop()
        const current = generation
        const active = () => !disposed && generation === current
        await stopping
        if (!active()) return
        let batch = Promise.resolve()
        function schedule(paths: string[]) {
            if (!active() || paths.length === 0) return
            for (const path of paths) changedPaths.add(path)
            clearTimeout(timer)
            timer = setTimeout(() => {
                const paths = [...changedPaths]
                changedPaths = new Set()
                batch = batch
                    .then(async () => {
                        if (active()) await onChange(paths)
                    })
                    .catch(onError)
            }, 150)
        }
        let cleanup: (() => Promise<void>) | undefined
        try {
            if (source.kind === 'workspace') {
                const root = source.path.replaceAll('\\', '/').replace(/\/+$/, '')
                const unwatch = await services.watch(source.path, (paths) => {
                    schedule(
                        paths.flatMap((path) => {
                            const normalized = path.replaceAll('\\', '/')
                            if (normalized.startsWith(`${root}/`))
                                return [normalized.slice(root.length + 1)]
                            if (normalized === root || /^(?:[A-Za-z]:)?\//.test(normalized))
                                return []
                            return [normalized]
                        }),
                    )
                })
                cleanup = async () => unwatch()
            } else {
                let registration: string | undefined
                const unlisten = await services.listen((event) => {
                    if (
                        registration &&
                        event.instanceId === source.id &&
                        event.registrationId === registration
                    )
                        schedule(event.paths)
                })
                cleanup = async () => {
                    unlisten()
                    if (registration)
                        await services.unregister(source.id, registration).catch(onError)
                }
                if (active()) registration = await services.register(source.id)
            }
            if (active()) release = cleanup
            else await cleanup?.()
        } catch (error) {
            await cleanup?.()
            if (active()) onError(error)
        }
    }

    onScopeDispose(() => {
        disposed = true
        void stop()
    })
    return { start, stop }
}
