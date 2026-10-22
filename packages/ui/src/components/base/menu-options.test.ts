import assert from 'node:assert/strict'
import test from 'node:test'

import { visibleMenuOptions } from './menu-options.ts'

test('object capabilities share section ordering and omit unavailable sections', () => {
    const actions = visibleMenuOptions([
        { id: 'delete', section: 'danger' },
        { id: 'pin-home', section: 'pin' },
        { id: 'edit', section: 'manage' },
        { id: 'play', section: 'primary' },
        { id: 'folder', section: 'navigate' },
        { id: 'hidden', section: 'primary', shown: false },
    ])
    assert.deepEqual(
        actions.filter((action) => 'id' in action).map((action) => action.id),
        ['play', 'edit', 'folder', 'pin-home', 'delete'],
    )
    assert.equal(actions.filter((action) => 'divider' in action).length, 4)
    assert.deepEqual(
        visibleMenuOptions([
            { id: 'pin', section: 'pin', shown: false },
            { id: 'delete', section: 'danger' },
        ]),
        [{ id: 'delete', section: 'danger' }],
    )
})

test('generic menus retain order and action identity', () => {
    const action = () => {}
    const options = [{ id: 'second', action }, { divider: true }, { id: 'first' }]
    assert.deepEqual(visibleMenuOptions(options), options)
})
