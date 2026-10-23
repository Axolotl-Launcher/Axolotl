import { usePreview } from '../state'

/** The app's `helpers/instance.ts` `list()`, `get_full_path()` and `remove()` for the preview. Call from setup. */
export function useInstanceApi() {
    const { instances, clock } = usePreview()
    return {
        list: async () => [...instances.instances.value],
        get_full_path: async (id: string) => instances.get(id)?.path ?? '',
        /** `instance_remove` deletes the instance folder; the preview drops it from the list. */
        remove: (id: string) =>
            new Promise<void>((resolve, reject) =>
                clock.after(240, () => {
                    if (!instances.get(id)) {
                        reject(new Error('Unknown instance'))
                        return
                    }
                    instances.remove(id)
                    resolve()
                }),
            ),
    }
}
