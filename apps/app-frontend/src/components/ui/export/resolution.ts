import type { ExportEmbeddedFailureChoice, ExportResolutionSession } from '@/helpers/resources'

export function pendingFailures(
    session: ExportResolutionSession | null,
    embedded: string[],
): ExportEmbeddedFailureChoice[] {
    return (
        session?.files.flatMap((file) =>
            file.resolutions
                .filter(
                    (resolution) =>
                        resolution.kind === 'failed' &&
                        embedded.includes(file.snapshot.relativePath),
                )
                .map((resolution) => ({ snapshot: file.snapshot, source: resolution.source })),
        ) ?? []
    )
}
