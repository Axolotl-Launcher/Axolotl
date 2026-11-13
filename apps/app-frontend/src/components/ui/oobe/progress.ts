/**
 * Which OOBE steps an install still has to see. A step counts as seen once the
 * user moves past it (Next, Skip or finishing), so a release that adds a step
 * with a new id shows that step alone at the next start.
 */
export function pendingOobeSteps<T extends { id: string }>(
    steps: readonly T[],
    completed: readonly string[] | null | undefined,
): T[] {
    const seen = new Set(completed ?? [])
    return steps.filter((step) => !seen.has(step.id))
}

/**
 * The record after `id` was seen. Ids this build does not know are kept: they
 * belong to a newer build the install ran before, and dropping them would show
 * that build's steps again after the user returns to it.
 */
export function withCompletedOobeStep(completed: readonly string[], id: string): string[] {
    return completed.includes(id) ? [...completed] : [...completed, id]
}
