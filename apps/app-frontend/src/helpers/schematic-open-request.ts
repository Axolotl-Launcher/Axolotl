type SchematicOpenListener = (path: string) => void

const listeners = new Set<SchematicOpenListener>()

/**
 * Hand a schematic file the shell opened over to the workshop page, if it is
 * mounted.
 *
 * The route cannot carry this on its own: opening the same file twice produces
 * an identical location, which the router skips, so retrying a file that failed
 * to parse would do nothing at all.
 *
 * @returns whether a mounted workshop took the request
 */
export function requestSchematicOpen(path: string): boolean {
    for (const listener of listeners) listener(path)
    return listeners.size > 0
}

/** Subscribes to those hand-offs; call the result to unsubscribe. */
export function onSchematicOpenRequest(listener: SchematicOpenListener): () => void {
    listeners.add(listener)
    return () => {
        listeners.delete(listener)
    }
}
