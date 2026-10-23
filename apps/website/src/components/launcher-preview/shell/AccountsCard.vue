<template>
    <div
        v-if="accounts.length === 0"
        class="mt-2 flex flex-col gap-3 rounded-xl border border-solid border-surface-5 bg-surface-4 p-3"
    >
        <span>{{ formatMessage(messages.notSignedIn) }}</span>
        <Button type="colored" color="brand" :disabled="loginDisabled" @click="login()"
            ><LogInIcon v-if="!loginDisabled" />
            <SpinnerIcon v-else class="animate-spin" />
            {{ formatMessage(messages.signInToMinecraft) }}
        </Button>
        <Button :disabled="loginDisabled" @click="showThirdPartyModal()"
            ><PlusIcon />
            {{ formatMessage(messages.addThirdPartyAccount) }}
        </Button>
        <Button
            data-onboarding-id="offline-account-entry"
            :disabled="loginDisabled"
            @click="showOfflineAccountModal()"
            ><PlusIcon />
            {{ formatMessage(messages.addOfflineAccount) }}
        </Button>
    </div>
    <Accordion
        v-else
        class="mt-2 w-full overflow-clip rounded-xl border border-solid border-surface-5 bg-surface-4"
        button-class="button-base w-full bg-transparent px-3 py-2 border-0 cursor-pointer"
        :open-by-default="false"
    >
        <template #title>
            <div class="flex w-full min-w-0 gap-2">
                <Avatar
                    size="36px"
                    :src="selectedAccount ? avatarUrl : axolotlLogo"
                    :pixelated="Boolean(selectedAccount)"
                    :unframed-natural-width="72"
                />
                <div class="flex w-full min-w-0 flex-col items-start">
                    <span class="w-full truncate text-left">{{
                        selectedAccount
                            ? selectedAccount.profile.name
                            : formatMessage(messages.selectAccount)
                    }}</span>
                    <span class="text-xs text-[var(--color-text-tertiary)]">
                        {{
                            selectedAccount?.account_type === 'offline'
                                ? formatMessage(messages.offlineAccount)
                                : selectedAccount?.account_type === 'yggdrasil'
                                  ? selectedAccount.yggdrasil?.server_name ||
                                    formatMessage(messages.thirdPartyAccount)
                                  : formatMessage(messages.minecraftAccount)
                        }}
                    </span>
                </div>
            </div>
        </template>
        <div class="border-0 border-t border-solid border-surface-5 bg-surface-4 pb-2 pt-1">
            <div
                v-for="account in accounts"
                :key="account.account_id"
                class="flex items-center gap-1"
            >
                <button
                    class="button-base flex min-w-0 flex-shrink flex-grow cursor-pointer items-center gap-2 overflow-clip border-0 bg-transparent p-2"
                    @click="setAccount(account)"
                >
                    <RadioButtonCheckedIcon
                        v-if="selectedAccount && selectedAccount.account_id === account.account_id"
                        class="h-5 w-5 shrink-0 text-brand"
                    />
                    <RadioButtonIcon
                        v-else
                        class="h-5 w-5 shrink-0 text-[var(--color-text-tertiary)]"
                    />
                    <Avatar
                        :src="getAccountAvatarUrl(account)"
                        size="24px"
                        pixelated
                        :unframed-natural-width="72"
                    />
                    <div class="flex min-w-0 flex-1 flex-col text-left">
                        <p
                            class="m-0 truncate text-left"
                            :class="
                                selectedAccount && selectedAccount.account_id === account.account_id
                                    ? 'font-semibold text-[var(--color-text-primary)]'
                                    : 'text-[var(--color-text-default)]'
                            "
                        >
                            {{ account.profile.name }}
                        </p>
                        <p
                            v-if="duplicateAccountNames.has(account.profile.name)"
                            class="m-0 truncate text-left text-xs text-[var(--color-text-tertiary)]"
                        >
                            {{ account.profile.id }}
                        </p>
                    </div>
                    <span
                        v-if="account.account_type === 'offline'"
                        class="shrink-0 text-xs text-[var(--color-text-tertiary)]"
                    >
                        {{ formatMessage(messages.offlineBadge) }}
                    </span>
                    <span
                        v-else-if="account.account_type === 'microsoft'"
                        class="shrink-0 text-xs text-[var(--color-text-tertiary)]"
                    >
                        {{ formatMessage(messages.officialBadge) }}
                    </span>
                    <span
                        v-else-if="account.account_type === 'yggdrasil'"
                        class="shrink-0 text-xs text-[var(--color-text-tertiary)]"
                    >
                        {{
                            account.yggdrasil?.server_name ||
                            formatMessage(messages.thirdPartyBadge)
                        }}
                    </span>
                </button>
                <div class="flex shrink-0 items-center">
                    <button
                        v-tooltip="formatMessage(messages.copyUuid)"
                        type="button"
                        class="button-base cursor-pointer border-0 bg-transparent p-1.5 text-[var(--color-text-tertiary)] hover:text-brand"
                        @click="copyAccountUuid(account)"
                    >
                        <CopyIcon />
                    </button>
                    <button
                        v-tooltip="formatMessage(messages.removeAccount)"
                        type="button"
                        class="button-base cursor-pointer border-0 bg-transparent p-1.5 text-[var(--color-text-tertiary)] hover:text-red"
                        @click="logout(account)"
                    >
                        <TrashIcon />
                    </button>
                </div>
            </div>
            <div class="flex flex-col gap-2 px-2 pt-2">
                <Button class="w-full" :disabled="loginDisabled" @click="login()"
                    ><PlusIcon />
                    {{ formatMessage(messages.addMicrosoftAccount) }}
                </Button>
                <Button class="w-full" :disabled="loginDisabled" @click="showThirdPartyModal()"
                    ><PlusIcon />
                    {{ formatMessage(messages.addThirdPartyAccount) }}
                </Button>
                <Button
                    class="w-full"
                    data-onboarding-id="offline-account-entry"
                    :disabled="loginDisabled"
                    @click="showOfflineAccountModal()"
                    ><PlusIcon />
                    {{ formatMessage(messages.addOfflineAccount) }}
                </Button>
            </div>
        </div>
    </Accordion>
    <MinecraftLoginModal ref="minecraftLoginModal" />
    <PreviewModal ref="offlineAccountModal" :header="formatMessage(messages.offlineModalTitle)">
        <div class="flex min-w-[22rem] flex-col gap-4">
            <p class="m-0 text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.offlineModalDescription) }}
            </p>
            <label class="flex flex-col gap-2 font-semibold">
                {{ formatMessage(messages.usernameLabel) }}
                <StyledInput
                    v-model="offlineUsername"
                    :disabled="loginDisabled"
                    :placeholder="formatMessage(messages.usernamePlaceholder)"
                    autocomplete="off"
                    maxlength="16"
                    @keyup.enter="addOfflineAccount()"
                />
            </label>
            <p
                v-if="offlineUsername.length > 0 && !offlineUsernameValid"
                class="m-0 text-sm text-red"
            >
                {{ formatMessage(messages.usernameValidation) }}
            </p>
            <p
                v-if="offlineUsernameContainsChinese"
                class="m-0 rounded-lg border border-solid border-orange bg-highlight-orange p-3 text-sm text-[var(--color-text-primary)]"
            >
                {{ formatMessage(messages.chineseUsernameWarning) }}
            </p>
            <Checkbox
                v-model="offlineCustomUuid"
                :disabled="loginDisabled"
                :label="formatMessage(messages.customUuidLabel)"
            />
            <Admonition
                v-if="offlineCustomUuid"
                type="warning"
                :body="formatMessage(messages.customUuidWarning)"
            />
            <label v-if="offlineCustomUuid" class="flex flex-col gap-2 font-semibold">
                {{ formatMessage(messages.customUuidInputLabel) }}
                <StyledInput
                    v-model="offlineUuid"
                    :disabled="loginDisabled"
                    :placeholder="formatMessage(messages.customUuidPlaceholder)"
                    autocomplete="off"
                    spellcheck="false"
                    maxlength="36"
                    @keyup.enter="addOfflineAccount()"
                />
            </label>
            <p
                v-if="offlineCustomUuid && offlineUuid.length > 0 && !offlineUuidValid"
                class="m-0 text-sm text-red"
            >
                {{ formatMessage(messages.customUuidValidation) }}
            </p>
            <Admonition
                v-if="offlineUuidDuplicate"
                type="critical"
                :body="formatMessage(messages.customUuidDuplicate)"
            />
            <div class="input-group push-right">
                <Button :disabled="loginDisabled" @click="offlineAccountModal?.hide()"
                    >{{ formatMessage(commonMessages.cancelButton) }}
                </Button>
                <Button
                    type="colored"
                    color="brand"
                    :disabled="loginDisabled || !offlineFormValid"
                    @click="addOfflineAccount()"
                    ><SpinnerIcon v-if="loginDisabled" class="animate-spin" />
                    <PlusIcon v-else />
                    {{ formatMessage(messages.createOfflineAccount) }}
                </Button>
            </div>
        </div>
    </PreviewModal>
    <PreviewModal
        ref="thirdPartyAccountModal"
        :header="formatMessage(messages.thirdPartyModalTitle)"
    >
        <div class="flex min-w-[24rem] flex-col gap-4">
            <p class="m-0 text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.thirdPartyModalDescription) }}
            </p>
            <Button class="w-full" :disabled="loginDisabled" @click="useLittleSkinPreset()"
                >{{ formatMessage(messages.useLittleSkin) }}
            </Button>
            <label class="flex flex-col gap-2 font-semibold">
                {{ formatMessage(messages.apiRootLabel) }}
                <StyledInput
                    v-model="thirdPartyApiRoot"
                    :disabled="loginDisabled"
                    :placeholder="formatMessage(messages.apiRootPlaceholder)"
                    inputmode="url"
                />
            </label>
            <label class="flex flex-col gap-2 font-semibold">
                {{ formatMessage(messages.accountLabel) }}
                <StyledInput
                    v-model="thirdPartyLogin"
                    :disabled="loginDisabled"
                    :placeholder="formatMessage(messages.accountPlaceholder)"
                    autocomplete="username"
                />
            </label>
            <label class="flex flex-col gap-2 font-semibold">
                {{ formatMessage(messages.passwordLabel) }}
                <StyledInput
                    v-model="thirdPartyPassword"
                    type="password"
                    :disabled="loginDisabled"
                    autocomplete="current-password"
                    @keyup.enter="addThirdPartyAccount()"
                />
            </label>
            <Checkbox
                v-model="rememberThirdPartyPassword"
                :disabled="loginDisabled"
                :label="formatMessage(messages.rememberPassword)"
            />
            <div class="input-group push-right">
                <Button :disabled="loginDisabled" @click="thirdPartyAccountModal?.hide()"
                    >{{ formatMessage(commonMessages.cancelButton) }}
                </Button>
                <Button
                    type="colored"
                    color="brand"
                    :disabled="loginDisabled || !thirdPartyFormValid"
                    @click="addThirdPartyAccount()"
                    ><SpinnerIcon v-if="loginDisabled" class="animate-spin" />
                    <LogInIcon v-else />
                    {{ formatMessage(messages.signInButton) }}
                </Button>
            </div>
        </div>
    </PreviewModal>
</template>

<script setup lang="ts">
import CopyIcon from '@modrinth/assets/icons/copy.svg?component'
import LogInIcon from '@modrinth/assets/icons/log-in.svg?component'
import PlusIcon from '@modrinth/assets/icons/plus.svg?component'
import RadioButtonIcon from '@modrinth/assets/icons/radio-button.svg?component'
import RadioButtonCheckedIcon from '@modrinth/assets/icons/radio-button-checked.svg?component'
import SpinnerIcon from '@modrinth/assets/icons/spinner.svg?component'
import TrashIcon from '@modrinth/assets/icons/trash.svg?component'
import Accordion from '@modrinth/ui/src/components/base/Accordion.vue'
import Admonition from '@modrinth/ui/src/components/base/Admonition.vue'
import Avatar from '@modrinth/ui/src/components/base/Avatar.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Checkbox from '@modrinth/ui/src/components/base/Checkbox.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, ref, watch } from 'vue'

import { playerHeadUrl } from '../lib/skin-head'
import { usePreview } from '../lib/state'
import type { MinecraftAccount } from '../lib/types'
import PreviewModal from '../overlays/PreviewModal.vue'
import { accountsMessages as messages } from './accounts-messages'
import MinecraftLoginModal from './MinecraftLoginModal.vue'

/**
 * Port of the app's AccountsCard (apps/app-frontend/src/components/ui/
 * AccountsCard.vue). The account list, switching and offline accounts work as
 * in the app. Microsoft and third-party sign-in open the real dialogs, which
 * report that signing in needs the desktop app: the preview contacts no
 * authentication service.
 */
const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const { accounts: accountsState } = usePreview()
const { accounts, selected: selectedAccount, skins, equippedSkin } = accountsState

const axolotlLogo = '/axolotl.png'
const LITTLE_SKIN_API_ROOT = 'https://littleskin.cn/api/yggdrasil'

const loginDisabled = ref(false)

const minecraftLoginModal = ref<InstanceType<typeof MinecraftLoginModal>>()
const offlineAccountModal = ref<InstanceType<typeof PreviewModal>>()
const thirdPartyAccountModal = ref<InstanceType<typeof PreviewModal>>()

const offlineUsername = ref('')
const offlineCustomUuid = ref(false)
const offlineUuid = ref('')
const offlineUuidDuplicate = ref(false)
const offlineUsernameValid = computed(() =>
    /^[\p{L}\p{N}_]{1,16}$/u.test(offlineUsername.value.trim()),
)
const offlineUuidValid = computed(() =>
    /^[a-fA-F0-9]{32}$/.test(offlineUuid.value.replaceAll('-', '')),
)
const offlineFormValid = computed(
    () => offlineUsernameValid.value && (!offlineCustomUuid.value || offlineUuidValid.value),
)
const offlineUsernameContainsChinese = computed(() =>
    /\p{Script=Han}/u.test(offlineUsername.value.trim()),
)

watch([offlineUuid, offlineCustomUuid], () => {
    offlineUuidDuplicate.value = false
})

const thirdPartyApiRoot = ref(LITTLE_SKIN_API_ROOT)
const thirdPartyLogin = ref('')
const thirdPartyPassword = ref('')
const rememberThirdPartyPassword = ref(true)
const thirdPartyFormValid = computed(
    () =>
        thirdPartyApiRoot.value.trim().length > 0 &&
        thirdPartyLogin.value.trim().length > 0 &&
        thirdPartyPassword.value.length > 0,
)

/**
 * Names that more than one account uses; the app shows the profile id under
 * those names so the rows stay distinguishable.
 */
const duplicateAccountNames = computed(() => {
    const counts = new Map<string, number>()
    for (const account of accounts.value) {
        counts.set(account.profile.name, (counts.get(account.profile.name) ?? 0) + 1)
    }
    return new Set(
        Array.from(counts.entries())
            .filter(([, count]) => count > 1)
            .map(([name]) => name),
    )
})

const STEVE_TEXTURE = '/launcher-preview/skins/steve.png'
/** Shown for the moment it takes the browser to render a head. */
const BLANK_IMAGE = 'data:image/gif;base64,R0lGODlhAQABAAAAACw='

/** The app's `defaultSteveHeadUrl`: Steve's head while no other head is rendered. */
const defaultSteveHeadUrl = computed(() => playerHeadUrl(STEVE_TEXTURE) ?? BLANK_IMAGE)

function equippedHeadUrl() {
    return equippedSkin.value ? playerHeadUrl(equippedSkin.value.texture) : null
}

/** Offline accounts have no profile skin, so the selected one shows the equipped skin. */
const avatarUrl = computed(() =>
    selectedAccount.value ? (equippedHeadUrl() ?? defaultSteveHeadUrl.value) : axolotlLogo,
)

function getAccountAvatarUrl(account: MinecraftAccount) {
    if (account.account_id === selectedAccount.value?.account_id) {
        return equippedHeadUrl() ?? defaultSteveHeadUrl.value
    }
    return defaultSteveHeadUrl.value
}

function setAccount(account: MinecraftAccount) {
    accountsState.select(account.account_id)
}

async function login() {
    if (loginDisabled.value) return
    loginDisabled.value = true
    try {
        minecraftLoginModal.value?.showDeviceLogin()
    } finally {
        loginDisabled.value = false
    }
}

function showOfflineAccountModal() {
    offlineUsername.value = ''
    offlineUuid.value = ''
    offlineCustomUuid.value = false
    offlineUuidDuplicate.value = false
    offlineAccountModal.value?.show()
}

function addOfflineAccount() {
    if (!offlineFormValid.value || loginDisabled.value) return
    loginDisabled.value = true
    const result = accountsState.addOffline(
        offlineUsername.value.trim(),
        offlineCustomUuid.value ? offlineUuid.value.replaceAll('-', '') : undefined,
    )
    loginDisabled.value = false
    if (result.status === 'duplicate-uuid') {
        offlineUuidDuplicate.value = true
        return
    }
    if (result.status !== 'ok') {
        handleError(new Error(formatMessage(messages.usernameValidation)))
        return
    }
    offlineAccountModal.value?.hide()
    setAccount(result.account)
}

function logout(account: MinecraftAccount) {
    accountsState.remove(account.account_id)
    if (!selectedAccount.value && accounts.value.length > 0) {
        setAccount(accounts.value[0])
    }
}

async function copyAccountUuid(account: MinecraftAccount) {
    try {
        await navigator.clipboard.writeText(account.profile.id)
    } catch (error) {
        handleError(error as Error)
    }
}

function showThirdPartyModal() {
    thirdPartyAccountModal.value?.show()
}

function useLittleSkinPreset() {
    thirdPartyApiRoot.value = LITTLE_SKIN_API_ROOT
}

/**
 * The app sends the login to the server's Yggdrasil API (`begin_yggdrasil_login`)
 * and reports a failure through the error notification; the preview has no
 * server to ask, so it reports that, and keeps the form as the app does.
 */
function addThirdPartyAccount() {
    if (!thirdPartyFormValid.value || loginDisabled.value) return
    handleError(new Error(formatMessage(messages.thirdPartyDesktopOnly)))
}

/** Port of the app's `setEquippedSkin`: the head follows the skin a page just equipped. */
function setEquippedSkin(skin: {
    texture_key: string
    texture: string
    variant: string
    name?: string
}) {
    if (!skins.value.some((entry) => entry.texture_key === skin.texture_key)) {
        skins.value = [
            ...skins.value,
            {
                texture_key: skin.texture_key,
                name: skin.name ?? '',
                section: null,
                variant: skin.variant === 'SLIM' ? 'SLIM' : 'CLASSIC',
                texture: skin.texture,
                cape_id: null,
                source: 'custom',
                is_equipped: false,
            },
        ]
    }
    accountsState.equipSkin(skin.texture_key)
}

defineExpose({ login, loginDisabled, setEquippedSkin })
</script>
