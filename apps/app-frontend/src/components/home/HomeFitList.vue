<script setup lang="ts" generic="T">
import './home-widget.css'

import { ChevronDownIcon } from '@modrinth/assets'
import { defineMessages, PopoutMenu, useVIntl } from '@modrinth/ui'
import { computed, inject, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'

import { type HomeItemVariant, homeWidgetMoreTargetKey } from './home-utils'

/**
 * A Home list that only ever shows whole rows. It lays every item out, keeps
 * the ones that fit inside the widget completely and hides the rest, so no row
 * is cut in half at the card's edge and nothing scrolls inside the card. The
 * hidden items open from a "+N" button beside the widget's count, as plain
 * rows in a popover.
 */
const props = withDefaults(
    defineProps<{
        items: readonly T[]
        itemKey: (item: T) => string | number
        variant: HomeItemVariant
        listClass?: string
    }>(),
    { listClass: undefined },
)

defineSlots<{
    default(props: { item: T; variant: HomeItemVariant }): unknown
}>()

const { formatMessage } = useVIntl()
const messages = defineMessages({
    more: { id: 'app.home.widgets.more', defaultMessage: '{count} more' },
    moreLabel: {
        id: 'app.home.widgets.more-label',
        defaultMessage: 'Show the {count} items that do not fit',
    },
})

const list = ref<HTMLUListElement | null>(null)
const moreTarget = inject(homeWidgetMoreTargetKey, ref(null))
/** How many items, from the start, fit inside the list completely. */
const fitCount = ref(Number.POSITIVE_INFINITY)

const hiddenItems = computed(() => props.items.slice(fitCount.value))

function measure() {
    const element = list.value
    if (!element) return
    const bottom = element.getBoundingClientRect().bottom + 0.5
    let count = 0
    for (const child of element.children) {
        if (child.getBoundingClientRect().bottom > bottom) break
        count += 1
    }
    fitCount.value = count
}

let observer: ResizeObserver | null = null
onMounted(() => {
    measure()
    observer = new ResizeObserver(() => measure())
    if (list.value) {
        observer.observe(list.value)
        if (list.value.firstElementChild) observer.observe(list.value.firstElementChild)
    }
})
onBeforeUnmount(() => observer?.disconnect())

watch(
    () => props.items.length + props.variant,
    async () => {
        await nextTick()
        if (list.value?.firstElementChild) observer?.observe(list.value.firstElementChild)
        measure()
    },
    { flush: 'post' },
)
</script>

<template>
    <ul ref="list" class="home-pinned-list" :class="listClass" :data-variant="variant">
        <li
            v-for="(item, index) in items"
            :key="itemKey(item)"
            class="min-w-0"
            :data-fit-hidden="index >= fitCount ? '' : undefined"
            :inert="index >= fitCount ? true : undefined"
        >
            <slot :item="item" :variant="variant" />
        </li>
    </ul>
    <Teleport v-if="moreTarget && hiddenItems.length > 0" defer :to="moreTarget">
        <PopoutMenu
            class="home-fit-more"
            type="button"
            placement="bottom-end"
            dropdown-class="home-fit-more-menu"
            :aria-label="formatMessage(messages.moreLabel, { count: hiddenItems.length })"
            :tooltip="formatMessage(messages.more, { count: hiddenItems.length })"
        >
            <span class="home-fit-more-long">{{
                formatMessage(messages.more, { count: hiddenItems.length })
            }}</span>
            <span class="home-fit-more-short">+{{ hiddenItems.length }}</span>
            <ChevronDownIcon aria-hidden="true" />
            <template #menu>
                <ul class="home-fit-more-list home-surface-bare">
                    <li v-for="item in hiddenItems" :key="itemKey(item)" class="min-w-0">
                        <slot :item="item" variant="row" />
                    </li>
                </ul>
            </template>
        </PopoutMenu>
    </Teleport>
</template>
