import { expect, it } from 'vitest'

import { isDiscoverAreaPath, shouldWarnAboutUnsavedSelection } from './unsaved-selection'

it('warns when leaving the discover area with an uninstalled selection', () => {
    expect(shouldWarnAboutUnsavedSelection('/browse/mod', '/settings', 3)).toBe(true)
    expect(shouldWarnAboutUnsavedSelection('/browse/favorites', '/instance/example', 1)).toBe(true)
    expect(shouldWarnAboutUnsavedSelection('/browse/mod', '/library', 2)).toBe(true)
})

it('does not warn while moving within the discover area', () => {
    expect(shouldWarnAboutUnsavedSelection('/browse/mod', '/project/sodium', 2)).toBe(false)
    expect(shouldWarnAboutUnsavedSelection('/project/sodium', '/resource/example', 2)).toBe(false)
    expect(shouldWarnAboutUnsavedSelection('/browse/mod', '/browse/favorites', 2)).toBe(false)
    expect(shouldWarnAboutUnsavedSelection('/resource/example', '/browse', 2)).toBe(false)
})

it('does not warn when nothing is selected', () => {
    expect(shouldWarnAboutUnsavedSelection('/browse/mod', '/settings', 0)).toBe(false)
})

it('does not warn when entering the discover area', () => {
    expect(shouldWarnAboutUnsavedSelection('/settings', '/browse/mod', 4)).toBe(false)
})

it('recognizes every discover area route prefix', () => {
    expect(isDiscoverAreaPath('/browse')).toBe(true)
    expect(isDiscoverAreaPath('/browse/mod')).toBe(true)
    expect(isDiscoverAreaPath('/project/sodium')).toBe(true)
    expect(isDiscoverAreaPath('/resource/example')).toBe(true)
    expect(isDiscoverAreaPath('/settings')).toBe(false)
    expect(isDiscoverAreaPath('/instance/example')).toBe(false)
})
