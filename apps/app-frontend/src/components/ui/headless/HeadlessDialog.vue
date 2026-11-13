<template>
    <DialogRoot :open="open" @update:open="emit('update:open', $event)">
        <DialogTrigger v-if="$slots.default" as-child>
            <slot />
        </DialogTrigger>
        <DialogPortal>
            <DialogOverlay :class="headlessTokenClasses.dialogOverlay" />
            <DialogContent
                :class="[headlessTokenClasses.dialogContent, $attrs.class as string | undefined]"
            >
                <div class="flex items-start justify-between gap-3">
                    <DialogTitle v-if="$slots.title" :class="headlessTokenClasses.dialogTitle">
                        <slot name="title" />
                    </DialogTitle>
                    <DialogClose as-child>
                        <HeadlessButton class="!h-8 !w-8 !px-0" :aria-label="closeLabel || 'Close'">
                            <XIcon class="size-4" />
                        </HeadlessButton>
                    </DialogClose>
                </div>
                <DialogDescription
                    v-if="$slots.description"
                    :class="headlessTokenClasses.dialogDescription"
                >
                    <slot name="description" />
                </DialogDescription>
                <slot name="body" />
            </DialogContent>
        </DialogPortal>
    </DialogRoot>
</template>

<script setup lang="ts">
import { XIcon } from '@modrinth/assets'
import {
    DialogClose,
    DialogContent,
    DialogDescription,
    DialogOverlay,
    DialogPortal,
    DialogRoot,
    DialogTitle,
    DialogTrigger,
} from 'reka-ui'

import HeadlessButton from './HeadlessButton.vue'
import { headlessTokenClasses } from './token-classes'

defineProps<{
    open?: boolean
    closeLabel?: string
}>()

const emit = defineEmits<{
    'update:open': [value: boolean]
}>()

defineOptions({ inheritAttrs: false })
</script>
