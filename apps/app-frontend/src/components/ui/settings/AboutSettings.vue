<script setup lang="ts">
import {
    CheckIcon,
    ChevronDownIcon,
    CopyIcon,
    ExternalIcon,
    GithubIcon,
    GlobeIcon,
    HeartHandshakeIcon,
    IssuesIcon,
    ScaleIcon,
    UsersIcon,
} from '@modrinth/assets'
import { Avatar, Button, defineMessages, useVIntl } from '@modrinth/ui'
import { getVersion } from '@tauri-apps/api/app'
import {
    defineAsyncComponent,
    inject,
    nextTick,
    onErrorCaptured,
    onMounted,
    onScopeDispose,
    ref,
    shallowRef,
} from 'vue'

import AfdianIcon from '@/assets/external/afdian.png'
import QqIcon from '@/assets/external/qq.svg?component'
import EasterEggContributorsModal from '@/components/ui/easteregg/EasterEggContributorsModal.vue'
import EasterEggGameModal from '@/components/ui/easteregg/EasterEggGameModal.vue'
import { AxolotlBrandConfig } from '@/config'
import { contributors, type TeamMember, teamMembers } from '@/data/about'

import { type AboutMemberExperience, getAboutMemberExperience } from './about-member-experiences'
import QqChannelIcon from './QqChannelIcon.vue'
import SettingsSection from './SettingsSection.vue'
import TeamMemberAvatar from './TeamMemberAvatar.vue'

// Load the scene independently so the About content can render immediately.
const AboutScene = defineAsyncComponent({
    loader: () => import('../AboutScene.vue'),
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
const version = ref('')
const copied = ref(false)
const experienceHost = ref<HTMLElement>()
const activeMemberExperience = shallowRef<AboutMemberExperience>()
// A finished experience may be replaced by another one; a running one may not.
const activeExperienceSettled = ref(false)
const pressingMemberName = ref<string>()
let longPressTimer: ReturnType<typeof window.setTimeout> | undefined
let pressStart = { x: 0, y: 0 }
let suppressNextMemberClick = false
const replayOnboarding = inject<(mode: 'main' | 'instance') => Promise<void>>('replayOnboarding')

const licenseUrl = `${AxolotlBrandConfig.repositoryUrl}/blob/main/LICENSE`
const copyingUrl = `${AxolotlBrandConfig.repositoryUrl}/blob/main/COPYING.md`
const thirdPartyLicensesUrl = `${AxolotlBrandConfig.repositoryUrl}/tree/main/third-party/licenses`

onMounted(() => {
    void getVersion()
        .then((resolved) => {
            version.value = resolved
        })
        .catch(() => {
            // keep empty version string; do not fail the settings category
        })
})

// Keep a failing 3D scene from bubbling into Settings Suspense / the shell.
onErrorCaptured(() => {
    aboutSceneFailed.value = true
    return false
})

async function copyQqGroupNumber() {
    await navigator.clipboard.writeText(AxolotlBrandConfig.qqGroupNumber)
    copied.value = true
    setTimeout(() => {
        copied.value = false
    }, 3000)
}

function cancelMemberLongPress() {
    if (longPressTimer) window.clearTimeout(longPressTimer)
    longPressTimer = undefined
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
    longPressTimer = window.setTimeout(async () => {
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
        experienceHost.value?.scrollIntoView({ behavior: 'smooth', block: 'center' })
    }, experience.longPressDuration)
}

function moveMemberLongPress(event: PointerEvent) {
    if (!longPressTimer) return
    if (Math.hypot(event.clientX - pressStart.x, event.clientY - pressStart.y) > 8) {
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

const messages = defineMessages({
    productTitle: {
        id: 'app.settings.about.product-title',
        defaultMessage: 'About {productName}',
    },
    productDescription: {
        id: 'app.settings.about.description',
        defaultMessage: 'Your last launcher.',
    },
    version: {
        id: 'app.settings.about.version',
        defaultMessage: 'Version {version}',
    },
    replayOnboarding: {
        id: 'app.settings.about.replay-onboarding',
        defaultMessage: 'Replay tour',
    },
    developmentTeam: {
        id: 'app.settings.about.development-team',
        defaultMessage: 'Development team',
    },
    communitySupport: {
        id: 'app.settings.about.community-support',
        defaultMessage: 'Project & community',
    },
    projectWebsite: {
        id: 'app.settings.about.project-website',
        defaultMessage: 'Project website',
    },
    repository: {
        id: 'app.settings.about.repository',
        defaultMessage: 'Source code',
    },
    reportIssue: {
        id: 'app.settings.about.report-issue',
        defaultMessage: 'Issues & feedback',
    },
    qqGroup: {
        id: 'app.settings.about.qq-group',
        defaultMessage: 'Player QQ group',
    },
    qqChannel: {
        id: 'app.settings.about.qq-channel',
        defaultMessage: 'QQ channel',
    },
    copyQqGroup: {
        id: 'app.settings.about.copy-qq-group',
        defaultMessage: 'Copy group number',
    },
    copiedQqGroup: {
        id: 'app.settings.about.copied-qq-group',
        defaultMessage: 'Group number copied',
    },
    afdian: {
        id: 'app.settings.about.afdian',
        defaultMessage: 'Support on Afdian',
    },
    afdianDescription: {
        id: 'app.settings.about.afdian-description',
        defaultMessage: 'Help support continued development',
    },
    licenseAttribution: {
        id: 'app.settings.about.license-attribution',
        defaultMessage: 'License & attribution',
    },
    attribution: {
        id: 'app.settings.about.attribution',
        defaultMessage:
            'Axolotl Launcher is a modified version of the open-source Modrinth codebase.',
    },
    notAffiliated: {
        id: 'app.settings.about.not-affiliated',
        defaultMessage:
            'Modrinth is a trademark of Rinth, Inc. Axolotl Launcher is not affiliated with or endorsed by Rinth, Inc.',
    },
    originalSource: {
        id: 'app.settings.about.original-source',
        defaultMessage: 'Original Modrinth source',
    },
    projectLicense: {
        id: 'app.settings.about.project-license',
        defaultMessage: 'Project license (GPL-3.0)',
    },
    copyingGuidelines: {
        id: 'app.settings.about.copying-guidelines',
        defaultMessage: 'Copying guidelines',
    },
    thirdPartyLicenses: {
        id: 'app.settings.about.third-party-licenses',
        defaultMessage: 'Third-party licenses',
    },
    contributors: {
        id: 'app.settings.about.contributors',
        defaultMessage: 'Contributors',
    },
    contributorsCount: {
        id: 'app.settings.about.contributors-count',
        defaultMessage: '{count, plural, one {# contributor} other {# contributors}}',
    },
})

const projectLinks = [
    {
        href: AxolotlBrandConfig.website,
        label: messages.projectWebsite,
        icon: GlobeIcon,
    },
    {
        href: AxolotlBrandConfig.repositoryUrl,
        label: messages.repository,
        icon: GithubIcon,
    },
    {
        href: AxolotlBrandConfig.supportUrl,
        label: messages.reportIssue,
        icon: IssuesIcon,
    },
    {
        href: AxolotlBrandConfig.qqChannelUrl,
        label: messages.qqChannel,
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
                    class="relative m-0 w-full overflow-hidden h-64 rounded-xl"
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
                            formatMessage(messages.productTitle, {
                                productName: AxolotlBrandConfig.productName,
                            })
                        }}
                    </h2>
                    <p class="m-0 mt-1 text-[var(--color-text-tertiary)]">
                        {{ formatMessage(messages.version, { version }) }}
                    </p>
                </div>
            </div>
            <p class="m-0 mt-3 text-center text-[var(--color-text-default)]">
                {{ formatMessage(messages.productDescription) }}
            </p>
        </section>

        <SettingsSection>
            <template #header>
                <h3
                    class="m-0 flex items-center gap-2 text-base font-semibold text-[var(--color-text-primary)]"
                >
                    <UsersIcon class="size-5 text-[var(--color-text-tertiary)]" />
                    {{ formatMessage(messages.developmentTeam) }}
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
                            <TeamMemberAvatar
                                :name="member.name"
                                :src="member.avatarUrl"
                                :remote="member.avatarRemote"
                            />
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
                    {{ formatMessage(messages.communitySupport) }}
                </h3>
            </template>
            <div class="grid gap-3 p-4 sm:grid-cols-2">
                <a
                    v-for="link in projectLinks"
                    :key="link.label"
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
                            ? formatMessage(messages.copiedQqGroup)
                            : formatMessage(messages.copyQqGroup)
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
                            {{ formatMessage(messages.qqGroup) }}
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
                                    ? formatMessage(messages.copiedQqGroup)
                                    : formatMessage(messages.copyQqGroup)
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
                            {{ formatMessage(messages.afdian) }}
                        </span>
                        <span class="block text-sm text-[var(--color-text-tertiary)]">
                            {{ formatMessage(messages.afdianDescription) }}
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
                    {{ formatMessage(messages.licenseAttribution) }}
                </h3>
            </template>
            <div class="flex flex-col gap-3 p-4">
                <p class="m-0 text-[var(--color-text-default)]">
                    {{ formatMessage(messages.attribution) }}
                </p>
                <p class="m-0 mt-2 text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.notAffiliated) }}
                </p>
                <div class="mt-3 flex flex-wrap gap-2">
                    <a
                        :href="licenseUrl"
                        target="_blank"
                        rel="noopener noreferrer"
                        class="inline-flex items-center gap-2 rounded-lg bg-surface-4 px-3 py-2 text-sm font-semibold text-[var(--color-text-primary)] transition-colors hover:bg-surface-5"
                    >
                        {{ formatMessage(messages.projectLicense) }}
                        <ExternalIcon class="size-4 text-[var(--color-text-tertiary)]" />
                    </a>
                    <a
                        :href="copyingUrl"
                        target="_blank"
                        rel="noopener noreferrer"
                        class="inline-flex items-center gap-2 rounded-lg bg-surface-4 px-3 py-2 text-sm font-semibold text-[var(--color-text-primary)] transition-colors hover:bg-surface-5"
                    >
                        {{ formatMessage(messages.copyingGuidelines) }}
                        <ExternalIcon class="size-4 text-[var(--color-text-tertiary)]" />
                    </a>
                    <a
                        :href="thirdPartyLicensesUrl"
                        target="_blank"
                        rel="noopener noreferrer"
                        class="inline-flex items-center gap-2 rounded-lg bg-surface-4 px-3 py-2 text-sm font-semibold text-[var(--color-text-primary)] transition-colors hover:bg-surface-5"
                    >
                        {{ formatMessage(messages.thirdPartyLicenses) }}
                        <ExternalIcon class="size-4 text-[var(--color-text-tertiary)]" />
                    </a>
                    <a
                        href="https://github.com/modrinth/code"
                        target="_blank"
                        rel="noopener noreferrer"
                        class="inline-flex items-center gap-2 rounded-lg bg-surface-4 px-3 py-2 text-sm font-semibold text-[var(--color-text-primary)] transition-colors hover:bg-surface-5"
                    >
                        {{ formatMessage(messages.originalSource) }}
                        <ExternalIcon class="size-4 text-[var(--color-text-tertiary)]" />
                    </a>
                </div>
            </div>
        </SettingsSection>

        <details class="group pt-4 about-settings-details">
            <summary
                class="flex cursor-pointer list-none items-center gap-2 text-base font-semibold text-[var(--color-text-primary)] [&::-webkit-details-marker]:hidden"
            >
                <UsersIcon class="size-5 text-[var(--color-text-tertiary)]" />
                <span>{{ formatMessage(messages.contributors) }}</span>
                <span
                    class="rounded-full bg-surface-4 px-2 py-0.5 text-xs text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.contributorsCount, { count: contributors.length }) }}
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
                    <Avatar
                        :src="contributor.avatarUrl"
                        :alt="contributor.name"
                        size="1.5rem"
                        circle
                        no-shadow
                        loading="lazy"
                    />
                    <span class="truncate text-sm text-[var(--color-text-default)]">{{
                        contributor.name
                    }}</span>
                </a>
            </div>
        </details>

        <div id="settings-target-about-replay-tour" tabindex="-1" class="flex flex-wrap gap-2">
            <Button type="base" @click="replayOnboarding?.('main')">
                {{ formatMessage(messages.replayOnboarding) }}
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
