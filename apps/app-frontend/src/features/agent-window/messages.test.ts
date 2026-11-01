import { afterEach, expect, it, vi } from 'vitest'

import { dateLabel, setLocale, text } from '../../../../../packages/agent-window/src/messages.js'
import english from '../../locales/en-US/index.json'
import chinese from '../../locales/zh-CN/index.json'
import { agentWindowMessages, formatAgentWindowMessages } from './messages'

vi.mock('@modrinth/ui', () => ({ defineMessages: (messages: unknown) => messages }))

afterEach(() => setLocale('en-US'))

it('registers every standalone message with the extraction and translation catalogs', () => {
    for (const message of Object.values(agentWindowMessages)) {
        expect(Reflect.get(english, message.id)?.message).toBe(message.defaultMessage)
        expect(Reflect.get(chinese, message.id)?.message).toEqual(expect.any(String))
    }
})

it.each(['zh-CN', 'zh-TW', 'en-US', 'fr-FR', undefined, 'invalid_locale'])(
    'formats times with the actual locale %s independently of message fallback',
    (locale) => {
        const timestamp = new Date(2026, 9, 31, 13, 5).getTime()
        const messages = Object.fromEntries(
            Object.entries(agentWindowMessages).map(([key, message]) => [
                key,
                locale === 'zh-CN'
                    ? Reflect.get(chinese, message.id).message
                    : message.defaultMessage,
            ]),
        )
        setLocale(locale, messages)
        expect(text('newChat')).toBe(locale === 'zh-CN' ? '新建任务' : 'New task')
        expect(dateLabel(timestamp)).toBe(
            new Date(timestamp).toLocaleTimeString(
                locale && locale !== 'invalid_locale' ? locale : 'en-US',
                { hour: '2-digit', minute: '2-digit' },
            ),
        )
    },
)

it('uses the formatted host catalog and replaces it when the locale changes', () => {
    setLocale('fr-FR', { newChat: 'Nouvelle tâche' })
    expect(text('newChat')).toBe('Nouvelle tâche')
    setLocale('en-US', { newChat: 'New task' })
    expect(text('newChat')).toBe('New task')
    expect(text('unknown-message')).toBe('unknown-message')
    setLocale('en-US')
    expect(text('newChat')).toBe('newChat')
})

it('fills simple arguments and leaves unknown ones for the reader to notice', () => {
    setLocale('en-US', { stepsDone: '{count} steps', toastDone: '“{title}” is done' })
    expect(text('stepsDone', { count: 3 })).toBe('3 steps')
    expect(text('toastDone', { title: '{count}' })).toBe('“{count}” is done')
    expect(text('toastDone', {})).toBe('“{title}” is done')
})

it('keeps argument placeholders when the launcher formats the catalog', () => {
    const format = vi.fn(
        (message: { defaultMessage: string }, values: Record<string, unknown> = {}) =>
            message.defaultMessage.replace(/\{(\w+)\}/g, (_, name: string) => String(values[name])),
    )
    const catalog = formatAgentWindowMessages(format)
    expect(Object.keys(catalog)).toEqual(Object.keys(agentWindowMessages))
    expect(catalog.applyingProgress).toBe('Applying changes · {done}/{total}')
    expect(catalog.newChat).toBe('New task')
    setLocale('en-US', catalog)
    expect(text('applyingProgress', { done: 1, total: 2 })).toBe('Applying changes · 1/2')
})
