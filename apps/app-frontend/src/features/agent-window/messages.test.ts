import { afterEach, expect, it } from 'vitest'

import { dateLabel, setLocale, text } from '../../../../../packages/agent-window/src/messages.js'

afterEach(() => setLocale('en-US'))

it.each([
    ['zh-CN', '新建任务', 'zh-CN'],
    ['zh-TW', 'New task', 'en-US'],
    ['en-US', 'New task', 'en-US'],
    ['fr-FR', 'New task', 'en-US'],
    [undefined, 'New task', 'en-US'],
])(
    'uses the supported dictionary and time format for %s',
    (locale, expectedLabel, expectedLocale) => {
        const timestamp = new Date(2026, 9, 31, 13, 5).getTime()

        setLocale(locale)

        expect.soft(text('newChat')).toBe(expectedLabel)
        expect(dateLabel(timestamp)).toBe(
            new Date(timestamp).toLocaleTimeString(expectedLocale, {
                hour: '2-digit',
                minute: '2-digit',
            }),
        )
    },
)

it('resets Simplified Chinese messages when switching to Traditional Chinese', () => {
    setLocale('zh-CN')
    expect(text('waitingCount', { count: 2 })).toBe('2 个请求等待执行服务')

    setLocale('zh-TW')
    expect(text('waitingCount', { count: 2 })).toBe('2 requests waiting for execution service')
    expect(text('unknown-message')).toBe('unknown-message')
})
