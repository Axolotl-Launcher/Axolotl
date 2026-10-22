import assert from 'node:assert/strict'
import test from 'node:test'

import type { BrowseSearchResponse } from '../types.ts'
import { projectCardDates } from './project-dates.ts'

const result = {
    date_created: '2020-01-01',
    date_modified: '2025-01-01',
    resource_dates: {
        createdAt: '2020-01-01',
        projectPublishedAt: '2020-06-01',
        updatedAt: '2025-01-01',
        firstPublishedAt: '2021-01-01',
        latestPublishedAt: '2024-01-01',
    },
} as BrowseSearchResponse['projectHits'][number]

test('cards display the date dimension requested by the sort', () => {
    assert.deepEqual(projectCardDates(result, 'newest'), {
        datePublished: '2020-06-01',
        displayedDate: 'projectPublished',
    })
    assert.deepEqual(projectCardDates(result, 'created'), {
        datePublished: '2020-01-01',
        displayedDate: 'created',
    })
    assert.deepEqual(projectCardDates(result, 'firstPublished'), {
        datePublished: '2021-01-01',
        displayedDate: 'firstRelease',
    })
    assert.deepEqual(projectCardDates(result, 'latestPublished'), {
        dateUpdated: '2024-01-01',
        displayedDate: 'releaseUpdated',
    })
    assert.deepEqual(projectCardDates(result, 'updated'), {
        dateUpdated: '2025-01-01',
        displayedDate: 'projectModified',
    })
})

test('missing release dates remain absent and protocol-only callers retain their dates', () => {
    assert.equal(
        projectCardDates(
            { ...result, resource_dates: { createdAt: '2020-01-01' } },
            'firstPublished',
        ).datePublished,
        undefined,
    )
    assert.deepEqual(projectCardDates({ ...result, resource_dates: undefined }, 'newest'), {
        datePublished: '2020-01-01',
        dateUpdated: '2025-01-01',
        displayedDate: 'published',
    })
})
