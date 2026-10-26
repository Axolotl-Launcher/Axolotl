import { IntlMessageFormat } from 'intl-messageformat'
import { expect, it, vi } from 'vitest'

import english from '@/locales/en-US/index.json'
import chinese from '@/locales/zh-CN/index.json'

import {
    isResourceOperationError,
    resourceOperationError,
    resourceSearchFailure,
} from './resource-operation-logic'
import { parseResourceSearchFilter } from './resource-search-filter'
import type { ResourceSearchQuery } from './resources'

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

const browseQuery: ResourceSearchQuery = {
    text: '',
    types: ['mod'],
    sources: [],
    target: { gameVersions: [], loaders: [], channels: [] },
    categories: [],
    sort: 'relevance',
    pageSize: 20,
    filter: parseResourceSearchFilter(
        '(environment = `client_only` OR environment = `client_or_server`) AND game_versions = `26.2` AND categories = `fabric` AND project_types = `mod`',
        undefined,
        (value) => value === 'fabric',
    ),
}

const filterFailure = {
    source: 'curseforge' as const,
    code: 'sourceUnavailable',
    message: 'unsupported resource operation: CurseForge cannot express these search filters',
    retryable: false,
}

it('explains the client environment filter failure when browsing all sources without a keyword', () => {
    expect(resourceSearchFailure(filterFailure, browseQuery, formatter('zh-CN'))).toBe(
        'CurseForge 不支持按客户端或服务端筛选。移除运行环境条件后即可显示其结果。',
    )
    expect(resourceSearchFailure(filterFailure, browseQuery, formatter('en-US'))).toBe(
        'CurseForge does not support filtering by client or server. Remove the environment filter to include its results.',
    )
    expect(
        resourceSearchFailure(filterFailure, { ...browseQuery, filter: null }, formatter('zh-CN')),
    ).toBe('CurseForge 不支持当前筛选组合，请减少筛选条件或切换内容来源。')
})

it('distinguishes an unsupported sort from environment filters and connection failures', () => {
    expect(
        resourceSearchFailure(
            {
                ...filterFailure,
                message: 'unsupported resource operation: source cannot provide Followers ordering',
            },
            { ...browseQuery, sort: 'followers' },
            formatter('zh-CN'),
        ),
    ).toBe('CurseForge 不支持当前排序方式。切换为“相关性”或“下载量”后即可显示其结果。')
    expect(
        resourceSearchFailure(
            { ...filterFailure, message: 'resource request failed (HTTP status None)' },
            browseQuery,
            formatter('zh-CN'),
        ),
    ).toBe('CurseForge：无法连接内容来源，请检查网络后重试。')
    expect(
        resourceSearchFailure(
            { ...filterFailure, message: 'resource request failed (HTTP status Some(503))' },
            browseQuery,
            formatter('zh-CN'),
        ),
    ).toBe('CurseForge：内容来源拒绝了请求（HTTP 503），请稍后重试。')
})

it('keeps unknown source diagnostics out of the UI without blaming active filters', () => {
    const failure = { ...filterFailure, message: 'opaque backend diagnostic' }
    expect(resourceSearchFailure(failure, browseQuery, formatter('zh-CN'))).toBe(
        'CurseForge 暂时无法加载搜索结果，请稍后重试。',
    )
    expect(failure.message).toBe('opaque backend diagnostic')
})
