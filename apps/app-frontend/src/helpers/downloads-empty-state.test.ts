import assert from 'node:assert/strict'
import test from 'node:test'

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

test('an empty history tab explains that there is no history', () => {
    assert.equal(emptyState({ tab: 'history' }), 'no-download-history')
})

test('an empty active tab keeps the downloads wording', () => {
    assert.equal(emptyState(), 'no-downloads')
})

test('a search explains the empty list on either tab', () => {
    assert.equal(emptyState({ query: 'sodium' }), 'no-search-result')
    assert.equal(emptyState({ tab: 'history', query: 'sodium' }), 'no-search-result')
})

test('a whitespace-only search is not a search', () => {
    assert.equal(emptyState({ query: '   ' }), 'no-downloads')
    assert.equal(emptyState({ tab: 'history', query: '   ' }), 'no-download-history')
})

test('a filter explains the empty list only when it could have hidden records', () => {
    assert.equal(emptyState({ providerFiltered: true, sourceJobCount: 3 }), 'no-search-result')
    assert.equal(
        emptyState({ tab: 'history', historyStatusFiltered: true, sourceJobCount: 2 }),
        'no-search-result',
    )
    assert.equal(emptyState({ providerFiltered: true, sourceJobCount: 0 }), 'no-downloads')
    assert.equal(
        emptyState({ tab: 'history', historyStatusFiltered: true, sourceJobCount: 0 }),
        'no-download-history',
    )
})

test('the history status filter does not narrow the active tab', () => {
    assert.equal(emptyState({ historyStatusFiltered: true, sourceJobCount: 5 }), 'no-downloads')
})
