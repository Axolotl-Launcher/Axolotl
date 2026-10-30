import { expect, it } from 'vitest'

import {
    effectiveInstallProgress,
    effectiveParallelProgress,
    hasDeterminateInstallProgress,
    installProgressFraction,
    installProgressTextSource,
    preserveMonotonicProgress,
    type ProgressTextSnapshot,
} from './install-progress.ts'

function progressJob(overrides: Record<string, unknown> = {}) {
    return {
        status: 'running',
        phase: 'resolving_loader',
        progress: null,
        details: { type: 'empty' },
        summary: {
            files_completed: 0,
            files_total: null,
            bytes_downloaded: 0,
            bytes_total: null,
        },
        ...overrides,
    }
}

it('content snapshots preserve displayed bytes without losing new item state', () => {
    const before = {
        ...progressJob(),
        kind: 'change_content',
        phase: 'downloading_content',
        summary: { files_completed: 0, bytes_downloaded: 80, bytes_total: 100 },
        items: [{ status: 'downloading' }],
    }
    const next = {
        ...before,
        summary: { files_completed: 1, bytes_downloaded: 60, bytes_total: 100 },
        items: [{ status: 'completed' }],
    }
    const merged = preserveMonotonicProgress(before, next)
    expect(installProgressFraction(merged)).toBe(0.8)
    expect(merged.summary.files_completed).toBe(1)
    expect(merged.items[0].status).toBe('completed')
    expect(next.summary.bytes_downloaded).toBe(60)
})

it('new phases, retries and terminal results retain authoritative progress', () => {
    const before = { ...progressJob(), progress: { current: 80, total: 100 } }
    for (const next of [
        { ...before, phase: 'applying_content', progress: { current: 0, total: 100 } },
        { ...before, status: 'queued', progress: { current: 0, total: 100 } },
        { ...before, status: 'failed', progress: { current: 60, total: 100 } },
    ])
        expect(preserveMonotonicProgress(before, next)).toBe(next)
})

it('modpack secondary byte progress remains monotonic', () => {
    const before = {
        ...progressJob(),
        phase: 'downloading_content',
        progress: { current: 1, total: 3, secondary: { current: 80, total: 100 } },
    }
    const next = {
        ...before,
        progress: { current: 2, total: 3, secondary: { current: 60, total: 100 } },
    }
    const merged = preserveMonotonicProgress(before, next)
    expect(installProgressFraction(merged)).toBe(0.8)
    expect(merged.progress.current).toBe(2)
})

it('CurseForge modpacks use settled file progress for completion', () => {
    const job = {
        ...progressJob(),
        provider: 'curse_forge',
        phase: 'downloading_content',
        progress: { current: 3, total: 3, secondary: { current: 900, total: 1000 } },
    }

    expect(installProgressFraction(job)).toBe(1)
    expect(installProgressTextSource(job)).toEqual({ type: 'items', current: 3, total: 3 })
})

it('CurseForge file progress remains monotonic while byte samples change', () => {
    const before = {
        ...progressJob(),
        provider: 'curse_forge',
        phase: 'downloading_content',
        progress: { current: 8, total: 10, secondary: { current: 1000, total: 1000 } },
    }
    const next = {
        ...before,
        progress: { current: 7, total: 10, secondary: { current: 800, total: 1000 } },
    }
    const merged = preserveMonotonicProgress(before, next)

    expect(merged.progress?.current).toBe(8)
    expect(installProgressFraction(merged)).toBe(0.8)
})

it('CurseForge completion switches from byte progress to settled files', () => {
    const before = {
        ...progressJob(),
        provider: 'curse_forge',
        phase: 'downloading_content',
        progress: { current: 9, total: 10, secondary: { current: 999, total: 1000 } },
    }
    const completed = {
        ...before,
        progress: { current: 10, total: 10, secondary: { current: 999, total: 1000 } },
    }

    const merged = preserveMonotonicProgress(before, completed)
    expect(merged.progress?.current).toBe(10)
    expect(installProgressFraction(merged)).toBe(1)
})

it('clears completed content progress when the next phase has no progress', () => {
    const completed = {
        phase: 'downloading_content',
        progress: { current: 10, total: 10 },
    }
    expect(installProgressFraction(completed)).toBe(1)

    const nextPhase = {
        phase: 'downloading_minecraft',
        progress: null,
    }
    expect(effectiveInstallProgress(nextPhase)).toBe(null)
    expect(installProgressFraction(nextPhase)).toBe(null)
})

it('CurseForge extraction does not inherit completed download bytes through the monotonic guard', () => {
    const downloaded: ProgressTextSnapshot = {
        ...progressJob(),
        provider: 'curse_forge',
        phase: 'downloading_content',
        progress: { current: 10, total: 10, secondary: { current: 1000, total: 1000 } },
        summary: {
            files_completed: 10,
            files_total: 10,
            bytes_downloaded: 1000,
            bytes_total: 1000,
        },
    }
    for (const phase of ['extracting_overrides', 'finalizing']) {
        const next = { ...downloaded, phase, progress: null }
        const merged = preserveMonotonicProgress(downloaded, next)

        expect(merged).toBe(next)
        expect(installProgressFraction(merged)).toBe(null)
        expect(hasDeterminateInstallProgress(effectiveInstallProgress(merged))).toBe(false)
        expect(installProgressTextSource(merged)).toEqual({ type: 'phase' })
    }
})

it('CurseForge phase changes can restart progress even with the same total', () => {
    const before = {
        ...progressJob(),
        provider: 'curse_forge',
        phase: 'downloading_content',
        progress: { current: 99, total: 100 },
    }
    for (const phase of ['extracting_overrides', 'downloading_minecraft']) {
        const next = { ...before, phase, progress: { current: 1, total: 100 } }
        expect(installProgressFraction(preserveMonotonicProgress(before, next))).toBe(0.01)
    }
})

it('parallel track exposes its own progress', () => {
    const job = {
        phase: 'downloading_content',
        progress: { current: 2, total: 3, secondary: { current: 220, total: 300 } },
        parallel: {
            phase: 'downloading_minecraft',
            current: 120,
            total: 300,
        },
    }
    expect(effectiveInstallProgress(job)).toEqual({ current: 220, total: 300 })
    expect(effectiveParallelProgress(job)).toEqual({ current: 120, total: 300 })
    expect(installProgressFraction(job)).toBe(220 / 300)

    expect(effectiveParallelProgress({ phase: 'downloading_minecraft', progress: null })).toBe(null)
})

it('treats zero and non-finite totals as indeterminate', () => {
    for (const total of [0, Number.NaN, Number.POSITIVE_INFINITY]) {
        const progress = { current: 1, total }
        expect(hasDeterminateInstallProgress(progress)).toBe(false)
        expect(installProgressFraction({ phase: 'downloading_minecraft', progress })).toBe(null)
    }
})

it('non-download phase ignores historical byte and file summary', () => {
    const source = installProgressTextSource(
        progressJob({
            phase: 'resolving_loader',
            summary: {
                files_completed: 186,
                files_total: 187,
                bytes_downloaded: 268,
                bytes_total: 18,
            },
        }),
    )

    expect(source).toEqual({ type: 'phase' })
    expect(JSON.stringify(source)).not.toMatch(/268|18 MiB/)
})

it('non-download phase ignores historical file counter', () => {
    expect(
        installProgressTextSource(
            progressJob({
                phase: 'resolving_loader',
                summary: {
                    files_completed: 186,
                    files_total: 187,
                    bytes_downloaded: 0,
                    bytes_total: null,
                },
            }),
        ),
    ).toEqual({ type: 'phase' })
})

it('pack download uses current progress instead of stale summary', () => {
    expect(
        installProgressTextSource(
            progressJob({
                phase: 'downloading_pack_file',
                progress: { current: 0, total: 51 },
                summary: {
                    files_completed: 0,
                    files_total: null,
                    bytes_downloaded: 268,
                    bytes_total: 300,
                },
            }),
        ),
    ).toEqual({ type: 'bytes', current: 0, total: 51 })
})

it('minecraft download uses current progress instead of content summary', () => {
    expect(
        installProgressTextSource(
            progressJob({
                phase: 'downloading_minecraft',
                progress: { current: 0, total: 18 },
                summary: {
                    files_completed: 187,
                    files_total: 187,
                    bytes_downloaded: 268,
                    bytes_total: 300,
                },
            }),
        ),
    ).toEqual({ type: 'bytes', current: 0, total: 18 })
})

it('content download uses current secondary bytes', () => {
    expect(
        installProgressTextSource(
            progressJob({
                phase: 'downloading_content',
                progress: {
                    current: 2,
                    total: 3,
                    secondary: { current: 220, total: 300 },
                },
            }),
        ),
    ).toEqual({ type: 'bytes', current: 220, total: 300 })
})

it('content download without secondary uses current file counter', () => {
    expect(
        installProgressTextSource(
            progressJob({
                phase: 'downloading_content',
                progress: { current: 2, total: 3 },
            }),
        ),
    ).toEqual({ type: 'items', current: 2, total: 3 })
})

it('content change uses aggregate live download bytes without losing its queued counter', () => {
    const preparing = progressJob({
        kind: 'change_content',
        phase: 'downloading_content',
        progress: { current: 0, total: 3 },
    })
    expect(effectiveInstallProgress(preparing)).toEqual({ current: 0, total: 3 })

    const downloading = progressJob({
        kind: 'change_content',
        phase: 'downloading_content',
        progress: { current: 0, total: 3 },
        summary: {
            files_completed: 0,
            files_total: 3,
            bytes_downloaded: 200,
            bytes_total: 800,
        },
    })
    expect(effectiveInstallProgress(downloading)).toEqual({ current: 200, total: 800 })
    expect(installProgressTextSource(downloading)).toEqual({
        type: 'bytes',
        current: 200,
        total: 800,
    })
})

it('Java downloading uses current byte progress', () => {
    expect(
        installProgressTextSource(
            progressJob({
                phase: 'preparing_java',
                progress: { current: 4, total: 12 },
                details: { type: 'java', major_version: 21, step: 'downloading' },
            }),
        ),
    ).toEqual({ type: 'bytes', current: 4, total: 12 })
})

it('waiting job preserves required-file progress policy', () => {
    expect(
        installProgressTextSource(
            progressJob({
                status: 'waiting_for_user',
                phase: 'downloading_content',
                progress: { current: 2, total: 3 },
            }),
        ),
    ).toEqual({ type: 'required_files' })
})
