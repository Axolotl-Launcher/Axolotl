export interface WindowAppearance {
    os: string
    transparent: boolean
    blur: boolean
    decorated: boolean
    maximized: boolean
}

export interface WindowFrameResult {
    cssBorder: boolean
    warnings: string[]
}

/** Serialize compositor and frame updates, replacing pending work with the latest window state. */
export function createWindowAppearanceController(
    applyEffects: (state: WindowAppearance) => Promise<void>,
    applyFrame: (state: WindowAppearance) => Promise<WindowFrameResult>,
    publishBorder: (enabled: boolean) => void,
    reportError: (error: unknown) => void,
) {
    let pending: WindowAppearance | undefined
    let active: Promise<void> | undefined
    let revision = 0
    let disposed = false

    async function drain() {
        try {
            while (pending && !disposed) {
                const state = pending
                const request = revision
                pending = undefined
                const warnings: string[] = []
                try {
                    await applyEffects(state)
                } catch (error) {
                    warnings.push(`Window effects: ${String(error)}`)
                }
                if (disposed) break
                try {
                    const result = await applyFrame(state)
                    if (request === revision && !disposed) {
                        publishBorder(
                            result.cssBorder &&
                                state.transparent &&
                                !state.decorated &&
                                !state.maximized,
                        )
                        warnings.push(...result.warnings)
                        if (warnings.length) reportError(warnings)
                    }
                } catch (error) {
                    if (request === revision && !disposed) {
                        publishBorder(false)
                        reportError(error)
                    }
                }
            }
        } finally {
            active = undefined
        }
    }

    return {
        update(state: WindowAppearance): Promise<void> {
            if (disposed) return Promise.resolve()
            revision++
            pending = { ...state }
            publishBorder(false)
            if (!active) active = drain()
            return active
        },
        dispose() {
            disposed = true
            revision++
            pending = undefined
        },
    }
}
