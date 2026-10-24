import { computed, type MaybeRef, onScopeDispose, ref, shallowRef, toValue, watch } from 'vue'

/** Snapshot requests keep the desktop page's query contract without a second app client. */
export function useQuery<T>(options: {
    queryKey: MaybeRef<readonly unknown[]>
    queryFn: (context: { queryKey: any }) => Promise<T> | T
    enabled?: MaybeRef<boolean>
    staleTime?: number
}) {
    const data = shallowRef<T>()
    const error = shallowRef<unknown>(null)
    const status = ref<'pending' | 'success' | 'error'>('pending')
    const isFetching = ref(false)
    let generation = 0
    async function refresh() {
        const current = ++generation
        if (options.enabled !== undefined && !toValue(options.enabled)) {
            isFetching.value = false
            return
        }
        isFetching.value = true
        try {
            const result = await options.queryFn({ queryKey: toValue(options.queryKey) })
            if (generation !== current) return
            data.value = result
            error.value = null
            status.value = 'success'
        } catch (failure) {
            if (generation !== current) return
            error.value = failure
            status.value = 'error'
        } finally {
            if (generation === current) isFetching.value = false
        }
    }
    watch(
        () => [toValue(options.queryKey), toValue(options.enabled ?? true)],
        () => {
            data.value = undefined
            status.value = 'pending'
            void refresh()
        },
        { immediate: true, deep: true },
    )
    onScopeDispose(() => generation++)
    return {
        data,
        error,
        isFetching,
        isPending: computed(() => status.value === 'pending'),
        isSuccess: computed(() => status.value === 'success'),
        isError: computed(() => status.value === 'error'),
        refetch: refresh,
    }
}
