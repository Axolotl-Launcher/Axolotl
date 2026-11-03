import { expect, it } from 'vitest'

import { type DownloadEmptyStateInput, selectDownloadEmptyState } from './downloads-empty-state.ts'

function emptyState(overrides: Partial<DownloadEmptyStateInput> = {}) {
    return selectDownloadEmptyState({
        query: '',
        tab: 'active',
        providerFiltered: false,
        historyStatusFiltered: false,
        sourceJobCount: 0,
        ...overrides,
    })
}

it('an empty history tab explains that there is no history', () => {
    expect(emptyState({ tab: 'history' })).toBe('no-download-history')
})

it('an empty active tab keeps the downloads wording', () => {
    expect(emptyState()).toBe('no-downloads')
})

it('a search explains the empty list on either tab', () => {
    expect(emptyState({ query: 'sodium' })).toBe('no-search-result')
    expect(emptyState({ tab: 'history', query: 'sodium' })).toBe('no-search-result')
})

it('a whitespace-only search is not a search', () => {
    expect(emptyState({ query: '   ' })).toBe('no-downloads')
    expect(emptyState({ tab: 'history', query: '   ' })).toBe('no-download-history')
})

it('a filter explains the empty list only when it could have hidden records', () => {
    expect(emptyState({ providerFiltered: true, sourceJobCount: 3 })).toBe('no-search-result')
    expect(emptyState({ tab: 'history', historyStatusFiltered: true, sourceJobCount: 2 })).toBe(
        'no-search-result',
    )
    expect(emptyState({ providerFiltered: true, sourceJobCount: 0 })).toBe('no-downloads')
    expect(emptyState({ tab: 'history', historyStatusFiltered: true, sourceJobCount: 0 })).toBe(
        'no-download-history',
    )
})

it('the history status filter does not narrow the active tab', () => {
    expect(emptyState({ historyStatusFiltered: true, sourceJobCount: 5 })).toBe('no-downloads')
})
