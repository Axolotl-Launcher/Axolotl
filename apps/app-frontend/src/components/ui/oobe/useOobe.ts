import { readonly, ref, shallowRef } from 'vue'

import { update } from '@/helpers/settings.ts'

import { pendingOobeSteps, withCompletedOobeStep } from './progress'
import { OOBE_STEPS, type OobeStepDefinition } from './steps'

type SaveRecord = (completed: string[]) => Promise<unknown>

/**
 * Owns when the OOBE shows and what it has recorded. The host only reports the
 * steps the user moved past; this writes them one at a time, in order, so a
 * quick double step never lands an older record over a newer one.
 */
export function createOobeController(
    save: SaveRecord = (completed) => update({ oobe_completed_steps: completed }),
) {
    const visible = ref(false)
    const steps = shallowRef<readonly OobeStepDefinition[]>([])
    /** What the settings hold; a step joins it only once its own write succeeded. */
    let persisted: string[] = []
    /** Steps reported this run, so a double report writes once. */
    const reported = new Set<string>()
    let writes: Promise<void> = Promise.resolve()

    /** Reads the record from the settings; returns whether any step is pending. */
    function prepare(record: readonly string[] | null | undefined): boolean {
        persisted = [...(record ?? [])]
        reported.clear()
        steps.value = pendingOobeSteps(OOBE_STEPS, persisted)
        return steps.value.length > 0
    }

    function show() {
        if (steps.value.length > 0) visible.value = true
    }

    /** Runs the whole flow again on request; every step is already recorded. */
    function replay() {
        steps.value = OOBE_STEPS
        visible.value = true
    }

    function complete(id: string): Promise<void> {
        if (reported.has(id) || persisted.includes(id)) return writes
        reported.add(id)
        writes = writes.then(async () => {
            // Built when the write runs, from what earlier writes actually stored, so a step
            // whose write failed is not carried into the next one and returns at the next start.
            const record = withCompletedOobeStep(persisted, id)
            try {
                await save(record)
                persisted = record
            } catch (error) {
                // The step returns at the next start; that beats trapping the user here.
                console.warn('Failed to record the OOBE step', id, error)
            }
        })
        return writes
    }

    async function finish() {
        await writes
        visible.value = false
    }

    return {
        visible: readonly(visible),
        steps,
        prepare,
        show,
        replay,
        complete,
        finish,
    }
}

export type OobeController = ReturnType<typeof createOobeController>
