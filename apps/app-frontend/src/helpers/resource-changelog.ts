import type { ResourceText, SourceReleaseRef } from './resources'
import type { TranslationRequest, TranslationSegment } from './translation-batching'

export function changelogTranslationRequest(
    reference: SourceReleaseRef,
    text: ResourceText,
    language: string,
    segments: TranslationSegment[],
): TranslationRequest {
    return {
        source_language: 'auto',
        target_language: language,
        context: {
            title: JSON.stringify([
                'changelog',
                reference.project.source,
                reference.project.projectId,
                reference.releaseId,
            ]),
            description: JSON.stringify([text.format, text.body]),
        },
        segments,
    }
}
