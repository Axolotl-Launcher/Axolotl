import { describe, expect, it, vi } from 'vitest'

import { createMessageCompiler } from './i18n'

const { constructions } = vi.hoisted(() => ({ constructions: { count: 0 } }))

vi.mock('intl-messageformat', async (importOriginal) => {
    const actual = await importOriginal<typeof import('intl-messageformat')>()
    return {
        default: class extends actual.default {
            constructor(...args: ConstructorParameters<typeof actual.default>) {
                super(...args)
                constructions.count += 1
            }
        },
    }
})

function format(message: string, locale: string, values: Record<string, unknown>): string {
    const compile = createMessageCompiler()
    const formatMessage = compile(message, {
        locale,
        key: 'test',
    } as Parameters<typeof compile>[1]) as (context: { values: Record<string, unknown> }) => string
    return formatMessage({ values })
}

describe('message compiler cache', () => {
    it('reuses a formatter for the same locale and message', () => {
        const message = '{count, plural, one {# item} other {# items}}'
        constructions.count = 0

        expect(format(message, 'en-US', { count: 2 })).toBe('2 items')
        expect(format(message, 'en-US', { count: 2 })).toBe('2 items')
        expect(constructions.count).toBe(1)
    })

    it('does not reuse a formatter across locales', () => {
        const message = '{value, number}'
        constructions.count = 0

        expect(format(message, 'en-US', { value: 1234.5 })).toBe('1,234.5')
        expect(format(message, 'de-DE', { value: 1234.5 })).toBe('1.234,5')
        expect(constructions.count).toBe(2)
    })
})
