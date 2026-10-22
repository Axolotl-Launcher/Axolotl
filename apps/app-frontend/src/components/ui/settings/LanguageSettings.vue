<script setup lang="ts">
import { MonitorIcon } from '@modrinth/assets'
import {
    Admonition,
    AutoLink,
    Combobox,
    type ComboboxOption,
    commonSettingsMessages,
    defineMessages,
    IntlFormatted,
    languageSelectorMessages,
    LOCALES,
    useVIntl,
} from '@modrinth/ui'
import { computed, ref } from 'vue'

import { get, update } from '@/helpers/settings.ts'
import i18n, {
    getSystemResolvedLocale,
    isFollowingSystemLocale,
    setAppLocale,
    setFollowSystemLocale,
} from '@/i18n.config'

import SettingsSaveStatus from './SettingsSaveStatus.vue'
import SettingsSection from './SettingsSection.vue'

const { formatMessage } = useVIntl()

const platform = computed(() => formatMessage(languageSelectorMessages.platformApp))

const settings = ref(await get())
const $isChanging = ref(false)
const saveStatus = ref<'idle' | 'saving' | 'saved' | 'error'>('idle')
const retryLocale = ref<string | null>(null)

const messages = defineMessages({
    systemLanguage: {
        id: 'app.settings.language.system',
        defaultMessage: 'System language',
    },
    systemLanguageDescription: {
        id: 'app.settings.language.system.description',
        defaultMessage: 'Follow the language of this device',
    },
    systemLanguageEnabledTooltip: {
        id: 'app.settings.language.system.enabled-tooltip',
        defaultMessage:
            'Following this device. Open the language list or turn this off to pick manually.',
    },
    systemLanguageDisabledTooltip: {
        id: 'app.settings.language.system.disabled-tooltip',
        defaultMessage: 'Turn on to follow the language of this device automatically.',
    },
})

const followSystem = ref(isFollowingSystemLocale(settings.value.locale))

const systemResolvedLocale = computed(() => getSystemResolvedLocale())

const systemLocaleMeta = computed(() => {
    const code = systemResolvedLocale.value
    const definition = LOCALES.find((locale) => locale.code === code)
    return {
        code,
        name: definition?.name ?? code,
        translatedName: definition ? formatMessage(definition.translatedName) : code,
    }
})

const selectedLocale = computed(() =>
    followSystem.value
        ? systemResolvedLocale.value
        : settings.value.locale || systemResolvedLocale.value,
)

const systemToggleTooltip = computed(() =>
    formatMessage(
        followSystem.value
            ? messages.systemLanguageEnabledTooltip
            : messages.systemLanguageDisabledTooltip,
    ),
)

const localeOptions = computed<ComboboxOption<string>[]>(() =>
    LOCALES.map((locale) => ({
        value: locale.code,
        label: `${locale.name} — ${formatMessage(locale.translatedName)}`,
        searchTerms: [locale.code, locale.name, formatMessage(locale.translatedName)],
    })),
)

async function persistLocale(follow: boolean, concreteLocale: string) {
    const previousStoredLocale = settings.value.locale
    const previousFollow = followSystem.value
    const previousActiveLocale = i18n.global.locale.value

    $isChanging.value = true
    saveStatus.value = 'saving'
    retryLocale.value = null

    try {
        setFollowSystemLocale(follow)
        followSystem.value = follow
        await setAppLocale(concreteLocale)
        settings.value.locale = concreteLocale
        await update({ locale: concreteLocale })
        saveStatus.value = 'saved'
    } catch {
        setFollowSystemLocale(previousFollow)
        followSystem.value = previousFollow
        await setAppLocale(previousActiveLocale)
        settings.value.locale = previousStoredLocale
        retryLocale.value = follow ? 'system' : concreteLocale
        saveStatus.value = 'error'
    } finally {
        $isChanging.value = false
    }
}

async function unlockSystemLanguage() {
    if (!followSystem.value || $isChanging.value) return
    await persistLocale(false, settings.value.locale || systemResolvedLocale.value)
}

async function onLocaleChange(newLocale: string) {
    if (!newLocale) return
    // Selecting a concrete language always leaves system-follow mode.
    await persistLocale(false, newLocale)
}

async function toggleFollowSystem() {
    if ($isChanging.value) return
    if (followSystem.value) {
        await unlockSystemLanguage()
        return
    }
    await persistLocale(true, systemResolvedLocale.value)
}

function retrySave() {
    if (!retryLocale.value) return
    if (retryLocale.value === 'system') void toggleFollowSystem()
    else void onLocaleChange(retryLocale.value)
}
</script>

<template>
    <div class="flex flex-col gap-6">
        <SettingsSection>
            <template #header>
                <h2
                    id="settings-target-language"
                    tabindex="-1"
                    class="m-0 text-lg font-semibold text-[var(--color-text-primary)]"
                >
                    {{ formatMessage(commonSettingsMessages.language) }}
                </h2>
            </template>
            <template #extra>
                <SettingsSaveStatus :status="saveStatus" :retry="retrySave" />
            </template>
            <div class="flex flex-col gap-4 p-4">
                <Admonition type="warning">
                    {{ formatMessage(languageSelectorMessages.languageWarning, { platform }) }}
                </Admonition>
                <p class="settings-page-description">
                    <IntlFormatted
                        :message-id="languageSelectorMessages.languagesDescription"
                        :values="{ platform }"
                    >
                        <template #~crowdin-link="{ children }">
                            <AutoLink to="https://translate.modrinth.com">
                                <component :is="() => children" />
                            </AutoLink>
                        </template>
                    </IntlFormatted>
                </p>
                <div data-onboarding-id="settings-language-select" class="flex flex-col gap-1.5">
                    <label class="text-sm font-semibold text-[var(--color-text-primary)]">
                        {{ formatMessage(commonSettingsMessages.language) }}
                    </label>
                    <div class="flex items-end gap-2">
                        <div class="min-w-0 flex-1">
                            <Combobox
                                :model-value="followSystem ? undefined : selectedLocale"
                                :display-value="
                                    followSystem
                                        ? formatMessage(messages.systemLanguage)
                                        : undefined
                                "
                                :options="localeOptions"
                                :disabled="$isChanging"
                                searchable
                                :search-placeholder="
                                    formatMessage(languageSelectorMessages.searchFieldPlaceholder)
                                "
                                trigger-class="w-full"
                                @open="unlockSystemLanguage"
                                @update:model-value="onLocaleChange"
                            />
                        </div>
                        <button
                            v-tooltip="systemToggleTooltip"
                            type="button"
                            role="switch"
                            class="inline-flex min-h-9 max-w-full shrink-0 flex-wrap items-center gap-1.5 rounded-xl border border-[var(--surface-5)] bg-[var(--surface-4)] px-2.5 py-2 text-[0.8125rem] font-semibold text-[var(--color-text-tertiary)] cursor-pointer transition-colors language-system-toggle"
                            :class="{ 'is-active': followSystem }"
                            :aria-checked="followSystem"
                            :aria-label="formatMessage(messages.systemLanguage)"
                            :disabled="$isChanging"
                            @click="toggleFollowSystem"
                        >
                            <MonitorIcon class="size-4 shrink-0" />
                            <span class="min-w-0 truncate">{{
                                formatMessage(messages.systemLanguage)
                            }}</span>
                        </button>
                    </div>
                    <p v-if="followSystem" class="m-0 text-xs text-[var(--color-text-tertiary)]">
                        {{ systemLocaleMeta.name }} — {{ systemLocaleMeta.translatedName }}
                    </p>
                </div>
            </div>
        </SettingsSection>
    </div>
</template>

<style scoped>
.settings-page-description {
    margin: 0;
    color: var(--color-text-tertiary);
    font-size: 0.875rem;
    line-height: 1.5;
}

.language-system-toggle:hover:not(:disabled) {
    border-color: var(--surface-5);
    color: var(--color-text-primary);
}

.language-system-toggle:focus-visible {
    outline: 2px solid var(--color-brand);
    outline-offset: 1px;
}

.language-system-toggle.is-active {
    border-color: var(--color-brand);
    background: var(--color-brand-highlight, var(--surface-3));
    color: var(--color-brand);
}

.language-system-toggle:disabled {
    opacity: 0.7;
    cursor: default;
}
</style>
