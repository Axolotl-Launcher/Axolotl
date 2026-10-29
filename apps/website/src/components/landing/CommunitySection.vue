<script setup lang="ts">
import CheckIcon from '@modrinth/assets/icons/check.svg?component'
import CopyIcon from '@modrinth/assets/icons/copy.svg?component'
import ExternalIcon from '@modrinth/assets/icons/external.svg?component'
import { defineMessages, useVIntl } from '@modrinth/ui'

import QqChannelIcon from '~/components/landing/QqChannelIcon.vue'
import QqIcon from '~/components/landing/QqIcon.vue'

const { formatMessage } = useVIntl()

const QQ_GROUP_NUMBER = '830619067'
const QQ_CHANNEL_URL = 'https://pd.qq.com/s/9nfp5rlz0'

const copied = ref(false)
let copyTimer: ReturnType<typeof setTimeout> | undefined

async function copyQqGroupNumber() {
    await navigator.clipboard.writeText(QQ_GROUP_NUMBER)
    copied.value = true
    clearTimeout(copyTimer)
    copyTimer = setTimeout(() => {
        copied.value = false
    }, 2000)
}

const messages = defineMessages({
    eyebrow: {
        id: 'axolotl-site.community.eyebrow',
        defaultMessage: 'Community',
    },
    title: {
        id: 'axolotl-site.community.title',
        defaultMessage: 'Join our community',
    },
    description: {
        id: 'axolotl-site.community.description',
        defaultMessage:
            'Join the official QQ group and QQ channel to get help, share feedback, and stay up to date with new releases.',
    },
    qqGroup: {
        id: 'axolotl-site.community.qq-group',
        defaultMessage: 'Official QQ group',
    },
    qqGroupDescription: {
        id: 'axolotl-site.community.qq-group-description',
        defaultMessage: 'Group number {number}. Get help and share feedback with other players.',
    },
    copyQqGroup: {
        id: 'axolotl-site.community.copy-qq-group',
        defaultMessage: 'Copy group number',
    },
    copiedQqGroup: {
        id: 'axolotl-site.community.copied-qq-group',
        defaultMessage: 'Copied!',
    },
    qqChannel: {
        id: 'axolotl-site.community.qq-channel',
        defaultMessage: 'QQ channel',
    },
    qqChannelDescription: {
        id: 'axolotl-site.community.qq-channel-description',
        defaultMessage: 'Official announcements, release updates, and channel events.',
    },
    qqChannelJoin: {
        id: 'axolotl-site.community.qq-channel-join',
        defaultMessage: 'Join the channel',
    },
})
</script>

<template>
    <section id="community" class="community-section" aria-labelledby="community-title">
        <div class="community-intro">
            <span class="text-xs font-extrabold uppercase tracking-[0.1em] text-brand">{{
                formatMessage(messages.eyebrow)
            }}</span>
            <h2 id="community-title">{{ formatMessage(messages.title) }}</h2>
            <p>{{ formatMessage(messages.description) }}</p>
        </div>

        <div
            class="community-cards mx-auto mt-10 flex w-[min(44rem,100%)] items-stretch justify-center"
        >
            <button
                type="button"
                class="community-card"
                :aria-label="formatMessage(messages.copyQqGroup)"
                @click="copyQqGroupNumber"
            >
                <span class="grid h-12 w-12 place-items-center text-[var(--color-contrast)]"
                    ><QqIcon
                /></span>
                <span class="text-[1.05rem] font-bold text-[var(--color-contrast)]">{{
                    formatMessage(messages.qqGroup)
                }}</span>
                <span class="m-0 text-sm leading-[1.6] text-[var(--color-secondary)]">
                    {{ formatMessage(messages.qqGroupDescription, { number: QQ_GROUP_NUMBER }) }}
                </span>
                <span class="community-action" :class="{ copied }" aria-live="polite">
                    <CheckIcon v-if="copied" />
                    <CopyIcon v-else />
                    {{
                        copied
                            ? formatMessage(messages.copiedQqGroup)
                            : formatMessage(messages.copyQqGroup)
                    }}
                </span>
            </button>

            <a class="community-card" :href="QQ_CHANNEL_URL" target="_blank" rel="noopener">
                <span class="grid h-12 w-12 place-items-center text-[var(--color-contrast)]"
                    ><QqChannelIcon
                /></span>
                <span class="text-[1.05rem] font-bold text-[var(--color-contrast)]">{{
                    formatMessage(messages.qqChannel)
                }}</span>
                <span class="m-0 text-sm leading-[1.6] text-[var(--color-secondary)]">
                    {{ formatMessage(messages.qqChannelDescription) }}
                </span>
                <span class="community-action">
                    {{ formatMessage(messages.qqChannelJoin) }}
                    <ExternalIcon />
                </span>
            </a>
        </div>
    </section>
</template>

<style scoped lang="scss">
.community-section {
    display: grid;
    grid-template-columns: minmax(0, 0.8fr) minmax(0, 1.2fr);
    gap: 4rem;
    width: min(76rem, calc(100% - 3rem));
    margin: 0 auto;
    padding: 7rem 0;
}

.community-intro {
    h2 {
        margin: 0.75rem 0 1rem;
        color: var(--color-contrast);
        font-size: clamp(2rem, 4vw, 3.25rem);
        line-height: 1.08;
    }

    p {
        max-width: 32rem;
        margin: 0;
        color: var(--color-secondary);
        font-size: 1.05rem;
        line-height: 1.7;
    }
}

.community-card {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.7rem;
    flex: 0 1 calc(50% - 0.75rem);
    max-width: 21.5rem;
    padding: 1.5rem;
    border: 1px solid var(--landing-border-color);
    border-radius: var(--radius-card);
    background: var(--landing-card-bg);
    box-shadow: var(--landing-card-shadow);
    backdrop-filter: blur(6px);
    -webkit-backdrop-filter: blur(6px);
    color: inherit;
    font: inherit;
    text-align: left;
    text-decoration: none;
    cursor: pointer;
    transition:
        background 160ms ease,
        border-color 160ms ease,
        box-shadow 160ms ease,
        transform 160ms ease;

    /* 错落叠放：左卡左倾、右卡右倾并压在上面 */
    &:first-child {
        z-index: 1;
        transform: rotate(-2deg);
    }

    &:last-child {
        z-index: 2;
        margin-left: -1.5rem;
        transform: rotate(4deg);
    }

    &:hover {
        border-color: color-mix(in srgb, var(--color-brand) 50%, var(--landing-border-color));
        box-shadow:
            var(--landing-card-shadow),
            0 0.45rem 1rem rgb(0 0 0 / 12%);
        transform: rotate(0deg);
    }

    &:focus-visible {
        outline: 2px solid var(--color-brand);
        outline-offset: 2px;
    }
}

.community-action {
    display: inline-flex;
    align-items: center;
    gap: 0.45rem;
    margin-top: auto;
    padding: 0.5rem 0.9rem;
    border: 1px solid var(--color-divider);
    border-radius: 0.75rem;
    background: var(--surface-2);
    color: var(--color-contrast);
    font-size: 0.82rem;
    font-weight: 600;
    transition:
        background 140ms ease,
        border-color 140ms ease,
        color 140ms ease;

    &.copied {
        border-color: color-mix(in srgb, var(--color-green) 50%, var(--color-divider));
        color: var(--color-green);
    }
}

@media (max-width: 800px) {
    .community-section {
        grid-template-columns: 1fr;
        gap: 2rem;
    }
}

@media (max-width: 560px) {
    .community-section {
        width: calc(100% - 2rem);
        padding: 4.5rem 0;
    }

    .community-cards {
        flex-direction: column;
        align-items: center;
        gap: 1rem;
    }

    .community-card {
        width: 100%;
        max-width: 24rem;

        &:first-child,
        &:last-child {
            margin-left: 0;
            transform: none;
        }
    }
}
</style>
