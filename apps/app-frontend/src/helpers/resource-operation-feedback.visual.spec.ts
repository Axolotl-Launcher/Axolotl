import { IntlMessageFormat } from 'intl-messageformat'
import { expect, it, vi } from 'vitest'

import english from '@/locales/en-US/index.json'
import chinese from '@/locales/zh-CN/index.json'

import {
    resourceFailureFeedback,
    type ResourceFailureKind,
    resourcePlanIssueFeedback,
} from './resource-operation-feedback'

vi.mock('@modrinth/ui', () => ({ defineMessages: (messages: unknown) => messages }))

function formatter(locale: 'en-US' | 'zh-CN') {
    const catalog: Record<string, { message: string }> = locale === 'zh-CN' ? chinese : english
    return (descriptor: { id: string; defaultMessage?: string }) =>
        String(new IntlMessageFormat(catalog[descriptor.id].message, locale).format())
}

const cases: [string, ResourceFailureKind][] = [
    ['HTTP status Some(404)', 'notFound'],
    ['HTTP status Some(429)', 'rateLimited'],
    ['HTTP status Some(403)', 'accessDenied'],
    ['resource request failed (HTTP status Some(503))', 'server'],
    ['request timed out', 'timeout'],
    ['TLS certificate failure', 'tls'],
    ['resource request failed (HTTP status None)', 'network'],
    ['resource source is unavailable', 'unavailable'],
    ['invalid source response: JSON', 'invalidResponse'],
    ['no compatible release found', 'noCompatibleRelease'],
    ['release window exhausted; choose an exact release', 'catalogIncomplete'],
    ['select one exact primary artifact', 'chooseFile'],
    ['invalid artifact integrity or ownership', 'invalidArtifact'],
    ['artifact belongs to another resource', 'identity'],
    ['artifact requires a manual download', 'manualDownload'],
    ['selected local file does not match the exact source artifact', 'localMismatch'],
    ['checksum mismatch', 'integrity'],
    ['invalid artifact filename', 'path'],
    ['target already exists: mods/example.jar', 'occupied'],
    ['Access is denied (os error 5)', 'permission'],
    ['No space left on device', 'diskFull'],
    ['instance changed after review', 'stale'],
    ['plan was revised or already queued', 'planExpired'],
    ['instance is not ready for resource changes', 'instanceBusy'],
    ['unknown instance', 'instanceMissing'],
    ['resource cache unavailable: diagnostic', 'cache'],
    ['resource database unavailable: diagnostic', 'database'],
    ['dependency requires local metadata verification', 'analysis'],
    ['dependency constraints did not converge', 'constraints'],
    ['this content type requires its supported installation scope', 'scope'],
    ['required local file is missing, disabled or ambiguous: dependency.jar', 'missingFile'],
    ['verified rollback backup not available', 'backup'],
    ['resource request canceled', 'canceled'],
    ['unsupported resource operation: source', 'unsupported'],
    ['opaque internal diagnostic', 'unknown'],
]

it.each(cases)(
    'provides localized recovery text and a separate brief reason for %s',
    (diagnostic, kind) => {
        for (const language of ['en-US', 'zh-CN'] as const) {
            const error = new Error(diagnostic)
            const feedback = resourceFailureFeedback(error, formatter(language))
            expect(feedback.kind).toBe(kind)
            expect(feedback.detail).not.toBe(feedback.summary)
            expect(feedback.detail).not.toContain('app.resources.')
            expect(feedback.summary).not.toContain('app.resources.')
            if (language === 'zh-CN') {
                expect(feedback.detail).toMatch(/[\u4e00-\u9fff]/)
                expect(feedback.summary).toMatch(/[\u4e00-\u9fff]/)
            }
            expect(error.message).toBe(diagnostic)
        }
    },
)

it('distinguishes the selected content from an unresolved dependency without inventing a network fault', () => {
    const issue = {
        code: 'missingDependency',
        blocking: true,
        message: 'opaque backend diagnostic',
        selectionId: 'selected',
    }
    const selected = resourcePlanIssueFeedback(issue, formatter('zh-CN'), true)
    const dependency = resourcePlanIssueFeedback(issue, formatter('zh-CN'), false)
    expect(selected.kind).toBe('selected')
    expect(selected.detail).toContain('跳过依赖检查无法解决此问题')
    expect(dependency.kind).toBe('dependency')
    expect(dependency.detail).toContain('无法准备所需依赖')
    expect(
        resourcePlanIssueFeedback({ ...issue, code: 'sourceUnavailable' }, formatter('zh-CN'), true)
            .kind,
    ).toBe('selected')
})
