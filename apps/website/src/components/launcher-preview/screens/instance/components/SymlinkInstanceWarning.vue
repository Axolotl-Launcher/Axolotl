<script setup lang="ts">
import LinkIcon from '@modrinth/assets/icons/link.svg?component'
import Admonition from '@modrinth/ui/src/components/base/Admonition.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed } from 'vue'

import { symlinkWarningMessages as messages } from '../messages'
import { injectSymlinkWarningDismiss } from './symlink-warning'

/** Port of `components/ui/SymlinkInstanceWarning.vue`. */
const props = withDefaults(
    defineProps<{
        symlinkTarget: string
        variant?: 'write' | 'delete'
        dismissible?: boolean
        badgeOnly?: boolean
    }>(),
    {
        variant: 'write',
        dismissible: false,
        badgeOnly: false,
    },
)

const emit = defineEmits<{
    dismiss: []
    dismissPermanently: []
}>()

defineOptions({ inheritAttrs: false })

const { formatMessage } = useVIntl()

const dismissState = injectSymlinkWarningDismiss()

const effectivelyHidden = computed(() => {
    if (props.variant === 'delete') return false
    return dismissState?.isHidden.value ?? false
})

const showBadge = computed(() => props.badgeOnly || effectivelyHidden.value)

const effectivelyDismissible = computed(() => {
    if (props.variant === 'delete') return false
    return props.dismissible || !!dismissState
})

function handleDismiss() {
    dismissState?.dismissTemp()
    emit('dismiss')
}

function handleDismissPermanently() {
    dismissState?.dismissPermanently()
    emit('dismissPermanently')
}
</script>

<template>
    <div :class="$attrs.class">
        <div v-if="showBadge" class="flex">
            <span
                v-tooltip="formatMessage(messages.writeBody, { path: props.symlinkTarget })"
                class="inline-flex items-center gap-1 rounded-full bg-bg-orange px-2 py-0.5 text-xs font-medium text-brand-orange"
            >
                <LinkIcon class="size-3" />
                {{ formatMessage(messages.sharedBadge) }}
            </span>
        </div>
        <template v-else>
            <Admonition
                v-if="props.variant === 'delete'"
                type="warning"
                :header="formatMessage(messages.deleteHeader)"
            >
                {{ formatMessage(messages.deleteBody, { path: props.symlinkTarget }) }}
            </Admonition>
            <Admonition
                v-else
                type="warning"
                :header="formatMessage(messages.writeHeader)"
                :dismissible="effectivelyDismissible"
                @dismiss="handleDismiss"
            >
                <span>
                    {{ formatMessage(messages.writeBody, { path: props.symlinkTarget }) }}
                    <template v-if="effectivelyDismissible">
                        {{ ' ' }}
                        <Button
                            type="quiet"
                            color="orange"
                            interaction="filled"
                            size="2xs"
                            native-type="button"
                            @click="handleDismissPermanently"
                        >
                            {{ formatMessage(messages.dismissPermanently) }}
                        </Button>
                    </template>
                </span>
            </Admonition>
        </template>
    </div>
</template>
