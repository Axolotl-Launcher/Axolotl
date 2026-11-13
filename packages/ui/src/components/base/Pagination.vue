<template>
    <div
        v-if="count > 1"
        class="flex flex-wrap items-center gap-1"
        :class="{ 'opacity-60': loading }"
    >
        <ButtonLink
            v-if="showFirstPage && linkFunction"
            type="quiet"
            circular
            icon-only
            :label="formatMessage(messages.firstPage)"
            :href="linkFunction(1)"
            :disabled="loading"
            @click.prevent="!loading && switchPage(1)"
        >
            <ChevronsLeftIcon />
        </ButtonLink>
        <IconButton
            v-else-if="showFirstPage"
            :label="formatMessage(messages.firstPage)"
            type="quiet"
            :disabled="loading"
            @click="switchPage(1)"
        >
            <ChevronsLeftIcon />
        </IconButton>
        <ButtonLink
            v-if="page > 1 && linkFunction"
            type="quiet"
            circular
            icon-only
            label="Previous Page"
            :href="linkFunction(page - 1)"
            :disabled="loading"
            @click.prevent="!loading && switchPage(page - 1)"
        >
            <ChevronLeftIcon />
        </ButtonLink>
        <IconButton
            v-else-if="page > 1"
            label="Previous Page"
            type="quiet"
            :disabled="loading"
            @click="switchPage(page - 1)"
        >
            <ChevronLeftIcon />
        </IconButton>
        <div
            v-for="(item, index) in pages"
            :key="'page-' + item + '-' + index"
            :class="{
                'page-number': page !== item,
                shrink: item !== '-' && item > 99,
            }"
            class="page-number-container"
        >
            <template v-if="item === '-'">
                <input
                    v-if="activeGapIndex === index"
                    ref="gapInput"
                    v-model="gapInputValue"
                    class="h-8 w-12 rounded-full border border-solid border-brand bg-surface-3 px-2 text-center text-sm text-[var(--color-text-primary)] outline-none focus-visible:ring-4 focus-visible:ring-brand-shadow"
                    type="number"
                    inputmode="numeric"
                    min="1"
                    :max="jumpCeiling ?? undefined"
                    :placeholder="formatMessage(messages.goToPagePlaceholder)"
                    :aria-label="formatMessage(messages.goToPage)"
                    @keydown.enter="commitGapInput"
                    @keydown.esc.prevent="closeGapInput"
                    @blur="commitGapInput"
                />
                <button
                    v-else
                    type="button"
                    class="grid h-8 w-8 place-content-center rounded-full text-[var(--color-text-tertiary)] transition-colors hover:bg-surface-3 hover:text-[var(--color-text-primary)] focus-visible:outline-none focus-visible:ring-4 focus-visible:ring-brand-shadow"
                    :aria-label="formatMessage(messages.goToPage)"
                    aria-expanded="false"
                    :disabled="loading"
                    @click="openGapInput(index)"
                >
                    <span class="rotate-90 grid place-content-center">
                        <EllipsisVerticalIcon />
                    </span>
                </button>
            </template>
            <template v-else-if="loading && page === item">
                <span
                    class="grid size-8 place-content-center rounded-full bg-surface-4 text-brand"
                    :aria-label="formatMessage(messages.loadingPage)"
                    role="status"
                >
                    <SpinnerIcon class="size-4 animate-spin" />
                </span>
            </template>
            <ButtonLink
                v-else-if="linkFunction"
                circular
                :type="page === item ? 'highlight' : 'quiet'"
                :color="page === item ? 'brand' : undefined"
                :href="linkFunction(item)"
                :disabled="loading"
                :aria-current="page === item ? 'page' : undefined"
                @click.prevent="!loading && page !== item ? switchPage(item) : null"
            >
                {{ item }}
            </ButtonLink>
            <Button
                v-else
                circular
                :type="page === item ? 'highlight' : 'quiet'"
                :color="page === item ? 'brand' : undefined"
                :disabled="loading"
                :aria-current="page === item ? 'page' : undefined"
                @click="page !== item ? switchPage(item) : null"
            >
                {{ item }}
            </Button>
        </div>

        <ButtonLink
            v-if="page !== pages[pages.length - 1] && linkFunction"
            type="quiet"
            circular
            icon-only
            label="Next Page"
            :href="linkFunction(page + 1)"
            :disabled="loading"
            @click.prevent="!loading && switchPage(page + 1)"
        >
            <ChevronRightIcon />
        </ButtonLink>
        <IconButton
            v-else-if="page !== pages[pages.length - 1]"
            label="Next Page"
            type="quiet"
            :disabled="loading"
            @click="switchPage(page + 1)"
        >
            <ChevronRightIcon />
        </IconButton>
        <ButtonLink
            v-if="showLastPage && linkFunction"
            type="quiet"
            circular
            icon-only
            :label="formatMessage(messages.lastPage)"
            :href="linkFunction(count)"
            :disabled="loading"
            @click.prevent="!loading && switchPage(count)"
        >
            <ChevronsRightIcon />
        </ButtonLink>
        <IconButton
            v-else-if="showLastPage"
            :label="formatMessage(messages.lastPage)"
            type="quiet"
            :disabled="loading"
            @click="switchPage(count)"
        >
            <ChevronsRightIcon />
        </IconButton>
        <input
            v-model="jumpInputValue"
            class="ml-1 h-8 w-12 rounded-full border border-solid border-surface-4 bg-surface-3 px-2 text-center text-sm text-[var(--color-text-primary)] outline-none transition-colors hover:border-brand focus-visible:border-brand focus-visible:ring-4 focus-visible:ring-brand-shadow"
            type="number"
            inputmode="numeric"
            min="1"
            :max="jumpCeiling ?? undefined"
            :title="jumpLimitHint"
            :placeholder="formatMessage(messages.goToPagePlaceholder)"
            :aria-label="formatMessage(messages.goToPage)"
            :disabled="loading"
            @focus="selectJumpInput"
            @keydown.enter="commitJumpInput"
            @keydown.esc.prevent="clearJumpInput"
            @blur="commitJumpInput"
        />
    </div>
</template>
<script setup lang="ts">
import {
    ChevronLeftIcon,
    ChevronRightIcon,
    ChevronsLeftIcon,
    ChevronsRightIcon,
    EllipsisVerticalIcon,
    SpinnerIcon,
} from '@modrinth/assets'
import { computed, nextTick, ref, watch } from 'vue'

import { defineMessages, useVIntl } from '../../composables/i18n'
import Button from './buttons/Button.vue'
import ButtonLink from './buttons/ButtonLink.vue'
import IconButton from './buttons/IconButton.vue'

const emit = defineEmits<{
    'switch-page': [page: number]
}>()

const props = withDefaults(
    defineProps<{
        page: number
        count: number
        loading?: boolean
        linkFunction?: (page: number) => string | undefined
        /**
         * Whether `count` is the real number of pages. A cursor-paged source only
         * knows that one more page exists, so its `count` is a lower bound and a
         * page past it is still worth asking for: the source resolves it and
         * reports where it landed.
         */
        countKnown?: boolean
        /**
         * Whether the source resolves a page by walking every page before it. A
         * cursor-paged source cannot reach a distant page in one request even
         * when it can name it, so its forward jumps stay capped.
         */
        cursorPaged?: boolean
        /**
         * How far ahead of the current page a jump may reach when reaching a page
         * means walking every page in between. Ignored for sources that read any
         * page directly.
         */
        jumpLimit?: number
    }>(),
    {
        page: 1,
        count: 1,
        loading: false,
        countKnown: true,
        cursorPaged: false,
        jumpLimit: 10,
    },
)

const { formatMessage } = useVIntl()

const messages = defineMessages({
    firstPage: {
        id: 'pagination.first-page',
        defaultMessage: 'First page',
    },
    goToPage: {
        id: 'pagination.go-to-page',
        defaultMessage: 'Go to page',
    },
    goToPagePlaceholder: {
        id: 'pagination.go-to-page.placeholder',
        defaultMessage: 'Page',
    },
    jumpLimit: {
        id: 'pagination.jump-limit',
        defaultMessage:
            'One jump covers up to {count, plural, one {# page} other {# pages}} ahead.',
    },
    lastPage: {
        id: 'pagination.last-page',
        defaultMessage: 'Last page',
    },
    loadingPage: {
        id: 'pagination.loading-page',
        defaultMessage: 'Loading page…',
    },
})

// Every bar offers the same controls: the jump field and the end buttons are not
// conditional on the page count, so a small result set does not get a bar that
// behaves differently from a large one. The end buttons only disappear once the
// reader is already on that end, or when the source cannot reach it in one go.
const showFirstPage = computed(() => props.page > 1)
// A cursor-paged source has no last page it can reach in one request, so naming
// one would be a button that starts a walk of every page in between.
const showLastPage = computed(
    () => !props.cursorPaged && props.countKnown && props.page < props.count,
)

/**
 * Whether the jump limit is what bounds this bar: a cursor-paged source walks
 * every page it skips, and a source with no known end has nothing else to stop
 * at.
 */
const jumpLimitApplies = computed(() => props.cursorPaged || !props.countKnown)

/**
 * The highest page a jump may ask for, or `null` when nothing caps it.
 *
 * A cursor-paged source reads page N by fetching every page before it, so the
 * jump is capped there even when `count` is exact: naming the last page is one
 * request for an offset source and a full walk for a cursor one.
 */
const jumpCeiling = computed(() => {
    const end = props.countKnown ? props.count : null
    if (!jumpLimitApplies.value || !Number.isFinite(props.jumpLimit)) return end

    const ahead = props.page + props.jumpLimit
    return end === null ? ahead : Math.min(end, ahead)
})

const jumpLimitHint = computed(() =>
    jumpLimitApplies.value && Number.isFinite(props.jumpLimit)
        ? formatMessage(messages.jumpLimit, { count: props.jumpLimit })
        : undefined,
)

const pages = computed(() => {
    const pages: ('-' | number)[] = []

    const first = 1
    // The strip never offers a page the bar would refuse to open, so a capped bar
    // ends its numbers at the last page it can reach from here.
    const last = Math.min(props.count, jumpCeiling.value ?? props.count)
    const current = props.page
    const prev = current - 1
    const next = current + 1
    const gap = '-'

    if (prev > first) {
        pages.push(first)
    }
    if (prev > first + 1) {
        pages.push(gap)
    }
    if (prev >= first) {
        pages.push(prev)
    }
    pages.push(current)
    if (next <= last) {
        pages.push(next)
    }
    if (next < last - 1) {
        pages.push(gap)
    }
    if (next < last) {
        pages.push(last)
    }

    return pages
})

const activeGapIndex = ref<number | null>(null)
const gapInputValue = ref('')
const gapInput = ref<HTMLInputElement[] | HTMLInputElement>()
const jumpInputValue = ref(String(props.page))

async function openGapInput(index: number) {
    activeGapIndex.value = index
    gapInputValue.value = ''
    await nextTick()
    const el = Array.isArray(gapInput.value) ? gapInput.value[0] : gapInput.value
    el?.focus()
    el?.select()
}

function closeGapInput() {
    activeGapIndex.value = null
    gapInputValue.value = ''
}

/**
 * A keydown that confirms an IME candidate belongs to the IME, not to the field:
 * the typed value is not in the model yet, and cancelling the default action
 * would stop the candidate from being confirmed at all.
 */
function isImeKeydown(event?: Event) {
    return event instanceof KeyboardEvent && event.isComposing
}

function fieldValue(event: Event | undefined, modelValue: string) {
    const element = event?.target
    return element instanceof HTMLInputElement ? element.value : modelValue
}

function clampPage(newPage: number) {
    const lowerBounded = Math.max(newPage, 1)
    const ceiling = jumpCeiling.value
    return ceiling === null ? lowerBounded : Math.min(lowerBounded, ceiling)
}

function jumpToPage(value: string) {
    const trimmed = value.trim()
    // `Number` rejects the partial reads `parseInt` accepts: a number field can
    // hold "1e3" or "2.5", and only a whole page is a page. An empty field reads
    // as 0, which would silently mean the first page, so it is rejected here.
    const parsed = trimmed === '' ? Number.NaN : Number(trimmed)
    if (!Number.isSafeInteger(parsed)) return null

    const target = clampPage(parsed)
    if (target !== props.page) switchPage(target)
    return target
}

function commitGapInput(event?: Event) {
    if (activeGapIndex.value === null || isImeKeydown(event)) return

    jumpToPage(fieldValue(event, gapInputValue.value))
    closeGapInput()
}

function commitJumpInput(event?: Event) {
    if (isImeKeydown(event)) return

    const target = jumpToPage(fieldValue(event, jumpInputValue.value))
    // The field always names a real page: where a capped jump landed, or the page
    // the reader is on when the value was not usable.
    jumpInputValue.value = String(target ?? props.page)
}

function clearJumpInput() {
    jumpInputValue.value = String(props.page)
}

/**
 * The field carries the page number, so the value has to be replaced rather than
 * typed after: without this a jump to 5 followed by a jump to 20 reads "520".
 */
function selectJumpInput(event: FocusEvent) {
    if (event.target instanceof HTMLInputElement) event.target.select()
}

watch(
    () => props.page,
    (page) => {
        closeGapInput()
        // Follow the page instead of clearing: a jump the source capped or
        // resolved elsewhere should still report the page it landed on.
        jumpInputValue.value = String(page)
    },
)

function switchPage(newPage: number) {
    emit('switch-page', clampPage(newPage))
}
</script>
