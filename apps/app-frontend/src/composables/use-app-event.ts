import { instance_listener } from '@/helpers/events.js'

import { useScopedEventListeners } from './use-scoped-event-listeners'

export interface InstanceAppEvent {
    event: string
    instance_id?: string
}

export function useAppEvent(type: 'instance', callback: (event: InstanceAppEvent) => void) {
    if (type !== 'instance') return
    const { subscribe } = useScopedEventListeners()
    void subscribe(instance_listener, callback).catch(console.error)
}
