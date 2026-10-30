/** A navigation hint only; project requests remain authoritative for all actions. */
export interface ProjectPreview {
    targetPath: string
    title: string
}

export function createProjectPreviewState(targetPath: string, title: string) {
    return { projectPreview: { targetPath, title } satisfies ProjectPreview }
}

export function readProjectPreviewTitle(
    targetPath: string,
    state: unknown = window.history.state,
): string | undefined {
    if (!state || typeof state !== 'object' || !('projectPreview' in state)) return
    const preview = state.projectPreview
    if (
        !preview ||
        typeof preview !== 'object' ||
        !('targetPath' in preview) ||
        !('title' in preview)
    )
        return
    if (
        preview.targetPath === targetPath &&
        typeof preview.title === 'string' &&
        preview.title.trim()
    )
        return preview.title
}
