<script setup lang="ts">
import GithubIcon from '@modrinth/assets/external/github.svg?component'
import CheckIcon from '@modrinth/assets/icons/check.svg?component'
import ChevronDownIcon from '@modrinth/assets/icons/chevron-down.svg?component'
import CopyIcon from '@modrinth/assets/icons/copy.svg?component'
import ExternalIcon from '@modrinth/assets/icons/external.svg?component'
import GlobeIcon from '@modrinth/assets/icons/globe.svg?component'
import HeartHandshakeIcon from '@modrinth/assets/icons/heart-handshake.svg?component'
import IssuesIcon from '@modrinth/assets/icons/issues.svg?component'
import ScaleIcon from '@modrinth/assets/icons/scale.svg?component'
import UsersIcon from '@modrinth/assets/icons/users.svg?component'
import Avatar from '@modrinth/ui/src/components/base/Avatar.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import {
    defineAsyncComponent,
    nextTick,
    onErrorCaptured,
    onMounted,
    onScopeDispose,
    ref,
    shallowRef,
} from 'vue'

import { usePreview } from '../../../lib/state/index'
import { injectPreviewWindow } from '../../../lib/window'
import SettingsSection from '../components/SettingsSection.vue'
import { messages } from '../messages'
import { PREVIEW_APP_VERSION } from '../state'
import { contributors, type TeamMember, teamMembers } from './about/about-data'
import AfdianIcon from './about/assets/afdian.png'
import QqIcon from './about/assets/qq.svg?component'
import { AxolotlBrandConfig } from './about/brand'
import EasterEggContributorsModal from './about/easteregg/EasterEggContributorsModal.vue'
import EasterEggGameModal from './about/easteregg/EasterEggGameModal.vue'
import { type AboutMemberExperience, getAboutMemberExperience } from './about/member-experiences'
import { useReplayOnboarding } from './about/onboarding/replay'
import QqChannelIcon from './about/QqChannelIcon.vue'
import TeamMemberAvatar from './about/TeamMemberAvatar.vue'

/**
 * Settings > About (apps/app-frontend/src/components/ui/settings/AboutSettings.vue):
 * the 3D hero, the development team (a long press on some cards opens a mini
 * game in the hero), project and community links, license and attribution,
 * the contributors and the tour replay, plus the two keyboard easter eggs.
 * External links open in a new tab where the app hands them to the browser;
 * contributor avatars are remote images, so they show the app's fallback.
 */

// Load the scene independently so the About content can render immediately.
const AboutScene = defineAsyncComponent({
    loader: () => import('./about/AboutScene.vue'),
    suspensible: false,
    onError(error, retry, fail) {
        // one retry, then leave the scene slot empty instead of tearing down Settings
        if ((error as { __aboutSceneRetried?: boolean }).__aboutSceneRetried) {
            fail()
            return
        }
        ;(error as { __aboutSceneRetried?: boolean }).__aboutSceneRetried = true
        retry()
    },
})
const aboutSceneFailed = ref(false)

const { formatMessage } = useVIntl()
const { clock } = usePreview()
const { scale } = injectPreviewWindow()
const replayOnboarding = useReplayOnboarding()
const version = ref('')
const copied = ref(false)
const experienceHost = ref<HTMLElement>()
const activeMemberExperience = shallowRef<AboutMemberExperience>()
// A finished experience may be replaced by another one; a running one may not.
const activeExperienceSettled = ref(false)
const pressingMemberName = ref<string>()
let cancelLongPress: (() => void) | undefined
let cancelCopiedReset: (() => void) | undefined
let pressStart = { x: 0, y: 0 }
let suppressNextMemberClick = false

const licenseUrl = `${AxolotlBrandConfig.repositoryUrl}/blob/main/LICENSE`
const copyingUrl = `${AxolotlBrandConfig.repositoryUrl}/blob/main/COPYING.md`
const thirdPartyLicensesUrl = `${AxolotlBrandConfig.repositoryUrl}/tree/main/third-party/licenses`

onMounted(() => {
    // getVersion()
    version.value = PREVIEW_APP_VERSION
})

// Keep a failing 3D scene from bubbling into Settings Suspense / the shell.
onErrorCaptured(() => {
    aboutSceneFailed.value = true
    return false
})

/** Copies text with the async clipboard, or the selection fallback where it is refused. */
async function writeClipboardText(text: string) {
    try {
        await navigator.clipboard.writeText(text)
        return
    } catch {
        const field = document.createElement('textarea')
        field.value = text
        field.setAttribute('readonly', '')
        field.style.position = 'fixed'
        field.style.opacity = '0'
        document.body.appendChild(field)
        field.select()
        const copiedText = document.execCommand('copy')
        field.remove()
        if (!copiedText) throw new Error('Clipboard unavailable')
    }
}

async function copyQqGroupNumber() {
    await writeClipboardText(AxolotlBrandConfig.qqGroupNumber)
    copied.value = true
    cancelCopiedReset?.()
    cancelCopiedReset = clock.after(3000, () => {
        copied.value = false
    })
}

/** `scrollIntoView({ block: 'center' })` within the settings panel, not the website page. */
function scrollHostIntoView(element: HTMLElement) {
    const container = element.closest<HTMLElement>('.settings-content-scroll')
    if (!container) return
    const factor = scale.value || 1
    const offset =
        (element.getBoundingClientRect().top - container.getBoundingClientRect().top) / factor
    const top = container.scrollTop + offset - (container.clientHeight - element.offsetHeight) / 2
    container.scrollTo({ top, behavior: 'smooth' })
}

function cancelMemberLongPress() {
    cancelLongPress?.()
    cancelLongPress = undefined
    pressingMemberName.value = undefined
}

function startMemberLongPress(member: TeamMember, event: PointerEvent) {
    const experience = getAboutMemberExperience(member.experience)
    if (!experience || event.button !== 0) return
    // Only one member experience owns the host at a time. A game that is still
    // being played must not be replaced; one sitting on its result screen may be.
    if (activeMemberExperience.value && !activeExperienceSettled.value) return

    cancelMemberLongPress()
    pressStart = { x: event.clientX, y: event.clientY }
    pressingMemberName.value = member.name
    cancelLongPress = clock.after(experience.longPressDuration, async () => {
        cancelLongPress = undefined
        activeExperienceSettled.value = false
        // Re-picking the same game must still restart it, so unmount the finished
        // instance first instead of assigning an identical component definition.
        if (activeMemberExperience.value === experience) {
            activeMemberExperience.value = undefined
            await nextTick()
        }
        activeMemberExperience.value = experience
        suppressNextMemberClick = true
        cancelMemberLongPress()
        await nextTick()
        if (experienceHost.value) scrollHostIntoView(experienceHost.value)
    })
}

function moveMemberLongPress(event: PointerEvent) {
    if (!cancelLongPress) return
    if (
        Math.hypot(event.clientX - pressStart.x, event.clientY - pressStart.y) /
            (scale.value || 1) >
        8
    ) {
        cancelMemberLongPress()
    }
}

function handleMemberClick(event: MouseEvent) {
    if (!suppressNextMemberClick) return
    suppressNextMemberClick = false
    event.preventDefault()
    event.stopPropagation()
}

function handleMemberContextMenu(member: TeamMember, event: MouseEvent) {
    if (getAboutMemberExperience(member.experience)) event.preventDefault()
}

function closeMemberExperience() {
    activeMemberExperience.value = undefined
    activeExperienceSettled.value = false
}

const gameModal = ref<InstanceType<typeof EasterEggGameModal> | null>(null)
const contributorsModal = ref<InstanceType<typeof EasterEggContributorsModal> | null>(null)

let typedBuffer = ''
const secretCodes = ['cyf112233', 'cxkcxkckx']

const konamiSequence = [
    'ArrowUp',
    'ArrowUp',
    'ArrowDown',
    'ArrowDown',
    'ArrowLeft',
    'ArrowRight',
    'ArrowLeft',
    'ArrowRight',
    'KeyB',
    'KeyA',
]
let konamiIndex = 0

function handleEasterEggKeydown(event: KeyboardEvent) {
    typedBuffer = (typedBuffer + event.key).toLowerCase()
    const maxCodeLen = Math.max(...secretCodes.map((c) => c.length))
    if (typedBuffer.length > maxCodeLen) {
        typedBuffer = typedBuffer.slice(-maxCodeLen)
    }
    if (secretCodes.some((code) => typedBuffer.endsWith(code))) {
        typedBuffer = ''
        gameModal.value?.show()
        return
    }

    const expected = konamiSequence[konamiIndex]
    if (event.code === expected) {
        konamiIndex++
        if (konamiIndex === konamiSequence.length) {
            konamiIndex = 0
            contributorsModal.value?.show()
        }
    } else {
        konamiIndex = event.code === konamiSequence[0] ? 1 : 0
    }
}

function onEasterEggOpenGame() {
    gameModal.value?.show()
}

document.addEventListener('keydown', handleEasterEggKeydown)
onScopeDispose(() => document.removeEventListener('keydown', handleEasterEggKeydown))

onScopeDispose(cancelMemberLongPress)
onScopeDispose(() => cancelCopiedReset?.())

const projectLinks = [
    {
        href: AxolotlBrandConfig.website,
        label: messages.appSettingsAboutProjectWebsite,
        icon: GlobeIcon,
    },
    {
        href: AxolotlBrandConfig.repositoryUrl,
        label: messages.appSettingsAboutRepository,
        icon: GithubIcon,
    },
    {
        href: AxolotlBrandConfig.supportUrl,
        label: messages.appSettingsAboutReportIssue,
        icon: IssuesIcon,
    },
    {
        href: AxolotlBrandConfig.qqChannelUrl,
        label: messages.appSettingsAboutQqChannel,
        icon: QqChannelIcon,
    },
]
</script>

<template>
    <div class="about-page flex flex-col gap-6">
        <!-- The hero carries the 3D experience host and has no heading, so it
             stays its own panel rather than a `SettingsSection` (which would
             render no anchor without a title). -->
        <section id="settings-target-about-product" tabindex="-1" class="about-panel">
            <div class="flex flex-col items-center gap-4">
                <div
                    ref="experienceHost"
                    class="relative m-0 h-64 w-full overflow-hidden rounded-xl"
                    style="
                        mask-image: linear-gradient(to bottom, black 97%, transparent 100%);
                        -webkit-mask-image: linear-gradient(to bottom, black 97%, transparent 100%);
                    "
                >
                    <AboutScene v-if="!aboutSceneFailed" />
                    <component
                        :is="activeMemberExperience?.component"
                        v-if="activeMemberExperience"
                        @exit="closeMemberExperience"
                        @settled="activeExperienceSettled = $event"
                    />
                </div>
                <div class="min-w-0 text-center">
                    <h2 class="m-0 text-xl font-semibold text-[var(--color-text-primary)]">
                        {{
                            formatMessage(messages.appSettingsAboutProductTitle, {
                                productName: AxolotlBrandConfig.productName,
                            })
                        }}
                    </h2>
                    <p class="m-0 mt-1 text-[var(--color-text-tertiary)]">
                        {{ formatMessage(messages.appSettingsAboutVersion, { version }) }}
                    </p>
                </div>
            </div>
            <p class="m-0 mt-3 text-center text-[var(--color-text-default)]">
                {{ formatMessage(messages.appSettingsAboutDescription) }}
            </p>
        </section>

        <SettingsSection>
            <template #header>
                <h3
                    class="m-0 flex items-center gap-2 text-base font-semibold text-[var(--color-text-primary)]"
                >
                    <UsersIcon class="size-5 text-[var(--color-text-tertiary)]" />
                    {{ formatMessage(messages.appSettingsAboutDevelopmentTeam) }}
                </h3>
            </template>
            <div class="p-4">
                <ul class="m-0 grid list-none grid-cols-2 gap-3 p-0 sm:grid-cols-3">
                    <li v-for="member in teamMembers" :key="member.name" class="min-w-0">
                        <component
                            :is="member.url ? 'a' : 'div'"
                            :href="member.url"
                            :target="member.url ? '_blank' : undefined"
                            :rel="member.url ? 'noopener noreferrer' : undefined"
                            class="flex min-w-0 select-none flex-col items-center gap-3 rounded-xl bg-surface-4 p-4"
                            :class="[
                                member.url
                                    ? 'transition-colors hover:bg-surface-5'
                                    : 'cursor-default',
                                pressingMemberName === member.name
                                    ? 'ring-4 ring-brand-shadow'
                                    : '',
                            ]"
                            @pointerdown="startMemberLongPress(member, $event)"
                            @pointermove="moveMemberLongPress"
                            @pointerup="cancelMemberLongPress"
                            @pointercancel="cancelMemberLongPress"
                            @dragstart="cancelMemberLongPress"
                            @click="handleMemberClick"
                            @contextmenu="handleMemberContextMenu(member, $event)"
                        >
                            <TeamMemberAvatar :name="member.name" :src="member.avatarUrl" />
                            <span
                                class="block truncate text-center font-semibold text-[var(--color-text-primary)]"
                                >{{ member.name }}</span
                            >
                        </component>
                    </li>
                </ul>
            </div>
        </SettingsSection>

        <SettingsSection>
            <template #header>
                <h3
                    class="m-0 flex items-center gap-2 text-base font-semibold text-[var(--color-text-primary)]"
                >
                    <HeartHandshakeIcon class="size-5 text-[var(--color-text-tertiary)]" />
                    {{ formatMessage(messages.appSettingsAboutCommunitySupport) }}
                </h3>
            </template>
            <div class="grid gap-3 p-4 sm:grid-cols-2">
                <a
                    v-for="link in projectLinks"
                    :key="link.label.id"
                    :href="link.href"
                    target="_blank"
                    rel="noopener noreferrer"
                    class="flex min-w-0 items-center gap-3 rounded-xl bg-surface-4 p-4 transition-colors hover:bg-surface-5"
                >
                    <span
                        class="flex size-10 shrink-0 items-center justify-center rounded-xl bg-surface-2 text-[var(--color-text-primary)]"
                    >
                        <component :is="link.icon" class="size-6" />
                    </span>
                    <span class="min-w-0 flex-1 font-semibold text-[var(--color-text-primary)]">
                        {{ formatMessage(link.label) }}
                    </span>
                    <ExternalIcon class="size-5 shrink-0 text-[var(--color-text-tertiary)]" />
                </a>

                <button
                    type="button"
                    :disabled="copied"
                    :aria-label="
                        copied
                            ? formatMessage(messages.appSettingsAboutCopiedQqGroup)
                            : formatMessage(messages.appSettingsAboutCopyQqGroup)
                    "
                    class="flex w-full min-w-0 items-center gap-3 rounded-xl bg-surface-4 p-4 text-left transition-colors hover:bg-surface-5 disabled:cursor-default"
                    @click="copyQqGroupNumber"
                >
                    <span
                        class="flex size-10 shrink-0 items-center justify-center rounded-xl bg-surface-2 text-[var(--color-text-primary)]"
                    >
                        <QqIcon class="size-6" />
                    </span>
                    <span class="min-w-0 flex-1">
                        <span class="block font-semibold text-[var(--color-text-primary)]">
                            {{ formatMessage(messages.appSettingsAboutQqGroup) }}
                        </span>
                        <span class="block text-sm text-[var(--color-text-tertiary)]">
                            {{ AxolotlBrandConfig.qqGroupNumber }}
                        </span>
                    </span>
                    <span class="shrink-0" aria-live="polite">
                        <CheckIcon v-if="copied" class="size-5 text-green" />
                        <CopyIcon v-else class="size-5 text-[var(--color-text-tertiary)]" />
                        <span class="sr-only">
                            {{
                                copied
                                    ? formatMessage(messages.appSettingsAboutCopiedQqGroup)
                                    : formatMessage(messages.appSettingsAboutCopyQqGroup)
                            }}
                        </span>
                    </span>
                </button>

                <a
                    :href="AxolotlBrandConfig.sponsorUrl"
                    target="_blank"
                    rel="noopener noreferrer"
                    class="flex min-w-0 items-center gap-3 rounded-xl bg-surface-4 p-4 transition-colors hover:bg-surface-5"
                >
                    <span
                        class="flex size-10 shrink-0 items-center justify-center rounded-xl bg-surface-2"
                    >
                        <img :src="AfdianIcon" alt="" class="size-7 object-contain" />
                    </span>
                    <span class="min-w-0 flex-1">
                        <span class="block font-semibold text-[var(--color-text-primary)]">
                            {{ formatMessage(messages.appSettingsAboutAfdian) }}
                        </span>
                        <span class="block text-sm text-[var(--color-text-tertiary)]">
                            {{ formatMessage(messages.appSettingsAboutAfdianDescription) }}
                        </span>
                    </span>
                    <ExternalIcon class="size-5 shrink-0 text-[var(--color-text-tertiary)]" />
                </a>
            </div>
        </SettingsSection>

        <SettingsSection>
            <template #header>
                <h3
                    class="m-0 flex items-center gap-2 text-base font-semibold text-[var(--color-text-primary)]"
                >
                    <ScaleIcon class="size-5 text-[var(--color-text-tertiary)]" />
                    {{ formatMessage(messages.appSettingsAboutLicenseAttribution) }}
                </h3>
            </template>
            <div class="flex flex-col gap-3 p-4">
                <p class="m-0 text-[var(--color-text-default)]">
                    {{ formatMessage(messages.appSettingsAboutAttribution) }}
                </p>
                <p class="m-0 mt-2 text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.appSettingsAboutNotAffiliated) }}
                </p>
                <div class="mt-3 flex flex-wrap gap-2">
                    <a
                        :href="licenseUrl"
                        target="_blank"
                        rel="noopener noreferrer"
                        class="inline-flex items-center gap-2 rounded-lg bg-surface-4 px-3 py-2 text-sm font-semibold text-[var(--color-text-primary)] transition-colors hover:bg-surface-5"
                    >
                        {{ formatMessage(messages.appSettingsAboutProjectLicense) }}
                        <ExternalIcon class="size-4 text-[var(--color-text-tertiary)]" />
                    </a>
                    <a
                        :href="copyingUrl"
                        target="_blank"
                        rel="noopener noreferrer"
                        class="inline-flex items-center gap-2 rounded-lg bg-surface-4 px-3 py-2 text-sm font-semibold text-[var(--color-text-primary)] transition-colors hover:bg-surface-5"
                    >
                        {{ formatMessage(messages.appSettingsAboutCopyingGuidelines) }}
                        <ExternalIcon class="size-4 text-[var(--color-text-tertiary)]" />
                    </a>
                    <a
                        :href="thirdPartyLicensesUrl"
                        target="_blank"
                        rel="noopener noreferrer"
                        class="inline-flex items-center gap-2 rounded-lg bg-surface-4 px-3 py-2 text-sm font-semibold text-[var(--color-text-primary)] transition-colors hover:bg-surface-5"
                    >
                        {{ formatMessage(messages.appSettingsAboutThirdPartyLicenses) }}
                        <ExternalIcon class="size-4 text-[var(--color-text-tertiary)]" />
                    </a>
                    <a
                        href="https://github.com/modrinth/code"
                        target="_blank"
                        rel="noopener noreferrer"
                        class="inline-flex items-center gap-2 rounded-lg bg-surface-4 px-3 py-2 text-sm font-semibold text-[var(--color-text-primary)] transition-colors hover:bg-surface-5"
                    >
                        {{ formatMessage(messages.appSettingsAboutOriginalSource) }}
                        <ExternalIcon class="size-4 text-[var(--color-text-tertiary)]" />
                    </a>
                </div>
            </div>
        </SettingsSection>

        <details class="about-settings-details group pt-4">
            <summary
                class="flex cursor-pointer list-none items-center gap-2 text-base font-semibold text-[var(--color-text-primary)] [&::-webkit-details-marker]:hidden"
            >
                <UsersIcon class="size-5 text-[var(--color-text-tertiary)]" />
                <span>{{ formatMessage(messages.appSettingsAboutContributors) }}</span>
                <span
                    class="rounded-full bg-surface-4 px-2 py-0.5 text-xs text-[var(--color-text-tertiary)]"
                >
                    {{
                        formatMessage(messages.appSettingsAboutContributorsCount, {
                            count: contributors.length,
                        })
                    }}
                </span>
                <ChevronDownIcon
                    class="ml-auto size-5 text-[var(--color-text-tertiary)] transition-transform group-open:rotate-180"
                />
            </summary>
            <div class="mt-3 flex flex-wrap gap-2">
                <a
                    v-for="contributor in contributors"
                    :key="contributor.name"
                    :href="contributor.url"
                    target="_blank"
                    rel="noopener noreferrer"
                    class="flex min-w-0 items-center gap-1.5 rounded-full bg-surface-4 py-1 pl-1 pr-2.5 transition-colors hover:bg-surface-5"
                >
                    <Avatar :alt="contributor.name" size="1.5rem" circle no-shadow loading="lazy" />
                    <span class="truncate text-sm text-[var(--color-text-default)]">{{
                        contributor.name
                    }}</span>
                </a>
            </div>
        </details>

        <div id="settings-target-about-replay-tour" tabindex="-1" class="flex flex-wrap gap-2">
            <Button type="base" @click="replayOnboarding('main')">
                {{ formatMessage(messages.appSettingsAboutReplayOnboarding) }}
            </Button>
        </div>
    </div>

    <EasterEggGameModal ref="gameModal" />
    <EasterEggContributorsModal ref="contributorsModal" @open-game="onEasterEggOpenGame" />
</template>

<style scoped>
.about-settings-details {
    border-top: 1px solid
        var(--settings-divider, color-mix(in srgb, var(--surface-4) 55%, transparent));
}

.about-panel {
    padding: 1.25rem;
    border: 1px solid var(--surface-4);
    border-radius: var(--radius-md);
    background: var(--surface-3);
}
</style>
