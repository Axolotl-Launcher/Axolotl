<script setup>
/** The shared `ShareModal`, rendered inside the launcher window. */
import MastodonIcon from '@modrinth/assets/external/mastodon.svg?component'
import RedditIcon from '@modrinth/assets/external/reddit.svg?component'
import TwitterIcon from '@modrinth/assets/external/twitter.svg?component'
import ClipboardCopyIcon from '@modrinth/assets/icons/clipboard-copy.svg?component'
import ExternalIcon from '@modrinth/assets/icons/external.svg?component'
import GlobeIcon from '@modrinth/assets/icons/globe.svg?component'
import MailIcon from '@modrinth/assets/icons/mail.svg?component'
import ShareIcon from '@modrinth/assets/icons/share.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import ButtonLink from '@modrinth/ui/src/components/base/buttons/ButtonLink.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { computed, nextTick, ref } from 'vue'

import { QrcodeVue } from '../lib/qrcode'
import NewModal from './PreviewModal.vue'

const props = defineProps({
    header: {
        type: String,
        default: 'Share',
    },
    shareTitle: {
        type: String,
        default: 'Modrinth',
    },
    shareText: {
        type: String,
        default: null,
    },
    link: {
        type: Boolean,
        default: false,
    },
    openInNewTab: {
        type: Boolean,
        default: true,
    },
    noblur: {
        type: Boolean,
        default: false,
    },
    socialButtons: {
        type: Boolean,
        default: true,
    },
    onHide: {
        type: Function,
        default() {
            return () => {}
        },
    },
})

const shareModal = ref(null)
const { addNotification } = injectNotificationManager()

const qrCode = ref(null)
const qrImage = ref(null)
const content = ref(null)
const url = ref(null)
const canShare = ref(false)
const share = () => {
    navigator.share(
        props.link
            ? {
                  title: props.shareTitle.toString(),
                  text: props.shareText,
                  url: url.value,
              }
            : {
                  title: props.shareTitle.toString(),
                  text: content.value,
              },
    )
}

const show = async (passedContent) => {
    content.value = props.shareText ? `${props.shareText}\n\n${passedContent}` : passedContent
    shareModal.value.show()
    if (props.link) {
        url.value = passedContent
        nextTick(() => {
            console.log(qrCode.value)
            fetch(qrCode.value.getElementsByTagName('canvas')[0].toDataURL('image/png'))
                .then((res) => res.blob())
                .then((blob) => {
                    console.log(blob)
                    qrImage.value = blob
                })
        })
    }
    if (navigator.canShare({ title: props.shareTitle.toString(), text: content.value })) {
        canShare.value = true
    }
}

const copyImage = async () => {
    const item = new ClipboardItem({ 'image/png': qrImage.value })
    await navigator.clipboard.write([item])
}

const copyText = async () => {
    try {
        await navigator.clipboard.writeText(url.value ?? content.value)
        addNotification({
            type: 'success',
            title: 'Link copied',
            text: 'The link has been copied to your clipboard.',
        })
    } catch (error) {
        const message = error instanceof Error ? error.message : String(error)
        addNotification({
            type: 'error',
            title: 'Failed to copy text',
            text: message,
        })
    }
}

const sendEmail = computed(
    () =>
        `mailto:user@test.com
    ?subject=${encodeURIComponent(props.shareTitle)}
    &body=${encodeURIComponent(content.value)}`,
)

const targetParameter = computed(() => (props.openInNewTab ? '_blank' : '_self'))

const sendTweet = computed(
    () => `https://twitter.com/intent/tweet?text=${encodeURIComponent(content.value)}`,
)

const sendToot = computed(() => `https://tootpick.org/#text=${encodeURIComponent(content.value)}`)

const postOnReddit = computed(
    () =>
        `https://www.reddit.com/submit?title=${encodeURIComponent(props.shareTitle)}&text=${encodeURIComponent(
            content.value,
        )}`,
)

defineExpose({
    show,
})
</script>

<template>
    <NewModal ref="shareModal" :header="header" :noblur="noblur" :on-hide="onHide">
        <div class="flex flex-col items-center gap-2">
            <div
                :class="[
                    'flex items-center justify-center',
                    link ? 'flex-wrap gap-4' : 'flex-col gap-2',
                ]"
            >
                <div v-if="link" class="group relative shrink-0">
                    <div ref="qrCode">
                        <QrcodeVue
                            :value="url"
                            class="rounded-[var(--radius-md)] !bg-white"
                            margin="3"
                        />
                    </div>
                    <Button
                        v-tooltip="'Copy QR code'"
                        type="quiet"
                        circular
                        icon-only
                        class="absolute right-0 top-0 m-2"
                        aria-label="Copy QR code"
                        @click="copyImage"
                        ><ClipboardCopyIcon class="h-5 w-5" aria-hidden="true" />
                    </Button>
                </div>
                <StyledInput
                    v-else
                    v-model="content"
                    multiline
                    resize="vertical"
                    wrapper-class="h-full w-[30rem]"
                >
                    <template #right>
                        <Button
                            v-tooltip="'Copy Text'"
                            type="quiet"
                            circular
                            icon-only
                            aria-label="Copy Text"
                            class="absolute right-0 top-0 m-2"
                            @click="copyText"
                            ><ClipboardCopyIcon class="h-5 w-5" aria-hidden="true" />
                        </Button>
                    </template>
                </StyledInput>
                <div
                    v-if="link || socialButtons"
                    :class="[
                        'flex flex-col justify-center gap-2',
                        link ? 'w-64 max-w-full' : 'flex-grow',
                    ]"
                >
                    <button
                        v-if="link"
                        v-tooltip="'Copy Link'"
                        type="button"
                        aria-label="Copy Link"
                        class="hover:bg-button-bg-hover flex h-10 w-full cursor-pointer items-center justify-between gap-2 rounded-xl border-none bg-surface-4 px-3 pr-1.5 text-[var(--color-text-default)] transition-all hover:brightness-125 active:scale-95"
                        @click="copyText"
                    >
                        <span
                            class="min-w-0 cursor-pointer truncate text-left font-semibold text-[var(--color-text-default)]"
                        >
                            {{ url }}
                        </span>
                        <div class="grid h-10 w-10 place-content-center">
                            <ClipboardCopyIcon class="h-5 w-5" aria-hidden="true" />
                        </div>
                    </button>
                    <ButtonLink
                        v-if="link"
                        :href="url"
                        target="_blank"
                        rel="noopener noreferrer"
                        class="w-full"
                    >
                        Open in new tab
                        <ExternalIcon aria-hidden="true" />
                    </ButtonLink>
                    <div v-if="socialButtons" class="flex flex-row gap-1">
                        <Button
                            v-if="canShare"
                            v-tooltip="'Share'"
                            circular
                            icon-only
                            aria-label="Share"
                            @click="share"
                            ><ShareIcon aria-hidden="true" />
                        </Button>
                        <ButtonLink
                            v-tooltip="'Send as an email'"
                            circular
                            icon-only
                            label="Send as an email"
                            :href="sendEmail"
                            :target="targetParameter"
                        >
                            <MailIcon aria-hidden="true" />
                        </ButtonLink>
                        <ButtonLink
                            v-if="link"
                            v-tooltip="'Open link in browser'"
                            circular
                            icon-only
                            label="Open link in browser"
                            :target="targetParameter"
                            :href="url"
                        >
                            <GlobeIcon aria-hidden="true" />
                        </ButtonLink>
                        <ButtonLink
                            v-tooltip="'Toot about it'"
                            circular
                            icon-only
                            label="Toot about it"
                            :target="targetParameter"
                            :href="sendToot"
                        >
                            <MastodonIcon aria-hidden="true" />
                        </ButtonLink>
                        <ButtonLink
                            v-tooltip="'Tweet about it'"
                            circular
                            icon-only
                            label="Tweet about it"
                            :target="targetParameter"
                            :href="sendTweet"
                        >
                            <TwitterIcon aria-hidden="true" />
                        </ButtonLink>
                        <ButtonLink
                            v-tooltip="'Share on Reddit'"
                            circular
                            icon-only
                            label="Share on Reddit"
                            :target="targetParameter"
                            :href="postOnReddit"
                        >
                            <RedditIcon aria-hidden="true" />
                        </ButtonLink>
                    </div>
                </div>
            </div>
        </div>
    </NewModal>
</template>
