import { defineMessages } from '@modrinth/ui/src/composables/i18n.ts'

import zhCN from '~/locales/launcher-preview/screenshots.zh-CN'

import { registerPreviewMessages } from '../../lib/i18n'

registerPreviewMessages('zh-CN', zhCN)

/**
 * Strings of the screenshots page. Ids and English text are the desktop
 * app's own (apps/app-frontend/src/components/ui/screenshots-page).
 */
export const messages = defineMessages({
    selectGroup: {
        id: 'app.screenshots.selection.select-group',
        defaultMessage: 'Select all screenshots in {group}',
    },
    heading: {
        id: 'app.screenshots.heading',
        defaultMessage: 'Screenshots',
    },
    emptyHeading: {
        id: 'app.screenshots.empty-heading',
        defaultMessage: 'No screenshots yet',
    },
    emptyDescription: {
        id: 'app.screenshots.empty-description',
        defaultMessage: 'Screenshots you take in-game will appear here.',
    },
    noResultsHeading: {
        id: 'app.screenshots.no-results-heading',
        defaultMessage: 'No matching screenshots',
    },
    noResultsDescription: {
        id: 'app.screenshots.no-results-description',
        defaultMessage: 'Try a different search or filter.',
    },
    errorHeading: {
        id: 'app.screenshots.error-heading',
        defaultMessage: 'Failed to load screenshots',
    },
    skippedFolders: {
        id: 'app.screenshots.skipped-folders',
        defaultMessage:
            '{count, plural, one {Skipped # inaccessible instance folder} other {Skipped # inaccessible instance folders}}',
    },
    newest: {
        id: 'app.screenshots.sort.newest',
        defaultMessage: 'Newest',
    },
    oldest: {
        id: 'app.screenshots.sort.oldest',
        defaultMessage: 'Oldest',
    },
    name: {
        id: 'app.screenshots.sort.name',
        defaultMessage: 'Name',
    },
    custom: {
        id: 'app.screenshots.group.custom',
        defaultMessage: 'Custom group',
    },
    instance: {
        id: 'app.screenshots.group.instance',
        defaultMessage: 'Instance',
    },
    date: {
        id: 'app.screenshots.group.date',
        defaultMessage: 'Date',
    },
    none: {
        id: 'app.screenshots.group.none',
        defaultMessage: 'No grouping',
    },
    today: {
        id: 'app.screenshots.group.today',
        defaultMessage: 'Today',
    },
    yesterday: {
        id: 'app.screenshots.group.yesterday',
        defaultMessage: 'Yesterday',
    },
    thisWeek: {
        id: 'app.screenshots.group.this-week',
        defaultMessage: 'This week',
    },
    thisMonth: {
        id: 'app.screenshots.group.this-month',
        defaultMessage: 'This month',
    },
    allScreenshots: {
        id: 'app.screenshots.group.all-screenshots',
        defaultMessage: 'All screenshots',
    },
    ungrouped: {
        id: 'app.screenshots.group.ungrouped',
        defaultMessage: 'Ungrouped',
    },
    editGroup: {
        id: 'app.screenshots.group.edit',
        defaultMessage: 'Edit group name',
    },
    deleteGroup: {
        id: 'app.screenshots.group.delete',
        defaultMessage: 'Delete group',
    },
    deleteGroupDescription: {
        id: 'app.screenshots.group.delete-description',
        defaultMessage: 'Screenshots in this group will become ungrouped.',
    },
    instanceAndDate: {
        id: 'app.screenshots.preview.instance-and-date',
        defaultMessage: '{instance} · {date}',
    },
    copy: {
        id: 'app.screenshots.copy',
        defaultMessage: 'Copy image',
    },
    copied: {
        id: 'app.screenshots.copied',
        defaultMessage: 'Copied',
    },
    edit: {
        id: 'app.screenshots.edit',
        defaultMessage: 'Edit screenshot',
    },
    showInFolder: {
        id: 'app.screenshots.show-in-folder',
        defaultMessage: 'Show in folder',
    },
    goToInstance: {
        id: 'app.screenshots.go-to-instance',
        defaultMessage: 'Go to instance',
    },
    deleteTitle: {
        id: 'app.screenshots.delete-title',
        defaultMessage: 'Delete screenshot',
    },
    deleteDescription: {
        id: 'app.screenshots.delete-description',
        defaultMessage: 'Permanently delete {name}? This action cannot be undone.',
    },
    deleteSuccess: {
        id: 'app.screenshots.delete-success',
        defaultMessage: 'Screenshot deleted',
    },
    selectionAriaLabel: {
        id: 'app.screenshots.selection.aria-label',
        defaultMessage: 'Selected screenshots',
    },
    selectedCount: {
        id: 'app.screenshots.selection.selected-count',
        defaultMessage: '{count} selected',
    },
    exportZip: {
        id: 'app.screenshots.selection.export-zip',
        defaultMessage: 'Export ZIP',
    },
    newGroup: {
        id: 'app.screenshots.group.new',
        defaultMessage: 'New group',
    },
    removeFromGroup: {
        id: 'app.screenshots.selection.remove-from-group',
        defaultMessage: 'Remove from group',
    },
    zipArchive: {
        id: 'app.screenshots.selection.zip-archive',
        defaultMessage: 'ZIP archive',
    },
    globalExportFilename: {
        id: 'app.screenshots.selection.global-export-filename',
        defaultMessage: 'Modrinth screenshots.zip',
    },
    instanceExportFilename: {
        id: 'app.screenshots.selection.instance-export-filename',
        defaultMessage: '{instance} screenshots.zip',
    },
    bulkDeleteTitle: {
        id: 'app.screenshots.selection.delete-title',
        defaultMessage: 'Delete selected screenshots',
    },
    bulkDeleteDescription: {
        id: 'app.screenshots.selection.delete-description',
        defaultMessage:
            'Delete {count, plural, one {# screenshot} other {# screenshots}}? This action cannot be undone.',
    },
    bulkDeleteSuccess: {
        id: 'app.screenshots.selection.delete-success',
        defaultMessage: '{count, plural, one {# screenshot deleted} other {# screenshots deleted}}',
    },
    collapse: {
        id: 'app.screenshots.group.collapse',
        defaultMessage: 'Collapse group',
    },
    expand: {
        id: 'app.screenshots.group.expand',
        defaultMessage: 'Expand group',
    },
    emptyGroup: {
        id: 'app.screenshots.group.empty',
        defaultMessage: 'Drag and drop to add screenshots.',
    },
    select: {
        id: 'app.screenshots.select',
        defaultMessage: 'Select {name}',
    },
    deselect: {
        id: 'app.screenshots.deselect',
        defaultMessage: 'Deselect {name}',
    },
    moreActions: {
        id: 'app.screenshots.more-actions',
        defaultMessage: 'More actions',
    },
    search: {
        id: 'app.screenshots.search',
        defaultMessage: 'Search',
    },
    sortBy: {
        id: 'app.screenshots.sort-by',
        defaultMessage: 'Sort by',
    },
    groupBy: {
        id: 'app.screenshots.group-by',
        defaultMessage: 'Group by',
    },
})
