import { describe, expect, it, vi } from 'vitest'

import { onSchematicOpenRequest, requestSchematicOpen } from './schematic-open-request'

describe('schematic open requests', () => {
    it('reports that nobody took the request when no page is mounted', () => {
        expect(requestSchematicOpen('/home/me/house.schem')).toBe(false)
    })

    it('hands the path to every mounted listener and reports the request as taken', () => {
        const first = vi.fn()
        const second = vi.fn()
        const stopFirst = onSchematicOpenRequest(first)
        const stopSecond = onSchematicOpenRequest(second)

        expect(requestSchematicOpen('/home/me/house.schem')).toBe(true)
        expect(first).toHaveBeenCalledWith('/home/me/house.schem')
        expect(second).toHaveBeenCalledWith('/home/me/house.schem')

        stopFirst()
        stopSecond()
    })

    it('stops delivering to a listener that unsubscribed', () => {
        const listener = vi.fn()
        const stop = onSchematicOpenRequest(listener)
        stop()

        expect(requestSchematicOpen('/home/me/house.schem')).toBe(false)
        expect(listener).not.toHaveBeenCalled()
    })

    it('delivers a repeated path every time, unlike a route push', () => {
        const listener = vi.fn()
        const stop = onSchematicOpenRequest(listener)

        requestSchematicOpen('/home/me/house.schem')
        requestSchematicOpen('/home/me/house.schem')

        expect(listener).toHaveBeenCalledTimes(2)

        stop()
    })
})
