import { computed, ref, watch } from 'vue'

export const INSTANCE_NAME_MAX_LENGTH = 80

type NameSave = { name: string; revision: number; tail: Promise<boolean> }
const nameSaves = new Map<string, NameSave>()

export function useInstanceName(
    instanceId: string,
    initialName: string,
    save: (name: string) => Promise<void>,
) {
    const pending = nameSaves.get(instanceId)
    const title = ref(pending?.name ?? initialName)
    const savedName = ref(initialName)
    const saving = ref(!!pending)
    const failed = ref(false)
    const invalid = computed(
        () => !title.value.trim() || title.value.length > INSTANCE_NAME_MAX_LENGTH,
    )
    let revision = 0
    let inFlight: Promise<void> | null = null
    let pendingName: string | null = null

    if (pending) {
        void pending.tail
            .then((saved) => {
                if (revision !== 0) return
                if (saved) savedName.value = pending.name
            })
            .catch(() => {
                if (revision === 0) failed.value = true
            })
            .finally(() => {
                if (revision === 0) saving.value = false
            })
    }

    function saveName(): Promise<void> {
        const name = title.value.trim()
        if (invalid.value)
            return (
                nameSaves
                    .get(instanceId)
                    ?.tail.then(() => {})
                    .catch(() => {}) ?? Promise.resolve()
            )
        if (inFlight && pendingName === name) return inFlight
        if (!nameSaves.has(instanceId) && name === savedName.value) return Promise.resolve()
        const currentRevision = ++revision
        saving.value = true
        failed.value = false
        pendingName = name
        const queue = nameSaves.get(instanceId) ?? {
            name,
            revision: 0,
            tail: Promise.resolve(true),
        }
        queue.name = name
        const queuedRevision = ++queue.revision
        // The instance owns the write order, including editors remounted by a tab change.
        const operation = queue.tail
            .catch(() => false)
            .then(async () => {
                if (queue.revision !== queuedRevision) return false
                await save(name)
                return true
            })
        queue.tail = operation
        nameSaves.set(instanceId, queue)
        inFlight = operation
            .then((saved) => {
                if (saved && currentRevision === revision) savedName.value = name
                if (queue.tail === operation) nameSaves.delete(instanceId)
            })
            .catch(() => {
                if (currentRevision === revision) failed.value = true
            })
            .finally(() => {
                if (currentRevision !== revision) return
                saving.value = false
                inFlight = null
                pendingName = null
            })
        return inFlight
    }

    watch(title, () => {
        failed.value = false
        void saveName()
    })

    function normalize() {
        title.value = title.value.trim()
    }

    return { title, saving, failed, invalid, saveName, normalize }
}
