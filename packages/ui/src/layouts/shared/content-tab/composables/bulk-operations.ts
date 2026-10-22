import { onBeforeUnmount, ref, watch } from 'vue'

export type BulkOperationType = 'enable' | 'disable' | 'delete' | 'update'

export function useBulkOperation() {
    const isBulkOperating = ref(false)
    const bulkProgress = ref(0)
    const bulkTotal = ref(0)
    const bulkOperation = ref<BulkOperationType | null>(null)
    const bulkWaiting = ref(false)

    function resetState() {
        isBulkOperating.value = false
        bulkOperation.value = null
        bulkProgress.value = 0
        bulkTotal.value = 0
        bulkWaiting.value = false
    }

    async function runBulk<T>(
        operation: BulkOperationType,
        items: T[],
        fn: (item: T) => Promise<void>,
        options?: { delayMs?: number; onComplete?: () => void },
    ) {
        const delayMs = options?.delayMs ?? 250
        isBulkOperating.value = true
        bulkOperation.value = operation
        bulkTotal.value = items.length
        bulkProgress.value = 0

        try {
            for (const item of items) {
                await fn(item)
                bulkProgress.value++
                if (delayMs > 0 && bulkProgress.value < items.length) {
                    await new Promise((resolve) => setTimeout(resolve, delayMs))
                }
            }
            options?.onComplete?.()
        } finally {
            resetState()
        }
    }

    async function runBulkWithWaiting(
        operation: BulkOperationType,
        total: number,
        fn: () => Promise<void | boolean>,
        onComplete?: () => void,
    ) {
        isBulkOperating.value = true
        bulkOperation.value = operation
        bulkTotal.value = total
        bulkProgress.value = 0
        bulkWaiting.value = true
        try {
            const result = await fn()
            if (result !== false) onComplete?.()
        } finally {
            resetState()
        }
    }

    function handleBeforeUnload(e: BeforeUnloadEvent) {
        if (isBulkOperating.value) {
            e.preventDefault()
            return ''
        }
    }

    if (typeof window !== 'undefined') {
        watch(isBulkOperating, (operating) => {
            if (operating) {
                window.addEventListener('beforeunload', handleBeforeUnload)
            } else {
                window.removeEventListener('beforeunload', handleBeforeUnload)
            }
        })

        onBeforeUnmount(() => {
            window.removeEventListener('beforeunload', handleBeforeUnload)
        })
    }

    return {
        isBulkOperating,
        bulkProgress,
        bulkTotal,
        bulkOperation,
        bulkWaiting,
        runBulk,
        runBulkWithWaiting,
    }
}
