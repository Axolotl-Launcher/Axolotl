import { onUnmounted } from 'vue'

import { instance_listener } from '@/helpers/events.js'

export interface InstanceAppEvent {
    event: string
    instance_id?: string
}

export function useAppEvent(type: 'instance', callback: (event: InstanceAppEvent) => void) {
    if (type !== 'instance') return
    let unlisten: (() => void) | undefined
    let disposed = false
    void instance_listener((event: InstanceAppEvent) => {
        if (!disposed) callback(event)
    })
        .then((dispose) => {
            if (disposed) dispose()
            else unlisten = dispose
        })
        .catch(console.error)
    onUnmounted(() => {
        disposed = true
        unlisten?.()
    })
}
