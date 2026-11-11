import { afterEach, describe, expect, it, vi } from 'vitest'

import {
    ACCOUNT_ORDER_STORAGE_KEY,
    readAccountOrder,
    sortMinecraftAccountsWithOrder,
    writeAccountOrder,
} from './account-order'
import type { MinecraftCredential } from './auth-types'

function account(
    id: string,
    name: string,
    type: MinecraftCredential['account_type'] = 'microsoft',
): MinecraftCredential {
    return {
        account_id: id,
        account_type: type,
        profile: { id: `${id}-profile`, name },
    }
}

function stubStorage(initial: Record<string, string> = {}) {
    const values = new Map(Object.entries(initial))
    const storage = {
        getItem: (key: string) => values.get(key) ?? null,
        setItem: (key: string, value: string) => {
            values.set(key, value)
        },
    }
    vi.stubGlobal('localStorage', storage)
    return values
}

afterEach(() => {
    vi.unstubAllGlobals()
})

describe('account order storage', () => {
    it('round-trips the order through local storage', () => {
        const values = stubStorage()

        writeAccountOrder(['microsoft:b', 'microsoft:a'])

        expect(JSON.parse(values.get(ACCOUNT_ORDER_STORAGE_KEY)!)).toEqual([
            'microsoft:b',
            'microsoft:a',
        ])
        expect(readAccountOrder()).toEqual(['microsoft:b', 'microsoft:a'])
    })

    it('treats missing or malformed values as no custom order', () => {
        stubStorage()
        expect(readAccountOrder()).toEqual([])

        stubStorage({ [ACCOUNT_ORDER_STORAGE_KEY]: 'not json' })
        expect(readAccountOrder()).toEqual([])

        stubStorage({ [ACCOUNT_ORDER_STORAGE_KEY]: '{"order":["microsoft:a"]}' })
        expect(readAccountOrder()).toEqual([])
    })

    it('drops duplicate and non-string ids', () => {
        stubStorage({
            [ACCOUNT_ORDER_STORAGE_KEY]: JSON.stringify(['microsoft:a', 'microsoft:a', 7, '']),
        })
        expect(readAccountOrder()).toEqual(['microsoft:a'])
    })
})

describe('sortMinecraftAccountsWithOrder', () => {
    it('keeps the default ordering without a saved order', () => {
        const sorted = sortMinecraftAccountsWithOrder(
            [account('microsoft:z', 'Zulu'), account('offline:a', 'Alpha', 'offline')],
            [],
        )
        expect(sorted.map((entry) => entry.account_id)).toEqual(['microsoft:z', 'offline:a'])
    })

    it('follows the saved order', () => {
        const sorted = sortMinecraftAccountsWithOrder(
            [account('microsoft:a', 'Alpha'), account('microsoft:b', 'Beta')],
            ['microsoft:b', 'microsoft:a'],
        )
        expect(sorted.map((entry) => entry.account_id)).toEqual(['microsoft:b', 'microsoft:a'])
    })

    it('places accounts the saved order does not name after the named ones', () => {
        const sorted = sortMinecraftAccountsWithOrder(
            [
                account('offline:new', 'Zulu', 'offline'),
                account('microsoft:a', 'Alpha'),
                account('microsoft:new', 'Bravo'),
                account('microsoft:b', 'Beta'),
            ],
            ['microsoft:b', 'microsoft:a'],
        )
        expect(sorted.map((entry) => entry.account_id)).toEqual([
            'microsoft:b',
            'microsoft:a',
            'microsoft:new',
            'offline:new',
        ])
    })

    it('names an account saved before account_id existed the way the backend does', () => {
        const legacy = {
            account_id: '',
            account_type: 'yggdrasil' as const,
            profile: { id: 'profile-id', name: 'Legacy' },
            yggdrasil: { api_root: 'https://example.test/api', server_name: 'Example', login: 'l' },
        }
        const sorted = sortMinecraftAccountsWithOrder(
            [account('microsoft:a', 'Alpha'), legacy],
            ['yggdrasil:https://example.test/api:profile-id', 'microsoft:a'],
        )
        expect(sorted.map((entry) => entry.profile.name)).toEqual(['Legacy', 'Alpha'])
    })
})
