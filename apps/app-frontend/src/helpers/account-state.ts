import { readonly, ref } from 'vue'

const revision = ref(0)
export const accountSelectionRevision = readonly(revision)
export function notifyAccountSelectionChanged() {
    revision.value++
}
