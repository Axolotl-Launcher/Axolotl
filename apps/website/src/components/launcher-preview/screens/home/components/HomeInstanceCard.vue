<script setup lang="ts">
import MoreVerticalIcon from '@modrinth/assets/icons/more-vertical.svg?component'
import PinIcon from '@modrinth/assets/icons/pin.svg?component'
import OverflowMenu from '@modrinth/ui/src/components/base/OverflowMenu.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed } from 'vue'

import type { GameInstance } from '../../../lib/types'
import { injectPreviewWindow } from '../../../lib/window'
import { homeMessages } from '../messages'
import Instance from './Instance.vue'

type InstanceCardLayout = 'spotlight' | 'row' | 'tile'

const props = withDefaults(
    defineProps<{
        instance: GameInstance
        pinned: boolean
        playing?: boolean
        layout?: InstanceCardLayout
    }>(),
    {
        playing: false,
        layout: 'row',
    },
)

const emit = defineEmits<{
    'pinned-change': [instance: GameInstance, pinned: boolean]
}>()

const { formatMessage } = useVIntl()
const { layer } = injectPreviewWindow()
const messages = homeMessages

const compact = computed(() => props.layout !== 'tile')
const menuOptions = computed(() => [
    {
        id: props.pinned ? 'unpin' : 'pin',
        action: () => emit('pinned-change', props.instance, !props.pinned),
    },
])
</script>

<template>
    <div class="home-instance-card relative min-w-0" :data-layout="layout" :data-compact="compact">
        <Instance
            :instance="instance"
            :compact="compact"
            :flat="true"
            :playing="playing"
            :first="layout === 'spotlight'"
        />
        <div class="home-instance-menu" @click.stop>
            <OverflowMenu
                class="relative inline-flex size-6 min-w-6 shrink-0 cursor-pointer items-center justify-center rounded-full border border-solid border-transparent bg-transparent p-0 text-[var(--color-text-default)] transition-[background-color,color,filter,transform] duration-150 hover:bg-surface-4 hover:brightness-[--hover-brightness] active:scale-[0.97] [&>svg]:size-4"
                :options="menuOptions"
                :tooltip="formatMessage(pinned ? messages.instancesUnpin : messages.instancesPin)"
                :container="layer ?? undefined"
            >
                <MoreVerticalIcon />
                <template #pin><PinIcon /> {{ formatMessage(messages.instancesPin) }}</template>
                <template #unpin>
                    <PinIcon class="rotate-45" /> {{ formatMessage(messages.instancesUnpin) }}
                </template>
            </OverflowMenu>
        </div>
    </div>
</template>

<style scoped>
.home-instance-card[data-compact='true'] {
    padding-right: 2.25rem;
}

.home-instance-menu {
    position: absolute;
    top: 0.25rem;
    right: 0.25rem;
    z-index: 2;
}

.home-instance-card[data-compact='true'] .home-instance-menu {
    top: 50%;
    right: 0;
    transform: translateY(-50%);
}
</style>
