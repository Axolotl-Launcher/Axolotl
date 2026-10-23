import { defineMessages } from '@modrinth/ui/src/composables/i18n.ts'

import zhCN from '~/locales/launcher-preview/library.zh-CN'

import { registerPreviewMessages } from '../../lib/i18n'

registerPreviewMessages('zh-CN', zhCN)

/**
 * Strings of the Library screen. Ids and English text are the desktop app's
 * own (`pages/library/Index.vue`, `components/GridDisplay.vue`,
 * `components/ui/library/InstanceGroup.vue`, `components/ui/Instance.vue`,
 * `components/ui/modal/InstanceGroupModal.vue` and
 * `components/ui/modal/ConfirmDeleteInstanceModal.vue`).
 */
export const messages = defineMessages({
    // pages/library/Index.vue
    library: { id: 'app.library.title', defaultMessage: 'Library' },
    allInstances: { id: 'app.library.tabs.all-instances', defaultMessage: 'All instances' },
    modpacks: { id: 'app.library.tabs.modpacks', defaultMessage: 'Modpacks' },
    servers: { id: 'app.library.tabs.servers', defaultMessage: 'Servers' },
    custom: { id: 'app.library.tabs.custom', defaultMessage: 'Custom' },
    shared: { id: 'app.library.tabs.shared', defaultMessage: 'Shared with me' },
    saved: { id: 'app.library.tabs.saved', defaultMessage: 'Saved' },
    noInstances: { id: 'app.library.no-instances', defaultMessage: 'No instances found' },
    createInstance: {
        id: 'app.library.create-instance',
        defaultMessage: 'Create new instance',
    },

    // components/GridDisplay.vue
    search: { id: 'app.instances.search', defaultMessage: 'Search' },
    select: { id: 'app.instances.select', defaultMessage: 'Select...' },
    groupBy: { id: 'app.instances.group-by', defaultMessage: 'Group by' },
    addContent: { id: 'app.instances.add-content', defaultMessage: 'Add content' },
    newGroup: { id: 'app.library.group.new', defaultMessage: 'New group' },
    viewInstance: { id: 'app.instances.view-instance', defaultMessage: 'View instance' },
    duplicateInstance: {
        id: 'app.instances.duplicate-instance',
        defaultMessage: 'Duplicate instance',
    },
    copyPath: { id: 'app.instances.copy-path', defaultMessage: 'Copy path' },
    pinToHome: { id: 'app.instances.pin-to-home', defaultMessage: 'Pin to Home' },
    unpinFromHome: { id: 'app.instances.unpin-from-home', defaultMessage: 'Unpin from Home' },
    name: { id: 'app.instances.sort.name', defaultMessage: 'Name' },
    lastPlayed: { id: 'app.instances.sort.last-played', defaultMessage: 'Last played' },
    dateCreated: { id: 'app.instances.sort.date-created', defaultMessage: 'Date created' },
    dateModified: { id: 'app.instances.sort.date-modified', defaultMessage: 'Date modified' },
    gameVersion: { id: 'app.instances.group.game-version', defaultMessage: 'Game version' },
    group: { id: 'app.instances.group.group', defaultMessage: 'Custom group' },
    loader: { id: 'app.instances.group.loader', defaultMessage: 'Loader' },
    none: { id: 'app.instances.group.none', defaultMessage: 'No grouping' },
    selectedCount: {
        id: 'app.instances.selected-count',
        defaultMessage: '{count, plural, one {# selected} other {# selected}}',
    },
    view: { id: 'app.library.view', defaultMessage: 'View' },
    standardView: { id: 'app.library.view.standard', defaultMessage: 'Standard grid' },
    cardsView: { id: 'app.library.view.cards', defaultMessage: 'Library cards' },
    sortBy: { id: 'app.instances.sort-by', defaultMessage: 'Sort by' },
    ascAlphabetical: {
        id: 'app.instances.sort.direction.asc-alphabetical',
        defaultMessage: 'A–Z',
    },
    descAlphabetical: {
        id: 'app.instances.sort.direction.desc-alphabetical',
        defaultMessage: 'Z–A',
    },
    ascVersion: {
        id: 'app.instances.sort.direction.asc-version',
        defaultMessage: 'Oldest version first',
    },
    descVersion: {
        id: 'app.instances.sort.direction.desc-version',
        defaultMessage: 'Newest version first',
    },
    ascRecency: {
        id: 'app.instances.sort.direction.asc-recency',
        defaultMessage: 'Least recent first',
    },
    descRecency: {
        id: 'app.instances.sort.direction.desc-recency',
        defaultMessage: 'Most recent first',
    },
    ascDate: { id: 'app.instances.sort.direction.asc-date', defaultMessage: 'Oldest first' },
    descDate: { id: 'app.instances.sort.direction.desc-date', defaultMessage: 'Newest first' },
    newGroupFromSelection: {
        id: 'app.library.selection.new-group',
        defaultMessage: 'New group',
    },
    removeFromGroup: {
        id: 'app.library.selection.remove-from-group',
        defaultMessage: 'Remove from group',
    },
    addToPinned: {
        id: 'app.library.context-menu.pin-instance',
        defaultMessage: 'Pin in Library',
    },
    removeFromPinned: {
        id: 'app.library.context-menu.unpin-instance',
        defaultMessage: 'Unpin from Library',
    },
    removeFromContextMenu: {
        id: 'app.library.context-menu.remove-from-group',
        defaultMessage: 'Remove from group',
    },

    // components/ui/library/InstanceGroup.vue
    ungrouped: { id: 'app.instances.group.ungrouped', defaultMessage: 'No group' },
    pinned: { id: 'app.instances.group.pinned', defaultMessage: 'Pinned' },
    emptyGroup: {
        id: 'app.instances.group.empty',
        defaultMessage: 'Drag and drop to add instances.',
    },
    editGroupName: { id: 'app.instances.group.edit-name', defaultMessage: 'Edit group name' },
    deleteGroup: { id: 'app.instances.group.delete-group', defaultMessage: 'Delete group' },
    deleteGroupDescription: {
        id: 'app.instances.group.delete-description',
        defaultMessage: 'Instances in this group will be ungrouped.',
    },
    moveGroupUp: { id: 'app.instances.group.move-up', defaultMessage: 'Move group up' },
    moveGroupDown: { id: 'app.instances.group.move-down', defaultMessage: 'Move group down' },
    addToGroup: { id: 'app.instances.group.add-to-group', defaultMessage: 'Add to group' },
    newInstance: { id: 'app.instances.group.new-instance', defaultMessage: 'New instance' },
    collapseGroup: { id: 'app.instances.group.collapse', defaultMessage: 'Collapse group' },
    expandGroup: { id: 'app.instances.group.expand', defaultMessage: 'Expand group' },

    // components/ui/Instance.vue
    loading: { id: 'app.instance.loading', defaultMessage: 'Instance is loading...' },
    played: { id: 'app.instance.played', defaultMessage: 'Played {time}' },
    neverPlayed: { id: 'app.instance.never-played', defaultMessage: 'Never played' },

    // components/ui/modal/ConfirmDeleteInstanceModal.vue
    deleteHeader: { id: 'app.instance.confirm-delete.header', defaultMessage: 'Delete instance' },
    deleteBatchHeader: {
        id: 'app.instance.confirm-delete.batch-header',
        defaultMessage: 'Delete instances',
    },
    deleteAdmonitionHeader: {
        id: 'app.instance.confirm-delete.admonition-header',
        defaultMessage: 'This action cannot be undone',
    },
    deleteAdmonitionBody: {
        id: 'app.instance.confirm-delete.admonition-body',
        defaultMessage:
            'All data for your instance will be permanently deleted, including your worlds, configs, and all installed content.',
    },
    deleteBatchAdmonitionBody: {
        id: 'app.instance.confirm-delete.batch-admonition-body',
        defaultMessage:
            '{count, plural, one {# instance} other {# instances}} will be permanently deleted, including worlds, configs, and all installed content.',
    },
    deleteSymlinkWarning: {
        id: 'app.instance.confirm-delete.symlink-warning',
        defaultMessage:
            'This is a shared instance linked to "{path}". Only the link will be removed; the original files will not be deleted.',
    },
    deleteButton: {
        id: 'app.instance.confirm-delete.delete-button',
        defaultMessage: 'Delete instance',
    },
    deleteBatchButton: {
        id: 'app.instance.confirm-delete.batch-delete-button',
        defaultMessage: 'Delete {count, plural, one {# instance} other {# instances}}',
    },
    deleteBackupCascadeWarning: {
        id: 'app.instance.confirm-delete.backup-cascade-warning',
        defaultMessage:
            '{count, plural, one {# backup} other {# backups}} ({size} total before deduplication) will also be permanently deleted.',
    },

    // components/ui/modal/InstanceGroupModal.vue
    groupModalHeader: {
        id: 'app.instances.batch-edit-groups.header',
        defaultMessage: 'Create group',
    },
    groupModalAddTitle: {
        id: 'app.instances.batch-edit-groups.add-to-group-title',
        defaultMessage: 'Add instances to "{groupName}"',
    },
    groupModalGroupName: {
        id: 'app.instances.batch-edit-groups.group-name',
        defaultMessage: 'Group name',
    },
    groupModalGroupNamePlaceholder: {
        id: 'app.instances.batch-edit-groups.group-name-placeholder',
        defaultMessage: 'Enter group name',
    },
    groupModalSearch: {
        id: 'app.instances.batch-edit-groups.search-instance',
        defaultMessage: 'Search instances',
    },
    groupModalNoInstances: {
        id: 'app.instances.batch-edit-groups.no-instances-found',
        defaultMessage: 'No instances found',
    },
    groupModalAdd: { id: 'app.instances.batch-edit-groups.add', defaultMessage: 'Add' },
    groupModalAdded: { id: 'app.instances.batch-edit-groups.added', defaultMessage: 'Added' },
    groupModalCancel: { id: 'app.instances.batch-edit-groups.cancel', defaultMessage: 'Cancel' },
    groupModalCreate: {
        id: 'app.instances.batch-edit-groups.create',
        defaultMessage: 'Create group',
    },
    groupModalSave: {
        id: 'app.instances.batch-edit-groups.save-changes',
        defaultMessage: 'Save',
    },
    groupModalLoading: {
        id: 'app.instances.batch-edit-groups.loading',
        defaultMessage: 'Loading instances…',
    },
    groupModalLoadFailed: {
        id: 'app.instances.batch-edit-groups.load-failed',
        defaultMessage: 'Could not load instances. Try again.',
    },
    groupModalRetry: { id: 'app.instances.batch-edit-groups.retry', defaultMessage: 'Retry' },
    groupModalSaveFailed: {
        id: 'app.instances.batch-edit-groups.save-failed',
        defaultMessage: 'Could not save the group. Your changes are kept; try again.',
    },
    groupModalMembershipFailed: {
        id: 'app.instances.batch-edit-groups.membership-failed',
        defaultMessage:
            'The group was created, but its instances could not be saved. Retry to finish saving your changes.',
    },

    // Names of the two groups the preview user starts with (user data in the app).
    sampleGroupAdventure: {
        id: 'launcher-preview.library.sample-group.adventure',
        defaultMessage: 'Adventure',
    },
    sampleGroupPerformance: {
        id: 'launcher-preview.library.sample-group.performance',
        defaultMessage: 'Performance',
    },
})
