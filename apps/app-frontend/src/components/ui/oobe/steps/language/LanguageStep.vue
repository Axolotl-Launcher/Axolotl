<script setup lang="ts">
import { CheckIcon, MonitorIcon } from '@modrinth/assets'
import { defineMessages, LOCALES, useVIntl } from '@modrinth/ui'
import { computed, nextTick, onMounted, ref } from 'vue'

import { get, update } from '@/helpers/settings.ts'
import i18n, {
    getSystemResolvedLocale,
    isFollowingSystemLocale,
    setAppLocale,
    setFollowSystemLocale,
} from '@/i18n.config'

import { useOobeStep } from '../../context'
import OobeChoice from '../../parts/OobeChoice.vue'

const SYSTEM = 'system'

const { formatMessage } = useVIntl()
const step = useOobeStep()

const messages = defineMessages({
    system: { id: 'app.oobe.language.system', defaultMessage: 'Match this device' },
    systemDetail: {
        id: 'app.oobe.language.system-detail',
        defaultMessage: 'Currently {language}',
    },
    listLabel: { id: 'app.oobe.language.list-label', defaultMessage: 'Launcher language' },
    saveFailed: {
        id: 'app.oobe.language.save-failed',
        defaultMessage: 'The language could not be saved. Try another choice or try again.',
    },
})

const choice = ref(SYSTEM)
const failed = ref(false)
const list = ref<HTMLElement | null>(null)
const systemLocale = computed(() => getSystemResolvedLocale())
const systemLocaleName = computed(
    () => LOCALES.find((locale) => locale.code === systemLocale.value)?.name ?? systemLocale.value,
)
const options = computed(() => [SYSTEM, ...LOCALES.map((locale) => locale.code)])

/** The language the launcher shows and has saved; a failed pick returns to it. */
let applied = { choice: SYSTEM, locale: i18n.global.locale.value }
/** Moves with every pick; a pick still waiting when a newer one arrives never runs. */
let revision = 0
let picks: Promise<void> = Promise.resolve()
let pending = 0

onMounted(async () => {
    try {
        const settings = await get()
        const saved = isFollowingSystemLocale(settings.locale) ? SYSTEM : settings.locale
        // A pick made while the settings were loading wins over what they held.
        if (revision === 0) {
            applied = { choice: saved, locale: i18n.global.locale.value }
            choice.value = saved
        }
    } catch (error) {
        console.warn('Failed to read the launcher language', error)
    }
    await nextTick()
    list.value
        ?.querySelector('[aria-checked="true"]')
        ?.scrollIntoView({ block: 'center', behavior: 'instant' })
})

/**
 * Picks apply and save one at a time, so locale loads and settings writes cannot finish
 * out of order, and the launcher ends on the latest pick.
 */
function choose(code: string) {
    if (code === choice.value) return
    const mine = ++revision
    choice.value = code
    failed.value = false
    pending += 1
    step.setBusy(true)
    picks = picks.then(async () => {
        try {
            if (mine !== revision) return
            const follow = code === SYSTEM
            const locale = follow ? systemLocale.value : code
            try {
                setFollowSystemLocale(follow)
                await setAppLocale(locale)
                await update({ locale })
                applied = { choice: code, locale }
            } catch (error) {
                console.warn('Failed to apply the launcher language', error)
                setFollowSystemLocale(applied.choice === SYSTEM)
                await setAppLocale(applied.locale).catch(() => {})
                if (mine === revision) {
                    choice.value = applied.choice
                    failed.value = true
                }
            }
        } finally {
            pending -= 1
            if (pending === 0) step.setBusy(false)
        }
    })
}

function onKeydown(event: KeyboardEvent) {
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return
    event.preventDefault()
    const current = options.value.indexOf(choice.value)
    const next = Math.min(
        Math.max(current + (event.key === 'ArrowDown' ? 1 : -1), 0),
        options.value.length - 1,
    )
    const target = list.value?.querySelectorAll<HTMLElement>('[role="radio"]')[next]
    target?.focus()
    target?.scrollIntoView({ block: 'nearest' })
    choose(options.value[next])
}
</script>

<template>
    <div class="flex flex-col gap-3">
        <p v-if="failed" class="m-0 text-sm text-[var(--color-red)]" role="alert">
            {{ formatMessage(messages.saveFailed) }}
        </p>
        <div
            ref="list"
            role="radiogroup"
            :aria-label="formatMessage(messages.listLabel)"
            class="flex flex-col gap-1.5"
            @keydown="onKeydown"
        >
            <OobeChoice
                role="radio"
                :selected="choice === SYSTEM"
                :tabindex="choice === SYSTEM ? 0 : -1"
                @select="choose(SYSTEM)"
            >
                <template #leading>
                    <span
                        class="flex size-9 shrink-0 items-center justify-center rounded-full bg-surface-4 text-[var(--color-text-primary)]"
                    >
                        <MonitorIcon class="size-4" />
                    </span>
                </template>
                <span class="truncate font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.system) }}
                </span>
                <span class="truncate text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.systemDetail, { language: systemLocaleName }) }}
                </span>
                <template #trailing>
                    <CheckIcon v-if="choice === SYSTEM" class="size-5 shrink-0 text-brand" />
                </template>
            </OobeChoice>
            <OobeChoice
                v-for="locale in LOCALES"
                :key="locale.code"
                role="radio"
                :selected="choice === locale.code"
                :tabindex="choice === locale.code ? 0 : -1"
                @select="choose(locale.code)"
            >
                <span
                    class="truncate font-semibold text-[var(--color-text-primary)]"
                    :lang="locale.code"
                >
                    {{ locale.name }}
                </span>
                <span class="truncate text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(locale.translatedName) }}
                </span>
                <template #trailing>
                    <CheckIcon v-if="choice === locale.code" class="size-5 shrink-0 text-brand" />
                </template>
            </OobeChoice>
        </div>
    </div>
</template>
