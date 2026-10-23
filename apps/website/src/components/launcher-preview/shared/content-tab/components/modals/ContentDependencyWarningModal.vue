<template>
    <NewModal
        ref="modal"
        :header="formatMessage(messages.header)"
        fade="danger"
        max-width="560px"
        :disable-close="busy"
    >
        <div class="flex flex-col gap-6">
            <SymlinkWarningAdmonition :symlink-target="symlinkTarget" />
            <Admonition type="critical" :header="formatMessage(messages.admonitionHeader)">
                <IntlFormatted
                    v-if="visibleItems.length === 1"
                    :message-id="messages.singleAdmonitionBody"
                    :values="{ context: contextLabel }"
                >
                    <template #project>
                        <MinecraftFormattedText
                            :text="
                                visibleItems[0]?.project.title ??
                                formatMessage(commonMessages.unknownLabel)
                            "
                        />
                    </template>
                </IntlFormatted>
                <template v-else>
                    {{ formatMessage(messages.bulkAdmonitionBody, { context: contextLabel }) }}
                </template>
            </Admonition>

            <div v-if="visibleItems.length > 0" class="flex flex-col gap-2">
                <span class="font-semibold text-[var(--color-text-primary)]">{{
                    formatMessage(messages.deletingLabel)
                }}</span>
                <div class="relative">
                    <Transition
                        enter-active-class="transition-all duration-200 ease-out"
                        enter-from-class="opacity-0 max-h-0"
                        enter-to-class="opacity-100 max-h-2"
                        leave-active-class="transition-all duration-200 ease-in"
                        leave-from-class="opacity-100 max-h-2"
                        leave-to-class="opacity-0 max-h-0"
                    >
                        <div
                            v-if="showDeletingTopFade"
                            class="pointer-events-none absolute left-0 right-0 top-0 z-10 h-2 bg-gradient-to-b from-bg-raised to-transparent"
                        />
                    </Transition>
                    <div
                        ref="deletingListRef"
                        class="flex max-h-[212px] flex-col gap-2 overflow-y-auto"
                        @scroll="checkDeletingScrollState"
                    >
                        <div
                            v-for="item in visibleItems"
                            :key="item.id"
                            :class="modalContentCardClasses"
                        >
                            <ContentCardItem
                                :project="item.project"
                                :project-link="item.projectLink"
                                :version="item.version"
                                :version-link="item.versionLink"
                                :owner="item.owner"
                                hide-actions
                                inline
                            />
                        </div>
                    </div>
                    <Transition
                        enter-active-class="transition-all duration-200 ease-out"
                        enter-from-class="opacity-0 max-h-0"
                        enter-to-class="opacity-100 max-h-2"
                        leave-active-class="transition-all duration-200 ease-in"
                        leave-from-class="opacity-100 max-h-2"
                        leave-to-class="opacity-0 max-h-0"
                    >
                        <div
                            v-if="showDeletingBottomFade"
                            class="pointer-events-none absolute bottom-0 left-0 right-0 z-10 h-2 bg-gradient-to-t from-bg-raised to-transparent"
                        />
                    </Transition>
                </div>
            </div>

            <div v-if="visibleDependents.length > 0" class="flex flex-col gap-2">
                <span class="font-semibold text-[var(--color-text-primary)]">{{
                    formatMessage(messages.affectedDependentsLabel, {
                        count: visibleDependents.length,
                    })
                }}</span>
                <div class="relative">
                    <Transition
                        enter-active-class="transition-all duration-200 ease-out"
                        enter-from-class="opacity-0 max-h-0"
                        enter-to-class="opacity-100 max-h-2"
                        leave-active-class="transition-all duration-200 ease-in"
                        leave-from-class="opacity-100 max-h-2"
                        leave-to-class="opacity-0 max-h-0"
                    >
                        <div
                            v-if="showDependentTopFade"
                            class="pointer-events-none absolute left-0 right-0 top-0 z-10 h-2 bg-gradient-to-b from-bg-raised to-transparent"
                        />
                    </Transition>
                    <div
                        ref="dependentListRef"
                        class="flex max-h-[212px] flex-col gap-2 overflow-y-auto"
                        @scroll="checkDependentScrollState"
                    >
                        <div
                            v-for="dependent in visibleDependents"
                            :key="dependent.item.id"
                            :class="modalContentCardClasses"
                        >
                            <ContentCardItem
                                :project="dependent.item.project"
                                :project-link="dependent.item.projectLink"
                                :version="dependent.item.version"
                                :version-link="dependent.item.versionLink"
                                :owner="dependent.item.owner"
                                hide-actions
                                inline
                            >
                                <template #title-badges>
                                    <span class="flex min-w-0 flex-wrap items-center gap-1">
                                        <span
                                            v-for="dependency in dependent.dependencies"
                                            :key="dependency.id"
                                            v-tooltip="{
                                                content: autoToHTML(dependency.project.title),
                                                html: true,
                                            }"
                                        >
                                            <span
                                                class="mr-0.5 truncate text-xs text-[var(--color-text-tertiary)]"
                                            >
                                                (<MinecraftFormattedText
                                                    :text="dependency.project.title"
                                                />)
                                            </span>
                                        </span>
                                    </span>
                                </template>
                            </ContentCardItem>
                        </div>
                    </div>
                    <Transition
                        enter-active-class="transition-all duration-200 ease-out"
                        enter-from-class="opacity-0 max-h-0"
                        enter-to-class="opacity-100 max-h-2"
                        leave-active-class="transition-all duration-200 ease-in"
                        leave-from-class="opacity-100 max-h-2"
                        leave-to-class="opacity-0 max-h-0"
                    >
                        <div
                            v-if="showDependentBottomFade"
                            class="pointer-events-none absolute bottom-0 left-0 right-0 z-10 h-2 bg-gradient-to-t from-bg-raised to-transparent"
                        />
                    </Transition>
                </div>
            </div>

            <div class="flex flex-col gap-4">
                <div class="flex flex-col gap-2">
                    <span class="font-semibold text-[var(--color-text-primary)]">{{
                        formatMessage(messages.whatHappensLabel)
                    }}</span>
                    <ul class="m-0 list-disc pl-6 text-[var(--color-text-default)]">
                        <li class="leading-6 marker:text-[var(--color-text-tertiary)]">
                            {{ formatMessage(messages.effectDependentContent) }}
                        </li>
                        <li class="leading-6 marker:text-[var(--color-text-tertiary)]">
                            {{ formatMessage(messages.effectInstance, { context: contextLabel }) }}
                        </li>
                    </ul>
                    <Checkbox
                        v-model="disableDependentsAfterDeleting"
                        :label="formatMessage(messages.disableDependentsLabel)"
                        label-class="font-medium text-[var(--color-text-default)]"
                        class="mt-1"
                    />
                </div>
            </div>
        </div>

        <template #actions>
            <p v-if="error" class="m-0 text-red">{{ error }}</p>
            <div class="flex justify-end gap-2">
                <Button
                    type="outlined"
                    :disabled="busy"
                    class="!border !border-surface-5"
                    @click="hide"
                    ><XIcon aria-hidden="true" />
                    {{ formatMessage(commonMessages.cancelButton) }}
                </Button>
                <Button
                    v-tooltip="props.actionDisabled ? props.actionDisabledTooltip : undefined"
                    type="colored"
                    color="red"
                    :disabled="props.actionDisabled || busy"
                    :loading="busy"
                    @click="confirm"
                    ><TrashIcon aria-hidden="true" />
                    {{ deleteButtonLabel }}
                </Button>
            </div>
        </template>
    </NewModal>
</template>

<script setup lang="ts">
import TrashIcon from '@modrinth/assets/icons/trash.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Admonition from '@modrinth/ui/src/components/base/Admonition.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Checkbox from '@modrinth/ui/src/components/base/Checkbox.vue'
import IntlFormatted from '@modrinth/ui/src/components/base/IntlFormatted.vue'
import MinecraftFormattedText from '@modrinth/ui/src/components/base/MinecraftFormattedText.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { useScrollIndicator } from '@modrinth/ui/src/composables/scroll-indicator.ts'
import SymlinkWarningAdmonition from '@modrinth/ui/src/layouts/shared/content-tab/components/modals/SymlinkWarningAdmonition.vue'
import type { ContentCardTableItem } from '@modrinth/ui/src/layouts/shared/content-tab/types.ts'
import {
    commonMessages,
    formatContentTypeSentence,
} from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, nextTick, ref } from 'vue'

import { autoToHTML } from '../../../../lib/motd-parser'
import NewModal from '../../../../overlays/PreviewModal.vue'
import ContentCardItem from '../ContentCardItem.vue'

export interface ContentDependencyWarningDependent {
    item: ContentCardTableItem
    dependencies: ContentCardTableItem[]
}

const props = withDefaults(
    defineProps<{
        items?: ContentCardTableItem[]
        dependents?: ContentDependencyWarningDependent[]
        itemType: string
        variant?: 'instance' | 'server'
        backupTip?: string
        actionDisabled?: boolean
        actionDisabledTooltip?: string
        symlinkTarget?: string
        action?: (disableDependents: boolean) => Promise<void>
    }>(),
    {
        items: () => [],
        dependents: () => [],
        variant: 'instance',
        backupTip: undefined,
        actionDisabled: false,
        actionDisabledTooltip: undefined,
        symlinkTarget: undefined,
    },
)

const emit = defineEmits<{
    (e: 'delete', disableDependentsAfterDeleting: boolean): void
}>()

const { formatMessage } = useVIntl()
const busy = ref(false)
const error = ref<string | null>(null)

const messages = defineMessages({
    failed: {
        id: 'content.dependency-warning.delete-failed',
        defaultMessage: 'The selected content and dependencies could not be deleted. Try again.',
    },
    header: {
        id: 'content.dependency-warning.header',
        defaultMessage: 'Dependency warning',
    },
    admonitionHeader: {
        id: 'content.dependency-warning.admonition-header',
        defaultMessage: 'This content is required by other content',
    },
    singleAdmonitionBody: {
        id: 'content.dependency-warning.single-admonition-body',
        defaultMessage:
            '{project} is installed as a dependency. Deleting it may break your {context} or stop dependent content from loading correctly.',
    },
    bulkAdmonitionBody: {
        id: 'content.dependency-warning.bulk-admonition-body',
        defaultMessage:
            'Some selected projects are installed as dependencies. Deleting them may break your {context} or stop dependent content from loading correctly.',
    },
    deletingLabel: {
        id: 'content.dependency-warning.deleting-label',
        defaultMessage: 'Deleting',
    },
    affectedDependentsLabel: {
        id: 'content.dependency-warning.affected-dependents-label',
        defaultMessage: 'Affected {count, plural, one {project} other {projects}}',
    },
    whatHappensLabel: {
        id: 'content.dependency-warning.what-happens-label',
        defaultMessage: 'What happens?',
    },
    effectDependentContent: {
        id: 'content.dependency-warning.effect-dependent-content',
        defaultMessage: 'Dependent content may fail to load or may disable itself',
    },
    effectInstance: {
        id: 'content.dependency-warning.effect-instance',
        defaultMessage: 'Your {context} may crash, refuse to start, or behave unexpectedly',
    },
    deleteAnywayButton: {
        id: 'content.dependency-warning.delete-anyway-button',
        defaultMessage: 'Delete anyway',
    },
    deleteManyAnywayButton: {
        id: 'content.dependency-warning.delete-many-anyway-button',
        defaultMessage: 'Delete {count, number} {itemType} anyway',
    },
    disableDependentsLabel: {
        id: 'content.dependency-warning.disable-dependents-label',
        defaultMessage: 'Disable dependents after deleting',
    },
    instanceContext: {
        id: 'content.dependency-warning.context.instance',
        defaultMessage: 'instance',
    },
    serverContext: {
        id: 'content.dependency-warning.context.server',
        defaultMessage: 'server',
    },
})

const modal = ref<InstanceType<typeof NewModal>>()
const deletingListRef = ref<HTMLElement | null>(null)
const dependentListRef = ref<HTMLElement | null>(null)
const visibleItems = ref<ContentCardTableItem[]>(props.items)
const visibleDependents = ref<ContentDependencyWarningDependent[]>(props.dependents)
const visibleItemType = ref(props.itemType)
const disableDependentsAfterDeleting = ref(false)
const modalContentCardClasses = 'rounded-xl border border-solid border-surface-5 p-4 !bg-surface-2'
const {
    showTopFade: showDeletingTopFade,
    showBottomFade: showDeletingBottomFade,
    checkScrollState: checkDeletingScrollState,
    forceCheck: forceCheckDeletingScroll,
} = useScrollIndicator(deletingListRef)
const {
    showTopFade: showDependentTopFade,
    showBottomFade: showDependentBottomFade,
    checkScrollState: checkDependentScrollState,
    forceCheck: forceCheckDependentScroll,
} = useScrollIndicator(dependentListRef)

const contextLabel = computed(() =>
    formatMessage(props.variant === 'server' ? messages.serverContext : messages.instanceContext),
)

const deleteButtonLabel = computed(() => {
    if (visibleItems.value.length <= 1) return formatMessage(messages.deleteAnywayButton)

    return formatMessage(messages.deleteManyAnywayButton, {
        count: visibleItems.value.length,
        itemType: formatContentTypeSentence(
            formatMessage,
            visibleItemType.value,
            visibleItems.value.length,
        ),
    })
})

async function show() {
    error.value = null
    await nextTick()
    visibleItems.value = props.items
    visibleDependents.value = props.dependents
    visibleItemType.value = props.itemType
    disableDependentsAfterDeleting.value = false
    modal.value?.show()
    await nextTick()
    forceCheckDeletingScroll()
    forceCheckDependentScroll()
}

function hide() {
    modal.value?.hide()
}

async function confirm() {
    if (props.actionDisabled || busy.value) return
    busy.value = true
    error.value = null
    try {
        if (props.action) await props.action(disableDependentsAfterDeleting.value)
        else emit('delete', disableDependentsAfterDeleting.value)
        busy.value = false
        await nextTick()
        await modal.value?.hide()
    } catch (failure) {
        console.error('Dependency deletion failed', failure)
        error.value = formatMessage(messages.failed)
    } finally {
        busy.value = false
    }
}

defineExpose({
    show,
    hide,
})
</script>
