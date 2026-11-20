import { type QueryClient, useQuery, useQueryClient } from '@tanstack/vue-query'
import { watch } from 'vue'

import { useScopedEventListeners } from '@/composables/use-scoped-event-listeners'
import { createCoalescedRefresh } from '@/helpers/coalesced-refresh'
import { DIRECT_LINKS_CHANGED_EVENT } from '@/helpers/direct-link-sync'
import { instance_groups_listener, instance_listener } from '@/helpers/events'
import { instanceListQueryOptions } from '@/pages/instance/query-options'

export function useInstanceList(onError: (error: unknown) => void) {
    const client = useQueryClient()
    const options = instanceListQueryOptions()
    const query = useQuery(options)
    watch(query.error, (error) => {
        if (error) onError(error)
    })
    return {
        data: query.data,
        load: () => client.fetchQuery(options),
    }
}

/** One event subscription for all consumers of the shared list query. */
export function useInstanceListUpdates(client: QueryClient, onError: (error: unknown) => void) {
    const { subscribe, isDisposed } = useScopedEventListeners()
    const options = instanceListQueryOptions()
    const filter = { queryKey: options.queryKey, exact: true }
    const refresh = createCoalescedRefresh(async () => {
        if (client.isFetching(filter)) {
            // The IPC request cannot be cancelled. Let it finish, then read
            // again so a change during that request is not lost.
            await client.fetchQuery({ ...options, staleTime: 0 }).catch(() => {})
        }
        if (!isDisposed()) {
            await client.invalidateQueries(filter, { cancelRefetch: false })
        }
    }, isDisposed)
    const schedule = () => {
        void refresh()
    }

    void subscribe(instance_listener, (event: { event: string }) => {
        if (['created', 'edited', 'removed'].includes(event.event)) schedule()
    }).catch(onError)
    void subscribe(instance_groups_listener, schedule).catch(onError)
    void subscribe<Event>((callback) => {
        window.addEventListener(DIRECT_LINKS_CHANGED_EVENT, callback)
        return () => window.removeEventListener(DIRECT_LINKS_CHANGED_EVENT, callback)
    }, schedule)
}
