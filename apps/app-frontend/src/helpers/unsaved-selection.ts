/**
 * Routes whose content selection is shared across the discover experience.
 * Moving between these keeps the selection, so navigation never prompts.
 */
export function isDiscoverAreaPath(path: string): boolean {
    return (
        path === '/browse' ||
        path.startsWith('/browse/') ||
        path.startsWith('/project/') ||
        path.startsWith('/resource/')
    )
}

/**
 * Warn before leaving the discover area when the user has selected content
 * that has not been installed yet; leaving would silently discard it.
 */
export function shouldWarnAboutUnsavedSelection(
    fromPath: string,
    toPath: string,
    selectedCount: number,
): boolean {
    return selectedCount > 0 && isDiscoverAreaPath(fromPath) && !isDiscoverAreaPath(toPath)
}
