import type { Ref } from 'vue'

/** Keep only unfinished work so a retry never repeats a successful deletion. */
export async function deletePendingItems<T>(
    pending: Ref<T[]>,
    remove: (item: T) => Promise<void>,
    getId: (item: T) => string,
    completed: (id: string) => void,
) {
    for (const item of [...pending.value]) {
        await remove(item)
        const id = getId(item)
        pending.value = pending.value.filter((remaining) => getId(remaining) !== id)
        completed(id)
    }
}

export async function disablePendingItems<T>(
    pending: Ref<T[]>,
    disable: (items: T[]) => Promise<void | boolean>,
    failed: () => Error,
) {
    if (pending.value.length === 0) return
    if ((await disable([...pending.value])) === false) throw failed()
    pending.value = []
}
