<script setup lang="ts">
import { MonitorIcon, RightArrowIcon } from '@modrinth/assets'
import { Button, defineMessages, LOCALES, useVIntl } from '@modrinth/ui'
import { computed, onMounted, ref } from 'vue'

import AxolotlLogo from '@/components/ui/AxolotlLogo.vue'
import { get, update } from '@/helpers/settings.ts'
import i18n, {
    getSystemResolvedLocale,
    isFollowingSystemLocale,
    setAppLocale,
    setFollowSystemLocale,
} from '@/i18n.config'

import { onboardingMessages, type OnboardingStep } from './onboardingConfig'

defineProps<{
    step: OnboardingStep
}>()

defineEmits<{
    start: []
    skip: []
}>()

const { formatMessage } = useVIntl()

const messages = defineMessages({
    languageLabel: {
        id: 'app.onboarding.welcome.language-label',
        defaultMessage: 'Language',
    },
    systemLanguage: {
        id: 'app.settings.language.system',
        defaultMessage: 'System language',
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

const applyingLocale = ref(false)
const settingsLocale = ref('')
const followSystem = ref(true)

onMounted(async () => {
    try {
        const loaded = await get()
        settingsLocale.value = loaded.locale
        followSystem.value = isFollowingSystemLocale(loaded.locale)
    } catch (error) {
        console.warn('Failed to load settings for onboarding language picker', error)
    }
})

const systemResolvedLocale = computed(() => getSystemResolvedLocale())

const systemLocaleName = computed(() => {
    const code = systemResolvedLocale.value
    return LOCALES.find((locale) => locale.code === code)?.name ?? code
})

const selectedLocale = computed(() =>
    followSystem.value
        ? systemResolvedLocale.value
        : settingsLocale.value || systemResolvedLocale.value,
)

const systemToggleTooltip = computed(() =>
    formatMessage(
        followSystem.value
            ? messages.systemLanguageEnabledTooltip
            : messages.systemLanguageDisabledTooltip,
    ),
)

// Native select keeps the welcome picker usable under the full-screen onboarding
// overlay (Combobox teleports its dropdown below the overlay z-index).
const localeOptions = computed(() =>
    LOCALES.map((locale) => ({
        value: locale.code,
        label: `${locale.name} — ${formatMessage(locale.translatedName)}`,
    })),
)

async function persistLocale(follow: boolean, concreteLocale: string) {
    if (applyingLocale.value) return
    applyingLocale.value = true
    const previousLocale = i18n.global.locale.value
    const previousFollow = followSystem.value
    try {
        setFollowSystemLocale(follow)
        followSystem.value = follow
        await setAppLocale(concreteLocale)
        await update({ locale: concreteLocale })
        settingsLocale.value = concreteLocale
    } catch (error) {
        console.warn('Failed to apply onboarding language', error)
        setFollowSystemLocale(previousFollow)
        followSystem.value = previousFollow
        await setAppLocale(previousLocale)
    } finally {
        applyingLocale.value = false
    }
}

async function unlockSystemLanguage() {
    if (!followSystem.value || applyingLocale.value) return
    await persistLocale(false, settingsLocale.value || systemResolvedLocale.value)
}

async function onLocaleChange(event: Event) {
    const newLocale = (event.target as HTMLSelectElement).value
    if (!newLocale) return
    await persistLocale(false, newLocale)
}

async function toggleFollowSystem() {
    if (applyingLocale.value) return
    if (followSystem.value) {
        await unlockSystemLanguage()
        return
    }
    await persistLocale(true, systemResolvedLocale.value)
}
</script>

<template>
    <div class="onboarding-welcome-content">
        <div class="onboarding-welcome-brand-stage">
            <div class="onboarding-welcome-brand">
                <div class="onboarding-welcome-logo">
                    <AxolotlLogo icon-only />
                </div>
                <div class="onboarding-welcome-wordmark" aria-label="Axolotl Launcher">
                    <span class="onboarding-welcome-wordmark-core" data-wordmark="Axolotl">
                        Axolotl
                    </span>
                    <span class="onboarding-welcome-wordmark-suffix" data-wordmark="Launcher">
                        Launcher
                    </span>
                </div>
            </div>
        </div>
        <div class="onboarding-welcome-panel">
            <div class="onboarding-welcome-panel-inner">
                <div class="onboarding-welcome-copy">
                    <h1 :id="`onboarding-title-${step.id}`">{{ formatMessage(step.title) }}</h1>
                    <p :id="`onboarding-description-${step.id}`">
                        {{ formatMessage(step.description) }}
                    </p>
                    <div class="onboarding-welcome-language">
                        <label
                            class="onboarding-welcome-language-label"
                            for="onboarding-welcome-language"
                        >
                            {{ formatMessage(messages.languageLabel) }}
                        </label>
                        <div class="onboarding-welcome-language-row">
                            <select
                                id="onboarding-welcome-language"
                                class="onboarding-welcome-language-select"
                                :value="followSystem ? '' : selectedLocale"
                                :disabled="applyingLocale"
                                @pointerdown="unlockSystemLanguage"
                                @focus="unlockSystemLanguage"
                                @change="onLocaleChange"
                            >
                                <option v-if="followSystem" value="" disabled>
                                    {{ formatMessage(messages.systemLanguage) }}
                                </option>
                                <option
                                    v-for="option in localeOptions"
                                    :key="option.value"
                                    :value="option.value"
                                >
                                    {{ option.label }}
                                </option>
                            </select>
                            <button
                                v-tooltip="systemToggleTooltip"
                                type="button"
                                role="switch"
                                class="onboarding-welcome-language-system"
                                :class="{ 'is-active': followSystem }"
                                :aria-checked="followSystem"
                                :aria-label="formatMessage(messages.systemLanguage)"
                                :disabled="applyingLocale"
                                @click="toggleFollowSystem"
                            >
                                <MonitorIcon class="size-4 shrink-0" />
                                <span>{{ formatMessage(messages.systemLanguage) }}</span>
                            </button>
                        </div>
                        <p v-if="followSystem" class="onboarding-welcome-language-system-meta">
                            {{ systemLocaleName }}
                        </p>
                    </div>
                </div>
                <div class="onboarding-welcome-actions">
                    <Button type="colored" color="brand" @click="$emit('start')"
                        >{{ formatMessage(step.action) }}
                        <RightArrowIcon />
                    </Button>
                    <div class="onboarding-welcome-secondary-action">
                        <span>{{ formatMessage(onboardingMessages.welcomeFooter) }}</span>
                        <Button type="quiet" @click="$emit('skip')"
                            >{{ formatMessage(onboardingMessages.skip) }}
                        </Button>
                    </div>
                </div>
            </div>
        </div>
    </div>
</template>

<style scoped lang="scss">
.onboarding-welcome-content {
    position: relative;
    width: 100%;
    height: 100dvh;
    min-height: 0;
    overflow: hidden;
    box-sizing: border-box;
    background: var(--color-bg);
    isolation: isolate;
}

.onboarding-welcome-brand-stage {
    position: absolute;
    inset: 0;
    overflow: hidden;
}

.onboarding-welcome-brand {
    position: absolute;
    top: 50%;
    left: 50%;
    display: flex;
    align-items: center;
    justify-content: flex-start;
    gap: clamp(0.75rem, 2vw, 1.5rem);
    transform: translate(-50%, -50%);
    animation: onboarding-welcome-brand-lift 3200ms cubic-bezier(0.22, 1, 0.36, 1) both;
}

.onboarding-welcome-logo {
    position: relative;
    z-index: 2;
    flex: none;
    width: clamp(7rem, 15vw, 11rem);
    height: clamp(7rem, 15vw, 11rem);
    animation: onboarding-welcome-logo-reveal 900ms cubic-bezier(0.16, 1, 0.3, 1) both;
}

.onboarding-welcome-wordmark {
    display: flex;
    align-items: baseline;
    gap: 0.35em;
    max-width: 0;
    overflow: hidden;
    color: var(--color-text-primary);
    font-size: 4.5rem;
    font-weight: 800;
    line-height: 1;
    letter-spacing: 0;
    white-space: nowrap;
    animation: onboarding-welcome-wordmark-reveal 1050ms 900ms cubic-bezier(0.16, 1, 0.3, 1) both;
}

.onboarding-welcome-wordmark span {
    position: relative;
    display: inline-block;
    transform: translateX(-4rem);
    animation: onboarding-welcome-wordmark-flight 1050ms 900ms cubic-bezier(0.16, 1, 0.3, 1) both;
}

.onboarding-welcome-wordmark-core {
    background-image: linear-gradient(90deg, var(--color-text-primary), var(--color-text-default));
    background-clip: text;
    -webkit-background-clip: text;
    color: transparent;
    -webkit-text-fill-color: transparent;
}

.onboarding-welcome-wordmark-suffix {
    color: var(--color-text-tertiary);
    font-weight: 650;
}

.onboarding-welcome-wordmark span::after {
    position: absolute;
    inset: 0;
    color: var(--color-brand);
    content: attr(data-wordmark);
    animation: onboarding-welcome-brand-scan 700ms 2050ms cubic-bezier(0.22, 1, 0.36, 1) both;
}

.onboarding-welcome-wordmark-suffix::after {
    animation-delay: 2400ms;
}

.onboarding-welcome-panel {
    position: absolute;
    right: 0;
    bottom: 0;
    left: 0;
    z-index: 3;
    border-top: 1px solid var(--color-divider);
    background: var(--surface-3);
    opacity: 0;
    transform: translateY(100%);
    animation: onboarding-welcome-panel-enter 650ms 3200ms cubic-bezier(0.22, 1, 0.36, 1) both;
}

.onboarding-welcome-panel::before {
    position: absolute;
    top: -2px;
    left: 0;
    width: clamp(4rem, 12vw, 10rem);
    height: 2px;
    background: var(--color-brand);
    content: '';
}

.onboarding-welcome-panel-inner {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: clamp(2rem, 6vw, 6rem);
    width: min(80rem, 100%);
    min-height: clamp(11rem, 27vh, 15rem);
    margin-inline: auto;
    padding: 1.5rem clamp(1.5rem, 5vw, 4rem);
    box-sizing: border-box;
}

.onboarding-welcome-copy {
    min-width: 0;
}

.onboarding-welcome-copy h1,
.onboarding-welcome-copy p {
    margin: 0;
}

.onboarding-welcome-copy h1 {
    color: var(--color-text-primary);
    font-size: 2.25rem;
    font-weight: 750;
    line-height: 1.15;
    letter-spacing: 0;
    text-wrap: balance;
}

.onboarding-welcome-copy p {
    max-width: 46rem;
    margin-top: 0.75rem;
    color: var(--color-text-tertiary);
    font-size: 1rem;
    line-height: 1.55;
    text-wrap: pretty;
}

.onboarding-welcome-language {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    max-width: 22rem;
    margin-top: 1.1rem;
}

.onboarding-welcome-language-label {
    color: var(--color-text-tertiary);
    font-size: 0.8125rem;
    font-weight: 600;
}

.onboarding-welcome-language-row {
    display: flex;
    align-items: stretch;
    gap: 0.5rem;
}

.onboarding-welcome-language-select {
    min-width: 0;
    flex: 1;
    min-height: 2.5rem;
    padding: 0.5rem 0.75rem;
    border: 1px solid var(--color-divider);
    border-radius: var(--radius-md);
    background: var(--surface-3);
    color: var(--color-text-primary);
    font: inherit;
    font-size: 0.9375rem;
    box-sizing: border-box;
}

.onboarding-welcome-language-select:focus-visible {
    outline: 2px solid var(--color-brand);
    outline-offset: 1px;
}

.onboarding-welcome-language-select:disabled {
    opacity: 0.75;
    cursor: default;
}

.onboarding-welcome-language-system {
    display: inline-flex;
    min-height: 2.5rem;
    flex-shrink: 0;
    align-items: center;
    gap: 0.4rem;
    padding: 0 0.75rem;
    border: 1px solid var(--color-divider);
    border-radius: var(--radius-md);
    background: var(--surface-4);
    color: var(--color-text-tertiary);
    font: inherit;
    font-size: 0.8125rem;
    font-weight: 600;
    white-space: nowrap;
    cursor: pointer;
    transition:
        background-color 140ms ease,
        border-color 140ms ease,
        color 140ms ease;
}

.onboarding-welcome-language-system:hover:not(:disabled) {
    color: var(--color-text-primary);
}

.onboarding-welcome-language-system:focus-visible {
    outline: 2px solid var(--color-brand);
    outline-offset: 1px;
}

.onboarding-welcome-language-system.is-active {
    border-color: var(--color-brand);
    background: color-mix(in srgb, var(--color-brand) 16%, transparent);
    color: var(--color-brand);
}

.onboarding-welcome-language-system:disabled {
    opacity: 0.7;
    cursor: default;
}

.onboarding-welcome-language-system-meta {
    margin: 0.35rem 0 0;
    color: var(--color-text-tertiary);
    font-size: 0.75rem;
}

.onboarding-welcome-actions {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 0.75rem;
    min-width: 15rem;
}

.onboarding-welcome-actions :deep(button) {
    width: 100%;
    justify-content: center;
    white-space: nowrap;
}

.onboarding-welcome-secondary-action {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 0.5rem;
    color: var(--color-text-tertiary);
    font-size: 0.8125rem;
}

.onboarding-welcome-secondary-action :deep(button) {
    width: auto;
}

@keyframes onboarding-welcome-logo-reveal {
    0% {
        opacity: 0;
        transform: scale(0.72);
    }
    65% {
        opacity: 1;
        transform: scale(1.04);
    }
    100% {
        opacity: 1;
        transform: scale(1);
    }
}

@keyframes onboarding-welcome-wordmark-reveal {
    from {
        max-width: 0;
    }
    to {
        max-width: 42rem;
    }
}

@keyframes onboarding-welcome-wordmark-flight {
    from {
        opacity: 0;
        transform: translateX(-4rem);
    }
    to {
        opacity: 1;
        transform: translateX(0);
    }
}

@keyframes onboarding-welcome-brand-scan {
    0%,
    12% {
        clip-path: inset(0 0 0 0);
    }
    100% {
        clip-path: inset(0 0 0 100%);
    }
}

@keyframes onboarding-welcome-brand-lift {
    0%,
    70% {
        top: 50%;
    }
    100% {
        top: clamp(9rem, 33vh, 18rem);
    }
}

@keyframes onboarding-welcome-panel-enter {
    from {
        opacity: 0;
        transform: translateY(100%);
    }
    to {
        opacity: 1;
        transform: translateY(0);
    }
}

@media (prefers-reduced-motion: reduce) {
    .onboarding-welcome-brand,
    .onboarding-welcome-logo,
    .onboarding-welcome-wordmark,
    .onboarding-welcome-wordmark span,
    .onboarding-welcome-panel {
        animation: none;
    }

    .onboarding-welcome-brand {
        top: clamp(9rem, 33vh, 18rem);
    }

    .onboarding-welcome-wordmark {
        max-width: 42rem;
    }

    .onboarding-welcome-logo,
    .onboarding-welcome-wordmark span,
    .onboarding-welcome-panel {
        opacity: 1;
        transform: none;
    }

    .onboarding-welcome-wordmark span::after {
        display: none;
    }
}

@media (max-width: 700px) {
    .onboarding-welcome-brand {
        gap: 0.75rem;
    }

    .onboarding-welcome-logo {
        width: 6.5rem;
        height: 6.5rem;
    }

    .onboarding-welcome-wordmark {
        flex-direction: column;
        align-items: flex-start;
        gap: 0.1rem;
        font-size: 2.5rem;
    }

    .onboarding-welcome-panel-inner {
        grid-template-columns: minmax(0, 1fr);
        gap: 1rem;
        min-height: min(19rem, 46vh);
        padding: 1.25rem;
    }

    .onboarding-welcome-copy h1 {
        font-size: 1.5rem;
    }

    .onboarding-welcome-copy p {
        margin-top: 0.5rem;
        font-size: 0.9375rem;
        line-height: 1.45;
    }

    .onboarding-welcome-actions {
        min-width: 0;
    }
}

@media (max-width: 480px) {
    .onboarding-welcome-wordmark {
        font-size: 1.875rem;
    }

    .onboarding-welcome-secondary-action > span {
        display: none;
    }
}

@media (min-width: 701px) and (max-width: 1000px) {
    .onboarding-welcome-wordmark {
        font-size: 3.5rem;
    }
}

@media (max-height: 680px) {
    .onboarding-welcome-brand {
        gap: 0.75rem;
    }

    .onboarding-welcome-logo {
        width: 6.5rem;
        height: 6.5rem;
    }

    .onboarding-welcome-wordmark {
        font-size: 2.5rem;
    }

    .onboarding-welcome-panel-inner {
        min-height: min(10rem, 32vh);
        padding-block: 1rem;
    }

    .onboarding-welcome-copy p {
        margin-top: 0.375rem;
        line-height: 1.4;
    }
}

@media (max-width: 700px) and (max-height: 680px) {
    .onboarding-welcome-brand {
        animation-name: onboarding-welcome-brand-lift-compact;
    }

    .onboarding-welcome-panel-inner {
        grid-template-columns: minmax(0, 1fr) auto;
        gap: 1rem;
        min-height: min(9.5rem, 40vh);
    }

    .onboarding-welcome-copy h1 {
        font-size: 1.25rem;
    }

    .onboarding-welcome-copy p {
        display: -webkit-box;
        overflow: hidden;
        -webkit-box-orient: vertical;
        -webkit-line-clamp: 2;
    }

    .onboarding-welcome-actions {
        min-width: 10rem;
    }

    .onboarding-welcome-secondary-action > span {
        display: none;
    }
}

@keyframes onboarding-welcome-brand-lift-compact {
    0%,
    70% {
        top: 50%;
    }
    100% {
        top: 29%;
    }
}
</style>
