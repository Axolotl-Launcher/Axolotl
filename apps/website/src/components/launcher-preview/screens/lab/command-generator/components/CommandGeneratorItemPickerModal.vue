<script setup lang="ts">
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { ref, useTemplateRef } from 'vue'

import NewModal from '../../../../overlays/PreviewModal.vue'
import { getSlotDisplay } from '../../recipe-generator/display.ts'
import { resolveRecipeItemName } from '../../recipe-generator/item-names.ts'
import {
    buildSlotContext,
    loadVersionResources,
    TEXTURE_ATLAS,
    type TextureAtlas,
} from '../../recipe-generator/resources.ts'
import type { JavaVersionId, SlotValue } from '../../recipe-generator/types.ts'
import CommandGeneratorItemPicker, { type CommandItemEntry } from './CommandGeneratorItemPicker.vue'

const props = defineProps<{
    version: string
}>()

const emit = defineEmits<{
    select: [value: string]
}>()

const { formatMessage, locale } = useVIntl()
const modal = useTemplateRef<InstanceType<typeof NewModal>>('modal')
const entries = ref<CommandItemEntry[]>([])
const loading = ref(false)
const cachedLocale = ref('')
const atlas: TextureAtlas = TEXTURE_ATLAS

const messages = defineMessages({
    title: {
        id: 'app.lab.command-generator.item-picker.title',
        defaultMessage: 'Pick an item',
    },
})

async function show() {
    modal.value?.show()
    if (entries.value.length > 0 && cachedLocale.value === locale.value) return

    loading.value = true
    try {
        const version = props.version as JavaVersionId
        const resources = await loadVersionResources(version)
        const ctx = buildSlotContext([], [], resources)
        const currentLocale = locale.value
        entries.value = resources.items.map((item) => {
            const value: SlotValue = { kind: 'item', id: item.id }
            const display = getSlotDisplay(value, ctx)!
            const localizedName = resolveRecipeItemName(item.id, currentLocale, item.name)
            return {
                id: item.id,
                name: localizedName,
                display,
                value,
            }
        })
        cachedLocale.value = currentLocale
    } finally {
        loading.value = false
    }
}

function onPick(value: SlotValue) {
    if (value.kind !== 'item') return
    emit('select', value.id)
    modal.value?.hide()
}

defineExpose({ show })
</script>

<template>
    <NewModal
        ref="modal"
        :header="formatMessage(messages.title)"
        width="min(640px, calc(1280px - 2rem))"
        max-width="640px"
        scrollable
        max-content-height="min(40rem, 640px)"
    >
        <div class="flex h-[32rem] min-h-0 min-w-0 flex-col">
            <CommandGeneratorItemPicker
                :entries="entries"
                :atlas="atlas"
                :loading="loading"
                @pick="onPick"
            />
        </div>
    </NewModal>
</template>
