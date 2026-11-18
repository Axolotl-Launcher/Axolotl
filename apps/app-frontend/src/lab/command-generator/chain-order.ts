/**
 * Moves one entry inside a subcommand chain.
 *
 * Returns a new array so the caller owns the result; `from` and `to` are
 * positions in the original list, and out-of-range or no-op moves return the
 * list unchanged.
 */
export function moveChainItem<T>(items: readonly T[], from: number, to: number): T[] {
    const next = [...items]
    if (from === to || from < 0 || to < 0 || from >= next.length || to >= next.length) {
        return next
    }
    const [moved] = next.splice(from, 1)
    next.splice(to, 0, moved)
    return next
}
