/**
 * Port of `apps/app-frontend/src/helpers/install-progress.ts` (pure helpers,
 * copied without changes).
 */
export interface ProgressValue {
    current: number
    total: number
    secondary?: ProgressValue | null
}

export interface ProgressSnapshot {
    kind?: string
    provider?: string
    phase: string
    progress?: ProgressValue | null
    summary?: {
        bytes_downloaded: number
        bytes_total?: number | null
    }
    parallel?: {
        phase: string
        current: number
        total: number
    } | null
}

export interface ProgressTextSnapshot extends ProgressSnapshot {
    status: string
    details?: { type: string; step?: string } | null
    summary: {
        files_completed: number
        files_total?: number | null
        bytes_downloaded: number
        bytes_total?: number | null
    }
}

export type InstallProgressTextSource =
    | { type: 'required_files' | 'phase' }
    | { type: 'bytes' | 'items'; current: number; total: number }

export function effectiveInstallProgress(
    snapshot: ProgressSnapshot,
): ProgressValue | null | undefined {
    if (snapshot.phase === 'downloading_content' && snapshot.provider === 'curse_forge') {
        return snapshot.progress
    }
    if (snapshot.phase === 'downloading_content' && snapshot.progress?.secondary) {
        return snapshot.progress.secondary
    }
    if (
        snapshot.phase === 'downloading_content' &&
        snapshot.kind === 'change_content' &&
        snapshot.summary?.bytes_total != null &&
        snapshot.summary.bytes_total > 0
    ) {
        return {
            current: snapshot.summary.bytes_downloaded,
            total: snapshot.summary.bytes_total,
        }
    }

    return snapshot.progress
}

export function effectiveParallelProgress(
    snapshot: ProgressSnapshot,
): ProgressValue | null | undefined {
    if (!snapshot.parallel) return null
    return {
        current: snapshot.parallel.current,
        total: snapshot.parallel.total,
    }
}

export function preserveMonotonicProgress<T extends ProgressSnapshot & { status: string }>(
    current: T,
    next: T,
): T {
    if (current.phase !== next.phase || current.status !== next.status) return next
    const before = effectiveInstallProgress(current)
    const after = effectiveInstallProgress(next)
    if (
        !hasDeterminateInstallProgress(before) ||
        !hasDeterminateInstallProgress(after) ||
        before.total !== after.total ||
        after.current >= before.current
    )
        return next

    if (next.provider === 'curse_forge') {
        return { ...next, progress: current.progress }
    }
    if (next.phase === 'downloading_content' && next.progress?.secondary) {
        return { ...next, progress: { ...next.progress, secondary: before } }
    }
    if (
        next.phase === 'downloading_content' &&
        next.kind === 'change_content' &&
        next.summary?.bytes_total
    ) {
        return {
            ...next,
            summary: { ...next.summary, bytes_downloaded: before.current },
        }
    }
    return { ...next, progress: current.progress }
}

export function hasDeterminateInstallProgress(
    progress: ProgressValue | null | undefined,
): progress is ProgressValue {
    return (
        progress != null &&
        Number.isFinite(progress.current) &&
        Number.isFinite(progress.total) &&
        progress.current >= 0 &&
        progress.total > 0
    )
}

export function installProgressFraction(snapshot: ProgressSnapshot): number | null {
    const progress = effectiveInstallProgress(snapshot)
    if (!hasDeterminateInstallProgress(progress)) return null

    return Math.max(0, Math.min(1, progress.current / progress.total))
}

export function installProgressTextSource(
    snapshot: ProgressTextSnapshot,
): InstallProgressTextSource {
    if (snapshot.status === 'waiting_for_user') return { type: 'required_files' }

    const isContentDownload = snapshot.phase === 'downloading_content'
    const isByteDownload =
        snapshot.phase === 'exporting' ||
        snapshot.phase === 'downloading_pack_file' ||
        snapshot.phase === 'downloading_minecraft' ||
        (snapshot.phase === 'preparing_java' &&
            snapshot.details?.type === 'java' &&
            snapshot.details.step === 'downloading')
    const progress = effectiveInstallProgress(snapshot)
    if (hasDeterminateInstallProgress(progress)) {
        if (isContentDownload) {
            const usesSecondaryProgress =
                snapshot.provider !== 'curse_forge' &&
                (snapshot.progress?.secondary != null ||
                    (snapshot.kind === 'change_content' && snapshot.summary.bytes_total))
            return {
                type: usesSecondaryProgress ? 'bytes' : 'items',
                current: progress.current,
                total: progress.total,
            }
        }
        if (isByteDownload) {
            return { type: 'bytes', current: progress.current, total: progress.total }
        }
    }

    if (snapshot.progress != null) return { type: 'phase' }

    if ((isContentDownload || isByteDownload) && snapshot.summary.bytes_total) {
        return {
            type: 'bytes',
            current: snapshot.summary.bytes_downloaded,
            total: snapshot.summary.bytes_total,
        }
    }
    if (isContentDownload && snapshot.summary.files_total) {
        return {
            type: 'items',
            current: snapshot.summary.files_completed,
            total: snapshot.summary.files_total,
        }
    }

    return { type: 'phase' }
}
