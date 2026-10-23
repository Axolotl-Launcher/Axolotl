<script setup lang="ts">
import ArrowLeftRightIcon from '@modrinth/assets/icons/arrow-left-right.svg?component'
import ChevronDownIcon from '@modrinth/assets/icons/chevron-down.svg?component'
import ChevronRightIcon from '@modrinth/assets/icons/chevron-right.svg?component'
import ClockIcon from '@modrinth/assets/icons/clock.svg?component'
import DownloadIcon from '@modrinth/assets/icons/download.svg?component'
import MoreVerticalIcon from '@modrinth/assets/icons/more-vertical.svg?component'
import SkullIcon from '@modrinth/assets/icons/skull.svg?component'
import SpinnerIcon from '@modrinth/assets/icons/spinner.svg?component'
import TrashIcon from '@modrinth/assets/icons/trash.svg?component'
import TrashExclamationIcon from '@modrinth/assets/icons/trash-exclamation.svg?component'
import TriangleAlertIcon from '@modrinth/assets/icons/triangle-alert.svg?component'
import UndoIcon from '@modrinth/assets/icons/undo.svg?component'
import UserIcon from '@modrinth/assets/icons/user.svg?component'
import AutoLink from '@modrinth/ui/src/components/base/AutoLink.vue'
import Avatar from '@modrinth/ui/src/components/base/Avatar.vue'
import BulletDivider from '@modrinth/ui/src/components/base/BulletDivider.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import IconButton from '@modrinth/ui/src/components/base/buttons/IconButton.vue'
import Checkbox from '@modrinth/ui/src/components/base/Checkbox.vue'
import MinecraftFormattedText from '@modrinth/ui/src/components/base/MinecraftFormattedText.vue'
import type { Option as OverflowMenuOption } from '@modrinth/ui/src/components/base/OverflowMenu.vue'
import Toggle from '@modrinth/ui/src/components/base/Toggle.vue'
import { useRelativeTime } from '@modrinth/ui/src/composables/how-ago.ts'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import type {
    ContentCardProject,
    ContentCardVersion,
    ContentOwner,
    ContentRowInlineAction,
    ContentWorldGroupMeta,
} from '@modrinth/ui/src/layouts/shared/content-tab/types.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, getCurrentInstance, ref } from 'vue'
import type { RouteLocationRaw } from 'vue-router'

import { autoCleanToText } from '../../../lib/motd-parser'
import { useMagicKeys } from '../../../lib/vueuse'
import TeleportOverflowMenu from '../../../overlays/PreviewTeleportOverflowMenu.vue'

const { formatMessage } = useVIntl()

const messages = defineMessages({
    selectProject: {
        id: 'content.card.select-project',
        defaultMessage: 'Select {project}',
    },
    pendingManualDownload: {
        id: 'content.card.pending-manual-download',
        defaultMessage: 'Manual download required',
    },
    duplicateMod: {
        id: 'content.card.duplicate-mod',
        defaultMessage: 'This mod is installed {count, number} times.',
    },
    rollbackTooltip: {
        id: 'content.card.rollback-tooltip',
        defaultMessage: 'Roll back to {fileName}',
    },
    dependencyBadge: {
        id: 'content.card.dependency-badge',
        defaultMessage: 'Dependency',
    },
    orphanedDependencyBadge: {
        id: 'content.card.orphaned-dependency-badge',
        defaultMessage: 'Orphaned dependency',
    },
    notPlayedYet: {
        id: 'content.card.group.not-played-yet',
        defaultMessage: 'Not played yet',
    },
    hardcore: {
        id: 'content.card.group.hardcore',
        defaultMessage: 'Hardcore mode',
    },
})

interface Props {
    project: ContentCardProject
    projectLink?: string | RouteLocationRaw
    version?: ContentCardVersion
    versionLink?: string | RouteLocationRaw
    owner?: ContentOwner
    enabled?: boolean
    installing?: boolean
    pendingManualDownload?: boolean
    duplicateCount?: number
    localPath?: string
    installationSource?: string
    groupStatus?: string
    updateLabel?: string
    hasUpdate?: boolean
    rollbackFileName?: string
    hideSwitchVersion?: boolean
    overflowOptions?: OverflowMenuOption[]
    inlineActions?: ContentRowInlineAction[]
    disabled?: boolean
    disabledTooltip?: string | null
    postUpgradeWarningTooltip?: string | null
    compatibilityWarningTooltip?: string | null
    toggleDisabled?: boolean
    toggleDisabledTooltip?: string | null
    dependencyBadge?: {
        autoDependency: boolean
        orphaned: boolean
    } | null
    showCheckbox?: boolean
    hideDelete?: boolean
    hideActions?: boolean
    inline?: boolean
    isGroupHeader?: boolean
    groupDepth?: number
    groupItemCount?: number
    groupExpanded?: boolean
    groupSwitchVersion?: () => void
    isGroupChild?: boolean
    groupKind?: 'folder' | 'world' | 'duplicate'
    groupMeta?: ContentWorldGroupMeta
    groupCheckboxIndeterminate?: boolean
    downloads?: number | null
    categories?: Array<{
        name: string
        icon?: string
        action?: (event: MouseEvent) => void
    }>
}

const props = withDefaults(defineProps<Props>(), {
    projectLink: undefined,
    version: undefined,
    versionLink: undefined,
    owner: undefined,
    enabled: undefined,
    installing: false,
    pendingManualDownload: false,
    duplicateCount: undefined,
    hasUpdate: false,
    rollbackFileName: undefined,
    hideSwitchVersion: false,
    overflowOptions: undefined,
    inlineActions: undefined,
    disabled: false,
    disabledTooltip: undefined,
    postUpgradeWarningTooltip: undefined,
    compatibilityWarningTooltip: undefined,
    toggleDisabled: false,
    toggleDisabledTooltip: undefined,
    dependencyBadge: null,
    showCheckbox: false,
    hideDelete: false,
    hideActions: false,
    inline: false,
    isGroupHeader: false,
    groupDepth: 0,
    groupItemCount: 0,
    groupExpanded: false,
    groupSwitchVersion: undefined,
    isGroupChild: false,
    groupKind: 'folder',
    groupMeta: undefined,
    groupCheckboxIndeterminate: false,
    downloads: null,
    categories: undefined,
})

const selected = defineModel<boolean>('selected')

const emit = defineEmits<{
    'update:enabled': [value: boolean]
    select: [value: boolean, event?: MouseEvent]
    delete: [event: MouseEvent]
    update: []
    switchVersion: []
    rollback: []
    toggleExpand: []
}>()

const instance = getCurrentInstance()
const hasDeleteListener = computed(() => typeof instance?.vnode.props?.onDelete === 'function')
const hasUpdateListener = computed(() => typeof instance?.vnode.props?.onUpdate === 'function')
const hasRollbackListener = computed(() => typeof instance?.vnode.props?.onRollback === 'function')
const hasSwitchVersionListener = computed(
    () => typeof instance?.vnode.props?.onSwitchVersion === 'function',
)

const formatCompact = (n: number | undefined | null) => {
    if (n == null) return ''
    return new Intl.NumberFormat('en', { notation: 'compact', maximumFractionDigits: 2 }).format(n)
}

const formatTimeAgo = useRelativeTime()

const versionNumberRef = ref<HTMLElement | null>(null)
const fileNameRef = ref<HTMLElement | null>(null)

const isDisabled = computed(() => props.disabled || props.installing)
const isToggleDisabled = computed(() => isDisabled.value || props.toggleDisabled)
const plainProjectTitle = computed(() => autoCleanToText(props.project.title))

const interactiveSelectors = 'a, button, input, select, [role="checkbox"], [role="button"]'

function handleRowClick(event: MouseEvent) {
    const target = event.target as HTMLElement
    if (target.closest(interactiveSelectors)) return
    emit('toggleExpand')
}

const { shift: shiftHeld } = useMagicKeys()
const deleteHovered = ref(false)
</script>

<template>
    <div
        v-if="isGroupHeader"
        role="row"
        class="flex h-[74px] cursor-pointer items-center justify-between gap-4"
        :class="[
            { 'opacity-50': disabled },
            groupKind === 'world'
                ? selected
                    ? 'card-shadow rounded-lg !bg-surface-content-selected p-3'
                    : 'card-shadow rounded-lg !bg-surface-content p-3 hover:!bg-surface-content'
                : 'rounded-lg bg-surface-3 px-3',
        ]"
        :style="
            groupDepth && groupKind !== 'world'
                ? { paddingLeft: `${groupDepth * 2.5}rem` }
                : undefined
        "
        @click="handleRowClick"
    >
        <div
            class="flex min-w-0 items-center gap-4"
            :class="
                hideActions
                    ? 'flex-1'
                    : 'flex-1 @[800px]:w-[45%] @[800px]:flex-none @[800px]:shrink-0'
            "
        >
            <Checkbox
                v-if="showCheckbox"
                :model-value="selected ?? false"
                :indeterminate="groupCheckboxIndeterminate"
                :aria-label="formatMessage(messages.selectProject, { project: plainProjectTitle })"
                class="shrink-0"
                @update:model-value="(value, event) => emit('select', value, event)"
            />

            <div class="flex min-w-0 items-center gap-3">
                <Avatar
                    :src="project.icon_url"
                    :alt="plainProjectTitle"
                    size="3rem"
                    no-shadow
                    class="rounded-2xl border border-surface-5"
                />
                <div class="flex min-w-0 flex-col gap-0.5">
                    <div class="flex min-w-0 items-center gap-1">
                        <TriangleAlertIcon
                            v-if="postUpgradeWarningTooltip || compatibilityWarningTooltip"
                            v-tooltip="
                                [postUpgradeWarningTooltip, compatibilityWarningTooltip]
                                    .filter(Boolean)
                                    .join('\n')
                            "
                            class="size-4 shrink-0 text-brand-orange"
                            aria-hidden="true"
                        />
                        <AutoLink
                            :target="
                                typeof projectLink === 'string' && projectLink.startsWith('http')
                                    ? '_blank'
                                    : undefined
                            "
                            :to="projectLink"
                            class="truncate text-[var(--color-text-primary)] !decoration-[var(--color-text-primary)]"
                            :class="[
                                groupKind === 'world'
                                    ? 'text-lg font-bold'
                                    : 'font-semibold leading-6',
                                { 'hover:underline': projectLink },
                            ]"
                        >
                            <MinecraftFormattedText :text="project.title" />
                        </AutoLink>
                        <span
                            v-if="groupKind === 'world'"
                            class="flex items-center gap-1 whitespace-nowrap text-sm font-semibold text-[var(--color-text-tertiary)]"
                        >
                            <UserIcon
                                aria-hidden="true"
                                class="h-4 w-4 shrink-0 text-[var(--color-text-tertiary)]"
                                stroke-width="3px"
                            />
                            {{ formatMessage(commonMessages.singleplayerLabel) }}
                        </span>
                        <span
                            class="shrink-0 text-sm font-medium text-[var(--color-text-tertiary)]"
                        >
                            ({{ groupItemCount }})
                        </span>
                    </div>
                    <div class="flex min-w-0 items-center gap-1">
                        <span
                            v-if="groupKind === 'duplicate'"
                            :title="groupStatus"
                            class="line-clamp-2 text-sm text-[var(--color-text-tertiary)]"
                            >{{ groupStatus }}</span
                        >
                        <template v-else-if="groupKind === 'world'">
                            <template v-if="groupMeta?.last_played">
                                <ClockIcon
                                    class="size-4 shrink-0 text-[var(--color-text-tertiary)]"
                                />
                                <span
                                    class="truncate text-sm leading-5 text-[var(--color-text-tertiary)]"
                                >
                                    {{
                                        formatMessage(commonMessages.playedLabel, {
                                            ago: formatTimeAgo(new Date(groupMeta.last_played)),
                                        })
                                    }}
                                </span>
                            </template>
                            <span
                                v-else
                                class="truncate text-sm leading-5 text-[var(--color-text-tertiary)]"
                            >
                                {{ formatMessage(messages.notPlayedYet) }}
                            </span>
                        </template>
                        <template v-else>
                            <AutoLink
                                v-if="owner"
                                :target="
                                    typeof owner.link === 'string' && owner.link.startsWith('http')
                                        ? '_blank'
                                        : undefined
                                "
                                :to="owner.link"
                                class="flex shrink-0 items-center gap-1 !decoration-[var(--color-text-tertiary)]"
                                :class="{ 'hover:underline': owner.link }"
                            >
                                <Avatar
                                    :src="owner.avatar_url"
                                    :alt="owner.name"
                                    size="1.5rem"
                                    :circle="owner.type === 'user'"
                                    no-shadow
                                    class="shrink-0"
                                />
                                <span class="text-sm leading-5 text-[var(--color-text-tertiary)]">{{
                                    owner.name
                                }}</span>
                            </AutoLink>
                            <template v-if="version">
                                <BulletDivider class="shrink-0 @[800px]:hidden" />
                                <AutoLink
                                    :target="
                                        typeof versionLink === 'string' &&
                                        versionLink.startsWith('http')
                                            ? '_blank'
                                            : undefined
                                    "
                                    :to="versionLink"
                                    class="truncate text-sm leading-5 text-[var(--color-text-tertiary)] !decoration-[var(--color-text-tertiary)] @[800px]:hidden"
                                    :class="{ 'hover:underline': versionLink }"
                                >
                                    {{ version.version_number }}
                                </AutoLink>
                                <template v-if="version.date_published">
                                    <BulletDivider class="shrink-0 @[800px]:hidden" />
                                    <ClockIcon
                                        class="size-4 shrink-0 text-[var(--color-text-tertiary)] @[800px]:hidden"
                                    />
                                    <span
                                        class="shrink-0 text-sm leading-5 text-[var(--color-text-tertiary)] @[800px]:hidden"
                                    >
                                        {{ formatTimeAgo(new Date(version.date_published)) }}
                                    </span>
                                </template>
                            </template>
                        </template>
                    </div>
                </div>
            </div>
        </div>

        <div
            class="hidden flex-col gap-0.5 @[800px]:flex"
            :class="hideActions ? 'flex-1' : 'min-w-0 flex-1'"
        >
            <template v-if="groupKind === 'world'">
                <div
                    class="flex min-w-0 items-center gap-1.5 font-medium leading-6 text-[var(--color-text-primary)]"
                >
                    <template v-if="groupMeta?.hardcore">
                        <SkullIcon aria-hidden="true" class="h-4 w-4 shrink-0 text-red" />
                        <span class="text-red">{{ formatMessage(messages.hardcore) }}</span>
                    </template>
                    <span
                        v-else-if="groupMeta?.game_mode"
                        class="text-[var(--color-text-tertiary)]"
                    >
                        {{
                            groupMeta.game_mode.charAt(0).toUpperCase() +
                            groupMeta.game_mode.slice(1)
                        }}
                    </span>
                </div>
            </template>
            <template v-else-if="version">
                <div
                    class="flex min-w-0 items-center gap-1.5 font-medium leading-6 text-[var(--color-text-primary)]"
                >
                    <AutoLink
                        :target="
                            typeof versionLink === 'string' && versionLink.startsWith('http')
                                ? '_blank'
                                : undefined
                        "
                        :to="versionLink"
                        class="self-start truncate !decoration-[var(--color-text-primary)]"
                        :class="{ 'hover:underline': versionLink, 'cursor-pointer': versionLink }"
                    >
                        {{ version.version_number }}
                    </AutoLink>
                    <template v-if="version.date_published">
                        <ClockIcon
                            class="hidden size-4 shrink-0 text-[var(--color-text-tertiary)] @[600px]:inline"
                        />
                        <span
                            class="hidden shrink-0 text-sm font-normal leading-6 text-[var(--color-text-tertiary)] @[600px]:inline"
                        >
                            {{ formatTimeAgo(new Date(version.date_published)) }}
                        </span>
                    </template>
                </div>
                <span class="flex min-w-0 leading-6 text-[var(--color-text-tertiary)]">
                    <span class="truncate">{{ version.file_name }}</span>
                </span>
            </template>
            <div
                v-if="downloads != null"
                class="flex flex-nowrap items-center gap-3 overflow-hidden"
            >
                <div
                    v-if="downloads != null"
                    class="flex items-center gap-2 text-[var(--color-text-tertiary)]"
                >
                    <DownloadIcon class="size-4" />
                    <span class="text-sm font-medium">{{ formatCompact(downloads) }}</span>
                </div>
            </div>
        </div>

        <div v-if="!hideActions" class="flex min-w-[160px] shrink-0 items-center justify-end gap-2">
            <template v-if="groupKind !== 'world'">
                <IconButton
                    v-if="hasUpdate"
                    v-tooltip="
                        isDisabled && disabledTooltip
                            ? disabledTooltip
                            : (updateLabel ?? formatMessage(commonMessages.updateAvailableLabel))
                    "
                    :label="updateLabel ?? formatMessage(commonMessages.updateAvailableLabel)"
                    type="quiet"
                    color="green"
                    interaction="filled"
                    :disabled="isDisabled"
                    @click.stop="emit('update')"
                >
                    <DownloadIcon class="size-5" />
                </IconButton>
                <Button
                    v-else-if="groupSwitchVersion"
                    v-tooltip="formatMessage(commonMessages.switchVersionButton)"
                    type="quiet"
                    circular
                    icon-only
                    @click.stop="groupSwitchVersion"
                    ><ArrowLeftRightIcon class="size-5" />
                </Button>
            </template>
            <Button
                type="quiet"
                circular
                icon-only
                class="flex items-center text-[var(--color-text-tertiary)] transition-colors hover:text-[var(--color-text-default)]"
                @click.stop="emit('toggleExpand')"
                ><ChevronDownIcon v-if="groupExpanded" class="size-5" />
                <ChevronRightIcon v-else class="size-5" />
            </Button>
        </div>
    </div>

    <div
        v-else
        role="row"
        class="flex items-center justify-between"
        :class="{
            'h-[74px] gap-4 px-3': !inline,
            'gap-3': inline,
            'opacity-50 grayscale': disabled && !installing,
            'opacity-50': installing,
            'pl-10': isGroupChild && !inline,
        }"
        :style="
            isGroupChild && !inline && groupDepth > 1
                ? { paddingLeft: `${groupDepth * 2.5}rem` }
                : undefined
        "
    >
        <div
            class="flex min-w-0 items-center gap-4"
            :class="
                hideActions
                    ? 'flex-1'
                    : 'flex-1 @[800px]:w-[45%] @[800px]:flex-none @[800px]:shrink-0'
            "
        >
            <Checkbox
                v-if="showCheckbox"
                :model-value="selected ?? false"
                :aria-label="formatMessage(messages.selectProject, { project: plainProjectTitle })"
                class="shrink-0"
                @update:model-value="(value, event) => emit('select', value, event)"
            />

            <div
                class="flex min-w-0 items-center gap-3 transition-[filter,opacity] duration-200"
                :class="enabled === false && !disabled ? 'opacity-50 grayscale' : ''"
            >
                <div
                    v-tooltip="
                        installing
                            ? formatMessage(commonMessages.installingLabel)
                            : pendingManualDownload
                              ? formatMessage(messages.pendingManualDownload)
                              : undefined
                    "
                    class="relative flex shrink-0 items-center"
                >
                    <Avatar
                        :src="project.icon_url"
                        :alt="plainProjectTitle"
                        size="3rem"
                        no-shadow
                        class="rounded-2xl border border-surface-5"
                    />
                    <div
                        v-if="installing"
                        class="absolute inset-0 flex items-center justify-center rounded-2xl bg-black/20"
                    >
                        <SpinnerIcon class="size-5 animate-spin text-white" />
                    </div>
                    <div
                        v-else-if="pendingManualDownload"
                        class="absolute -right-1 -top-1 flex size-5 items-center justify-center rounded-full bg-orange text-white"
                    >
                        <TriangleAlertIcon class="size-3.5" />
                    </div>
                </div>
                <div class="flex min-w-0 flex-col gap-0.5">
                    <span
                        v-if="groupKind === 'duplicate'"
                        :title="localPath"
                        class="truncate text-sm text-[var(--color-text-tertiary)]"
                        >{{ installationSource }} · {{ localPath }}</span
                    >
                    <div class="flex min-w-0 items-center gap-1">
                        <AutoLink
                            :target="
                                typeof projectLink === 'string' && projectLink.startsWith('http')
                                    ? '_blank'
                                    : undefined
                            "
                            :to="projectLink"
                            class="truncate font-semibold leading-6 text-[var(--color-text-primary)] !decoration-[var(--color-text-primary)]"
                            :class="{ 'hover:underline': projectLink }"
                        >
                            <MinecraftFormattedText :text="project.title" />
                        </AutoLink>
                        <TriangleAlertIcon
                            v-if="postUpgradeWarningTooltip || compatibilityWarningTooltip"
                            v-tooltip="
                                [postUpgradeWarningTooltip, compatibilityWarningTooltip]
                                    .filter(Boolean)
                                    .join('\n')
                            "
                            class="size-4 shrink-0 text-brand-orange"
                            aria-hidden="true"
                        />
                        <TriangleAlertIcon
                            v-if="duplicateCount && duplicateCount > 1"
                            v-tooltip="
                                formatMessage(messages.duplicateMod, { count: duplicateCount })
                            "
                            class="size-4 shrink-0 text-red"
                            aria-hidden="true"
                        />
                        <span
                            v-if="dependencyBadge"
                            v-tooltip="
                                dependencyBadge.orphaned
                                    ? formatMessage(messages.orphanedDependencyBadge)
                                    : formatMessage(messages.dependencyBadge)
                            "
                            class="shrink-0 rounded-md bg-surface-3 px-1.5 py-0.5 text-xs font-medium leading-4 text-[var(--color-text-tertiary)]"
                        >
                            {{
                                dependencyBadge.orphaned
                                    ? formatMessage(messages.orphanedDependencyBadge)
                                    : formatMessage(messages.dependencyBadge)
                            }}
                        </span>
                        <slot name="title-badges" />
                    </div>

                    <div class="flex min-w-0 items-center gap-1">
                        <span
                            v-if="project.description"
                            class="truncate text-sm leading-5 text-[var(--color-text-tertiary)]"
                            :title="project.description"
                        >
                            {{ project.description }}
                        </span>
                        <AutoLink
                            v-if="owner && !project.description"
                            :target="
                                typeof owner.link === 'string' && owner.link.startsWith('http')
                                    ? '_blank'
                                    : undefined
                            "
                            :to="owner.link"
                            class="flex shrink-0 items-center gap-1 !decoration-[var(--color-text-tertiary)]"
                            :class="{ 'hover:underline': owner.link }"
                        >
                            <Avatar
                                :src="owner.avatar_url"
                                :alt="owner.name"
                                size="1.5rem"
                                :circle="owner.type === 'user'"
                                no-shadow
                                class="shrink-0"
                            />
                            <span class="text-sm leading-5 text-[var(--color-text-tertiary)]">{{
                                owner.name
                            }}</span>
                        </AutoLink>
                        <template v-if="version && !project.description">
                            <BulletDivider class="shrink-0 @[800px]:hidden" />
                            <AutoLink
                                :target="
                                    typeof versionLink === 'string' &&
                                    versionLink.startsWith('http')
                                        ? '_blank'
                                        : undefined
                                "
                                :to="versionLink"
                                class="truncate text-sm leading-5 text-[var(--color-text-tertiary)] !decoration-[var(--color-text-tertiary)] @[800px]:hidden"
                                :class="{ 'hover:underline': versionLink }"
                            >
                                {{ version.version_number }}
                            </AutoLink>
                        </template>
                        <template v-else-if="version && project.description">
                            <BulletDivider class="shrink-0 @[800px]:hidden" />
                            <AutoLink
                                :target="
                                    typeof versionLink === 'string' &&
                                    versionLink.startsWith('http')
                                        ? '_blank'
                                        : undefined
                                "
                                :to="versionLink"
                                class="truncate text-sm leading-5 text-[var(--color-text-tertiary)] !decoration-[var(--color-text-tertiary)] @[800px]:hidden"
                                :class="{ 'hover:underline': versionLink }"
                            >
                                {{ version.version_number }}
                            </AutoLink>
                        </template>
                    </div>
                </div>
            </div>
        </div>

        <div
            class="hidden flex-col gap-0.5 transition-[filter,opacity] duration-200 @[800px]:flex"
            :class="[
                hideActions ? 'flex-1' : 'min-w-0 flex-1',
                enabled === false && !disabled ? 'opacity-50 grayscale' : '',
            ]"
        >
            <template v-if="version">
                <AutoLink
                    v-tooltip="{
                        content: version.version_number,
                        onlyWhenTruncated: true,
                        overflowTarget: versionNumberRef,
                    }"
                    :target="
                        typeof versionLink === 'string' && versionLink.startsWith('http')
                            ? '_blank'
                            : undefined
                    "
                    :to="versionLink"
                    class="inline-flex self-start font-medium leading-6 text-[var(--color-text-primary)] !decoration-[var(--color-text-primary)]"
                    :class="{ 'hover:underline': versionLink, 'cursor-pointer': versionLink }"
                >
                    <span ref="versionNumberRef" class="truncate">{{
                        version.version_number.slice(
                            0,
                            Math.ceil(version.version_number.length / 2),
                        )
                    }}</span
                    ><span class="shrink-0">{{
                        version.version_number.slice(Math.ceil(version.version_number.length / 2))
                    }}</span>
                </AutoLink>
                <span
                    v-tooltip="{
                        content: version.file_name,
                        onlyWhenTruncated: true,
                        overflowTarget: fileNameRef,
                    }"
                    class="flex min-w-0 leading-6 text-[var(--color-text-tertiary)]"
                >
                    <span ref="fileNameRef" class="truncate">{{
                        version.file_name.slice(0, Math.ceil(version.file_name.length / 2))
                    }}</span
                    ><span class="shrink-0">{{
                        version.file_name.slice(Math.ceil(version.file_name.length / 2))
                    }}</span>
                </span>
            </template>
        </div>

        <div
            v-if="!hideActions"
            class="flex min-w-[160px] shrink-0 items-center justify-end gap-2 transition-colors duration-200"
        >
            <slot name="additionalButtonsLeft" />

            <Button
                v-if="hasRollbackListener && rollbackFileName"
                v-tooltip="formatMessage(messages.rollbackTooltip, { fileName: rollbackFileName })"
                type="quiet"
                circular
                icon-only
                :aria-label="
                    formatMessage(messages.rollbackTooltip, { fileName: rollbackFileName })
                "
                :disabled="isDisabled"
                @click="emit('rollback')"
                ><UndoIcon class="size-5" />
            </Button>

            <!-- Fixed width container to reserve space for update/switch version button -->
            <div
                v-if="hasUpdateListener || hasSwitchVersionListener"
                class="flex w-8 items-center justify-center"
            >
                <IconButton
                    v-if="hasUpdate"
                    v-tooltip="
                        isDisabled && disabledTooltip
                            ? disabledTooltip
                            : formatMessage(commonMessages.updateAvailableLabel)
                    "
                    :label="formatMessage(commonMessages.updateAvailableLabel)"
                    type="quiet"
                    color="green"
                    interaction="filled"
                    :disabled="isDisabled"
                    @click.stop="emit('update')"
                >
                    <DownloadIcon class="size-5" />
                </IconButton>
                <Button
                    v-else-if="hasSwitchVersionListener && version && !hideSwitchVersion"
                    v-tooltip="
                        isDisabled && disabledTooltip
                            ? disabledTooltip
                            : formatMessage(commonMessages.switchVersionButton)
                    "
                    type="quiet"
                    circular
                    icon-only
                    :disabled="isDisabled"
                    @click="emit('switchVersion')"
                    ><ArrowLeftRightIcon class="size-5" />
                </Button>
            </div>

            <template v-for="action in inlineActions" :key="action.id">
                <Button
                    v-tooltip="action.label"
                    type="quiet"
                    circular
                    icon-only
                    :aria-label="action.label"
                    :disabled="isDisabled"
                    @click="action.action"
                    ><component :is="action.icon" class="size-5" />
                </Button>
            </template>

            <Toggle
                v-if="enabled !== undefined"
                v-tooltip="
                    isToggleDisabled && (toggleDisabledTooltip || disabledTooltip)
                        ? (toggleDisabledTooltip ?? disabledTooltip)
                        : undefined
                "
                :model-value="enabled"
                :disabled="isToggleDisabled"
                :aria-label="plainProjectTitle"
                class="my-auto"
                @update:model-value="(val) => emit('update:enabled', val as boolean)"
            />

            <Button
                v-if="hasDeleteListener && !props.hideDelete"
                v-tooltip="
                    isDisabled && disabledTooltip
                        ? disabledTooltip
                        : formatMessage(
                              shiftHeld && deleteHovered
                                  ? commonMessages.deleteImmediatelyLabel
                                  : commonMessages.deleteLabel,
                          )
                "
                type="quiet"
                circular
                icon-only
                :disabled="isDisabled"
                @click="emit('delete', $event)"
                @mouseenter="deleteHovered = true"
                @mouseleave="deleteHovered = false"
                ><span class="relative size-5">
                    <TrashIcon
                        class="absolute inset-0 size-5 text-[var(--color-text-tertiary)] transition-opacity duration-200"
                        :class="shiftHeld && deleteHovered ? 'opacity-0' : 'opacity-100'"
                    />
                    <TrashExclamationIcon
                        class="absolute inset-0 size-5 text-red transition-opacity duration-200"
                        :class="shiftHeld && deleteHovered ? 'opacity-100' : 'opacity-0'"
                    />
                </span>
            </Button>

            <slot name="additionalButtonsRight" />

            <TeleportOverflowMenu
                v-if="overflowOptions?.length"
                :options="overflowOptions"
                :disabled="isDisabled"
            >
                <MoreVerticalIcon class="size-5" />
            </TeleportOverflowMenu>
        </div>
    </div>
</template>
