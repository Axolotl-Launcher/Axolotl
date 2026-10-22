import { IntlMessageFormat } from 'intl-messageformat'
import { expect, it, vi } from 'vitest'

import english from '@/locales/en-US/index.json'
import chinese from '@/locales/zh-CN/index.json'

import { isResourceOperationError, resourceOperationError } from './resource-operation-logic'

vi.mock('@modrinth/ui', () => ({ defineMessages: (messages: unknown) => messages }))

function formatter(locale: 'en-US' | 'zh-CN') {
    const messages = (locale === 'zh-CN' ? chinese : english) as Record<string, { message: string }>
    return (
        descriptor: { id: string; defaultMessage?: string },
        values?: Record<string, unknown>,
    ) =>
        String(
            new IntlMessageFormat(
                messages[descriptor.id]?.message ?? descriptor.defaultMessage ?? descriptor.id,
                locale,
            ).format(values as never),
        )
}

it('formats structured resource failures in the chosen language and preserves raw diagnostics for inspection', () => {
    const error = {
        code: 'resourceError',
        message: 'resource request failed (HTTP status Some(503))',
    }
    expect(isResourceOperationError(error)).toBe(true)
    expect(resourceOperationError(error)).toBe(error.message)
    expect(resourceOperationError(error, formatter('en-US'))).toBe(
        'The content source rejected the request (HTTP 503). Try again later.',
    )
    expect(resourceOperationError(error, formatter('zh-CN'))).toBe(
        '内容来源拒绝了请求（HTTP 503），请稍后重试。',
    )
})

it('localizes stale plans and invalid search filters without exposing English diagnostic text', () => {
    expect(
        resourceOperationError(new Error('instance changed after review'), formatter('zh-CN')),
    ).toBe('实例或所选文件已发生变化，请刷新预览后继续。')
    expect(
        resourceOperationError(new Error('Unsupported search filter field'), formatter('zh-CN')),
    ).toBe('无法应用这些筛选条件，请重置筛选后重试。')
    expect(isResourceOperationError(new Error('Source presentation is unavailable'))).toBe(true)
    expect(
        resourceOperationError(
            { code: 'cursorExpired', message: 'opaque diagnostic' },
            formatter('zh-CN'),
        ),
    ).toBe('搜索会话已过期，请重新搜索。')
    expect(isResourceOperationError(new Error('A user-provided project description'))).toBe(false)
})

it('uses a localized recovery message for unknown failures and leaves the error object intact', () => {
    const error = new Error('opaque backend diagnostic')
    expect(resourceOperationError(error, formatter('zh-CN'))).toBe(
        '无法完成内容操作，请刷新后重试。',
    )
    expect(error.message).toBe('opaque backend diagnostic')
})

it('explains incomplete version catalogs without calling them incompatible', () => {
    const error = new Error(
        'Source release window is incomplete; choose a specific game version or file',
    )
    expect(isResourceOperationError(error)).toBe(true)
    expect(resourceOperationError(error, formatter('zh-CN'))).toBe(
        '内容来源未能返回全部版本，请打开项目的“版本”标签页选择文件。',
    )
})
