import type { BrowseSearchResponse } from '../types'

export function projectCardDates(
    result: BrowseSearchResponse['projectHits'][number],
    sort: string,
) {
    const dates = result.resource_dates
    if (!dates)
        return {
            datePublished: result.date_created,
            dateUpdated: result.date_modified,
            displayedDate: sort === 'newest' ? ('published' as const) : ('updated' as const),
        }
    switch (sort) {
        case 'newest':
            return {
                datePublished: dates.projectPublishedAt ?? undefined,
                displayedDate: 'projectPublished' as const,
            }
        case 'created':
            return {
                datePublished: dates.createdAt ?? undefined,
                displayedDate: 'created' as const,
            }
        case 'firstPublished':
            return {
                datePublished: dates.firstPublishedAt ?? undefined,
                displayedDate: 'firstRelease' as const,
            }
        case 'latestPublished':
            return {
                dateUpdated: dates.latestPublishedAt ?? undefined,
                displayedDate: 'releaseUpdated' as const,
            }
        default:
            return {
                dateUpdated: dates.updatedAt ?? undefined,
                displayedDate: 'projectModified' as const,
            }
    }
}
