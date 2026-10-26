import { type InstanceContentData, refreshInstanceContentUpdates } from './instance-content'

/** A page owns one whole-instance scan; metadata viewport changes never restart it. */
export function createContentUpdateScanner(commit: (data: InstanceContentData) => void) {
    let current:
        | {
              key: string
              started: number
              controller: AbortController
              pending: Promise<void>
              result?: InstanceContentData
          }
        | undefined

    function cancel() {
        current?.controller.abort()
        current = undefined
    }

    function scan(data: InstanceContentData): Promise<void> {
        const key = JSON.stringify([data.path, data.snapshot.revision])
        if (current?.key === key && Date.now() - current.started < 5 * 60_000) {
            if (current.result) commit(current.result)
            return current.pending
        }
        cancel()
        const controller = new AbortController()
        const pending = refreshInstanceContentUpdates(data, controller.signal).then((updated) => {
            if (!controller.signal.aborted && current?.controller === controller) {
                current.result = updated
                commit(updated)
            }
        })
        current = { key, started: Date.now(), controller, pending }
        return pending
    }

    return { scan, cancel }
}
