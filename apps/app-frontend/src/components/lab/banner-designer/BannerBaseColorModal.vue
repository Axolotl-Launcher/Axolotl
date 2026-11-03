<script setup lang="ts">
import { Button, defineMessages, NewModal, useVIntl } from '@modrinth/ui'
import { ref, useTemplateRef } from 'vue'

import BannerColorGrid from '@/components/lab/banner-designer/BannerColorGrid.vue'
import type { DyeColor } from '@/lab/banner-designer/colors.ts'

const props = defineProps<{ baseColor: DyeColor }>()

const emit = defineEmits<{
    change: [color: DyeColor]
}>()

const { formatMessage } = useVIntl()
const modal = useTemplateRef<InstanceType<typeof NewModal>>('modal')
const color = ref<DyeColor>(props.baseColor)

const messages = defineMessages({
    title: { id: 'app.lab.banner-designer.base-color.title', defaultMessage: 'Base color' },
    description: {
        id: 'app.lab.banner-designer.base-color.description',
        defaultMessage: 'The base color fills the banner behind every pattern.',
    },
    close: { id: 'app.lab.banner-designer.base-color.close', defaultMessage: 'Close' },
})

function show() {
    color.value = props.baseColor
    modal.value?.show()
}

function select(next: DyeColor) {
    color.value = next
    emit('change', next)
    modal.value?.hide()
}

defineExpose({ show })
</script>

<template>
    <NewModal
        ref="modal"
        :header="formatMessage(messages.title)"
        width="min(34rem, calc(95vw - 10rem))"
        max-width="34rem"
    >
        <div class="flex flex-col gap-3">
            <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.description) }}
            </p>
            <BannerColorGrid
                v-model="color"
                item-kind="banner"
                :label="formatMessage(messages.title)"
                @update:model-value="select"
            />
        </div>

        <template #actions>
            <div class="flex items-center justify-end gap-2">
                <Button type="quiet" @click="modal?.hide()">{{
                    formatMessage(messages.close)
                }}</Button>
            </div>
        </template>
    </NewModal>
</template>
