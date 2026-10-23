<script setup lang="ts">
import DropdownIcon from '@modrinth/assets/icons/dropdown.svg?component'
import Accordion from '@modrinth/ui/src/components/base/Accordion.vue'
import InlineEditableText from '@modrinth/ui/src/components/base/InlineEditableText.vue'
import TagItem from '@modrinth/ui/src/components/base/TagItem.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { nextTick, ref, watch } from 'vue'

import { messages } from './messages'

const props = withDefaults(
    defineProps<{
        title: string
        count: number
        collapsed?: boolean
        forceOpen?: boolean
        hideHeader?: boolean
        editable?: boolean
        startEditing?: boolean
        maxTitleLength?: number
        validateTitle?: (value: string) => boolean
        onTitleChange?: (value: string) => boolean | void | Promise<boolean | void>
    }>(),
    {
        collapsed: false,
        forceOpen: false,
        hideHeader: false,
        editable: false,
        startEditing: false,
        maxTitleLength: undefined,
        validateTitle: undefined,
        onTitleChange: undefined,
    },
)

const emit = defineEmits<{
    (e: 'update:collapsed', collapsed: boolean): void
}>()

const { formatMessage } = useVIntl()
const accordion = ref<InstanceType<typeof Accordion>>()
const titleInput = ref<InstanceType<typeof InlineEditableText>>()
const titleModel = ref(props.title)

function toggle() {
    if (accordion.value?.isOpen) {
        accordion.value.close()
    } else {
        accordion.value?.open()
    }
}

function handleOpen() {
    emit('update:collapsed', false)
}

function handleClose() {
    emit('update:collapsed', true)
}

async function startTitleEditing() {
    if (!props.editable) return
    await titleInput.value?.startEditing()
}

watch(
    () => props.title,
    (title) => {
        titleModel.value = title
    },
)

watch(
    () => props.startEditing,
    async (shouldStartEditing) => {
        if (!shouldStartEditing) return
        await nextTick()
        await startTitleEditing()
    },
    { immediate: true, flush: 'post' },
)
</script>

<template>
    <section class="flex w-full flex-col">
        <div
            v-if="!hideHeader"
            data-screenshot-group-header
            class="group/header flex h-10 w-full items-center gap-2 border-0 border-b border-solid border-b-surface-5"
        >
            <div
                class="group/open-target flex min-w-0 cursor-pointer items-center gap-2"
                @click="toggle"
            >
                <button
                    type="button"
                    class="flex shrink-0 cursor-pointer items-center border-0 bg-transparent p-0"
                    :aria-expanded="accordion?.isOpen"
                    :aria-label="
                        formatMessage(accordion?.isOpen ? messages.collapse : messages.expand)
                    "
                    @click.stop="toggle"
                >
                    <DropdownIcon
                        class="size-5 shrink-0 text-[var(--color-text-tertiary)] transition-all duration-300 group-hover/open-target:text-[var(--color-text-default)]"
                        :class="{ 'rotate-180': accordion?.isOpen }"
                    />
                </button>
                <InlineEditableText
                    v-if="editable"
                    ref="titleInput"
                    v-model="titleModel"
                    activation-mode="manual"
                    class="!h-10 select-none text-base font-semibold text-[var(--color-text-default)] group-hover/open-target:text-[var(--color-text-primary)]"
                    :edit-label="formatMessage(commonMessages.renameButton)"
                    max-width="24rem"
                    icon-text-class="select-none"
                    :max-length="maxTitleLength"
                    :validate="validateTitle"
                    :on-change="onTitleChange"
                />
                <span
                    v-else
                    class="select-none truncate text-base font-semibold text-[var(--color-text-default)] group-hover/open-target:text-[var(--color-text-primary)]"
                >
                    {{ title }}
                </span>
                <TagItem v-if="count" as="span" class="shrink-0 border-surface-3 bg-surface-2">
                    {{ count }}
                </TagItem>
            </div>
            <div class="min-w-0 flex-1" />
            <slot name="actions" :start-editing="startTitleEditing" />
        </div>
        <Accordion
            ref="accordion"
            :open-by-default="hideHeader || !props.collapsed"
            :force-open="forceOpen"
            overflow-visible
            class="w-full"
            @on-open="handleOpen"
            @on-close="handleClose"
        >
            <div class="mt-2.5">
                <slot />
            </div>
        </Accordion>
    </section>
</template>
