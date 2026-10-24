export type DownloadEmptyStateKind = 'no-search-result' | 'no-downloads' | 'no-download-history'

export interface DownloadEmptyStateInput {
    /** Current search box content. */
    query: string
    tab: 'active' | 'history'
    /** The provider dropdown holds something other than "all". */
    providerFiltered: boolean
    /** The history status dropdown holds something other than "all". */
    historyStatusFiltered: boolean
    /** Unfiltered job count for the current tab. */
    sourceJobCount: number
}

/**
 * Chooses which empty state the Downloads page shows.
 *
 * A search always explains an empty list; a filter only does when it could have
 * hidden records that exist on the other side of the filter. Otherwise the tab's
 * own "nothing here yet" wording wins, so an empty history tab never claims there
 * are no downloads.
 */
export function selectDownloadEmptyState(input: DownloadEmptyStateInput): DownloadEmptyStateKind {
    const narrowedByFilter =
        input.providerFiltered || (input.tab === 'history' && input.historyStatusFiltered)
    if (input.query.trim() || (narrowedByFilter && input.sourceJobCount > 0)) {
        return 'no-search-result'
    }
    return input.tab === 'history' ? 'no-download-history' : 'no-downloads'
}
