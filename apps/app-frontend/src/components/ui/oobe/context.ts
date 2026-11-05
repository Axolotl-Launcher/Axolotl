import { inject, type InjectionKey } from 'vue'

/** What a step may ask of the OOBE shell. Steps never navigate on their own. */
export interface OobeStepContext {
    /** Holds navigation while the step finishes work it started, such as a sign-in. */
    setBusy(busy: boolean): void
    /** Enables Next. A step that needs input before Next means anything starts not ready. */
    setReady(ready: boolean): void
    /**
     * Records a successful account or import change during this OOBE session. The step is
     * recorded as done right away, so a restart before Next does not offer to repeat it.
     */
    markChanged(): void
    /** Moves on as the primary action would. */
    next(): void
}

export const oobeStepContextKey: InjectionKey<OobeStepContext> = Symbol('oobe-step')

export function useOobeStep(): OobeStepContext {
    const context = inject(oobeStepContextKey, null)
    if (!context) throw new Error('An OOBE step must be rendered by OobeHost')
    return context
}
