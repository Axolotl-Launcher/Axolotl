<script setup lang="ts">
import {
    CheckIcon,
    CopyIcon,
    DropdownIcon,
    LogInIcon,
    MessagesSquareIcon,
    WrenchIcon,
} from '@modrinth/assets'
import {
    Admonition,
    Button,
    ButtonLink,
    Collapsible,
    defineMessages,
    IntlFormatted,
    NewModal,
    useVIntl,
} from '@modrinth/ui'
import { computed, ref } from 'vue'

import { AxolotlBrandConfig } from '@/config'
import { login as login_flow, set_default_user } from '@/helpers/auth.js'
import i18n from '@/i18n.config'
import { handleSevereError } from '@/store/error.js'

import { findMinecraftAuthError, type MinecraftAuthError } from './minecraft-auth-errors'
import { translateMinecraftAuthErrorText } from './minecraft-auth-errors-zh'

const modal = ref<InstanceType<typeof NewModal>>()
const rawError = ref<string>('')
const matchedError = ref<MinecraftAuthError | null>(null)
const debugCollapsed = ref(true)
const copied = ref(false)
const loadingSignIn = ref(false)
const { formatMessage } = useVIntl()
const missingJavaProfile = computed(() =>
    rawError.value.includes('The Microsoft account does not own Minecraft: Java Edition'),
)

const messages = defineMessages({
    title: { id: 'app.minecraft-auth.title', defaultMessage: 'Sign in failed' },
    warning: {
        id: 'app.minecraft-auth.warning',
        defaultMessage:
            "We couldn't sign you in to your Microsoft account. This may be due to account restrictions or regional limitations.",
    },
    missingJavaProfile: {
        id: 'app.minecraft-auth.missing-java-profile',
        defaultMessage:
            "We couldn't find a Minecraft: Java Edition profile for this Microsoft account.",
    },
    purchaseMinecraft: {
        id: 'app.minecraft-auth.purchase-minecraft',
        defaultMessage:
            'If you need to buy Minecraft, you can purchase it <store-link>here</store-link>.',
    },
    activateJavaProfile: {
        id: 'app.minecraft-auth.activate-java-profile',
        defaultMessage:
            'If you already own Minecraft, sign in on the <minecraft-link>official Minecraft website</minecraft-link>, then try signing in to the launcher again.',
    },
    whatHappened: {
        id: 'app.minecraft-auth.what-happened',
        defaultMessage: 'What we think happened',
    },
    howToFix: { id: 'app.minecraft-auth.how-to-fix', defaultMessage: 'How to fix it' },
    unknownError: { id: 'app.minecraft-auth.unknown-error', defaultMessage: 'Unknown error' },
    unknownDescription: {
        id: 'app.minecraft-auth.unknown-description',
        defaultMessage:
            "We don't recognize this error and can't recommend specific steps to resolve it.",
    },
    tryMinecraftLoginBefore: {
        id: 'app.minecraft-auth.try-login-before',
        defaultMessage: 'Try visiting',
    },
    minecraftLogin: { id: 'app.minecraft-auth.minecraft-login', defaultMessage: 'Minecraft Login' },
    tryMinecraftLoginAfter: {
        id: 'app.minecraft-auth.try-login-after',
        defaultMessage:
            'and signing in, as it may prompt you with the necessary steps. You can also contact support and we can investigate further.',
    },
    contactSupport: {
        id: 'app.minecraft-auth.contact-support',
        defaultMessage: 'Contact support',
    },
    signInAgain: { id: 'app.minecraft-auth.sign-in-again', defaultMessage: 'Sign in again' },
    loginTrouble: {
        id: 'minecraft-login.trouble',
        defaultMessage: 'Having trouble?',
    },
    loginBrowser: {
        id: 'minecraft-login.browser',
        defaultMessage: 'Use browser login',
    },
    loginDeviceCode: {
        id: 'minecraft-login.device-code',
        defaultMessage: 'Use device code',
    },
    debugInformation: {
        id: 'app.minecraft-auth.debug-information',
        defaultMessage: 'Debug information',
    },
    copyDebugInfo: {
        id: 'app.minecraft-auth.copy-debug-info',
        defaultMessage: 'Copy debug information',
    },
    noErrorMessage: {
        id: 'app.minecraft-auth.no-error-message',
        defaultMessage: 'No error message.',
    },
})

const localizedMatchedError = computed(() => {
    if (!matchedError.value) return null
    const locale = i18n.global.locale.value
    return {
        whatHappened: translateMinecraftAuthErrorText(matchedError.value.whatHappened, locale),
        stepsToFix: matchedError.value.stepsToFix.map((step) =>
            translateMinecraftAuthErrorText(step, locale),
        ),
    }
})

function show(errorVal: { message?: string }) {
    rawError.value = errorVal?.message ?? String(errorVal)

    matchedError.value = findMinecraftAuthError(rawError.value)

    debugCollapsed.value = true
    modal.value?.show()
}

function hide() {
    modal.value?.hide()
}

defineExpose({
    show,
    hide,
})

async function signInAgain() {
    try {
        loadingSignIn.value = true
        const loggedIn = await login_flow({
            trouble: formatMessage(messages.loginTrouble),
            browserLogin: formatMessage(messages.loginBrowser),
            deviceCode: formatMessage(messages.loginDeviceCode),
        })
        if (loggedIn) {
            await set_default_user(loggedIn.account_id)
        }
        loadingSignIn.value = false
        modal.value?.hide()
    } catch (err) {
        loadingSignIn.value = false
        handleSevereError(err)
    }
}

const debugInfo = computed(() => rawError.value || formatMessage(messages.noErrorMessage))

async function copyToClipboard(text: string) {
    await navigator.clipboard.writeText(text)
    copied.value = true
    setTimeout(() => {
        copied.value = false
    }, 3000)
}
</script>

<template>
    <NewModal ref="modal" :header="formatMessage(messages.title)" :max-width="'548px'">
        <div class="flex flex-col gap-6">
            <Admonition
                type="warning"
                :body="
                    formatMessage(
                        missingJavaProfile ? messages.missingJavaProfile : messages.warning,
                    )
                "
            />

            <!-- Matched error details -->
            <div class="bg-surface-2 rounded-2xl p-4 px-5 flex flex-col gap-3">
                <ul
                    v-if="missingJavaProfile"
                    class="list-disc flex flex-col gap-2 m-0 pl-5 text-sm"
                >
                    <li>
                        <IntlFormatted :message-id="messages.purchaseMinecraft">
                            <template #store-link="{ children }">
                                <a
                                    class="text-info font-medium underline hover:underline"
                                    href="https://www.xbox.com/zh-CN/games/store/minecraft-java-bedrock-edition-for-pc/9nxp44l49shj"
                                >
                                    <component :is="() => children" />
                                </a>
                            </template>
                        </IntlFormatted>
                    </li>
                    <li>
                        <IntlFormatted :message-id="messages.activateJavaProfile">
                            <template #minecraft-link="{ children }">
                                <a
                                    class="text-info font-medium underline hover:underline"
                                    href="https://minecraft.net"
                                >
                                    <component :is="() => children" />
                                </a>
                            </template>
                        </IntlFormatted>
                    </li>
                </ul>
                <template v-else-if="localizedMatchedError">
                    <div class="flex flex-col gap-1.5">
                        <h3 class="text-base font-bold m-0">
                            {{ formatMessage(messages.whatHappened) }}
                        </h3>
                        <p class="text-sm text-[var(--color-text-tertiary)] m-0">
                            {{ localizedMatchedError.whatHappened }}
                        </p>
                    </div>

                    <div class="flex flex-col gap-1.5">
                        <h3 class="text-base font-bold m-0">
                            {{ formatMessage(messages.howToFix) }}
                        </h3>
                        <ol class="list-none flex flex-col gap-2 m-0 pl-0">
                            <li
                                v-for="(step, index) in localizedMatchedError.stepsToFix"
                                :key="index"
                                class="flex items-baseline gap-2"
                            >
                                <span
                                    class="inline-flex items-center justify-center shrink-0 w-5 h-5 rounded-full bg-surface-4 border border-solid border-surface-5 text-xs font-medium"
                                >
                                    {{ index + 1 }}
                                </span>
                                <span
                                    class="text-sm [&_a]:text-info [&_a]:font-medium [&_a]:underline"
                                    v-html="step"
                                />
                            </li>
                        </ol>
                    </div>
                </template>
                <template v-else>
                    <div class="flex flex-col gap-1.5">
                        <h3 class="text-base font-bold m-0">
                            {{ formatMessage(messages.unknownError) }}
                        </h3>
                        <p class="text-sm text-[var(--color-text-tertiary)] m-0">
                            {{ formatMessage(messages.unknownDescription) }}
                        </p>
                        <p class="text-sm text-[var(--color-text-tertiary)] m-0">
                            {{ formatMessage(messages.tryMinecraftLoginBefore) }}
                            <a
                                class="text-info font-medium underline hover:underline"
                                href="https://www.minecraft.net/en-us/login"
                                >{{ formatMessage(messages.minecraftLogin) }}</a
                            >
                            {{ formatMessage(messages.tryMinecraftLoginAfter) }}
                        </p>
                    </div>
                </template>
            </div>

            <div class="flex flex-col gap-2">
                <div class="w-full h-[1px] bg-surface-5"></div>

                <!-- Debug info -->
                <div class="overflow-clip">
                    <button
                        class="flex items-center justify-between w-full bg-transparent border-0 py-4 cursor-pointer"
                        @click="debugCollapsed = !debugCollapsed"
                    >
                        <span
                            class="flex items-center gap-2 text-[var(--color-text-primary)] font-extrabold m-0"
                        >
                            <WrenchIcon class="h-4 w-4" />
                            {{ formatMessage(messages.debugInformation) }}
                        </span>
                        <DropdownIcon
                            class="h-5 w-5 text-[var(--color-text-tertiary)] transition-transform"
                            :class="{ 'rotate-180': !debugCollapsed }"
                        />
                    </button>
                    <Collapsible :collapsed="debugCollapsed">
                        <div
                            class="p-3 bg-surface-2 rounded-2xl text-xs grid grid-cols-[1fr_auto] max-w-full items-start"
                        >
                            <div
                                class="m-0 p-0 rounded-none bg-transparent text-sm font-mono break-words overflow-auto"
                            >
                                {{ debugInfo }}
                            </div>
                            <Button
                                v-tooltip="formatMessage(messages.copyDebugInfo)"
                                circular
                                icon-only
                                :disabled="copied"
                                @click="copyToClipboard(debugInfo)"
                                ><template v-if="copied">
                                    <CheckIcon class="text-green" />
                                </template>
                                <template v-else> <CopyIcon /> </template>
                            </Button>
                        </div>
                    </Collapsible>
                </div>
            </div>
        </div>
        <template #actions>
            <div class="flex flex-wrap items-center justify-end gap-2">
                <ButtonLink :href="AxolotlBrandConfig.supportUrl" @click="modal?.hide()">
                    <MessagesSquareIcon /> {{ formatMessage(messages.contactSupport) }}
                </ButtonLink>
                <Button type="colored" color="brand" :disabled="loadingSignIn" @click="signInAgain">
                    <LogInIcon /> {{ formatMessage(messages.signInAgain) }}
                </Button>
            </div>
        </template>
    </NewModal>
</template>
