<template>
    <div
        v-if="offline"
        class="flex flex-col gap-1 bg-highlight-orange border border-solid border-orange rounded-xl p-3 mt-2"
    >
        <span class="font-semibold text-[var(--color-text-primary)]">{{
            formatMessage(messages.offlineMode)
        }}</span>
        <span class="text-sm text-[var(--color-text-tertiary)]">{{
            formatMessage(messages.offlineModeDescription)
        }}</span>
        <Button class="mt-1" :disabled="refreshingNetwork" @click="refreshNetworkStatus()"
            ><SpinnerIcon v-if="refreshingNetwork" class="animate-spin" />
            <RefreshCwIcon v-else />
            {{ formatMessage(messages.refreshNetworkStatus) }}
        </Button>
    </div>
    <div
        v-if="accounts.length === 0"
        class="flex flex-col gap-3 bg-surface-4 border border-solid border-surface-5 rounded-xl p-3 mt-2"
    >
        <span>{{ formatMessage(messages.notSignedIn) }}</span>
        <Button
            v-if="!offline"
            type="colored"
            color="brand"
            :disabled="loginDisabled"
            @click="login()"
            ><LogInIcon v-if="!loginDisabled" />
            <SpinnerIcon v-else class="animate-spin" />
            {{ formatMessage(messages.signInToMinecraft) }}
        </Button>
        <Button v-if="!offline" :disabled="loginDisabled" @click="showYggdrasilAccountModal()"
            ><PlusIcon />
            {{ formatMessage(messages.addThirdPartyAccount) }}
        </Button>
        <Button
            data-onboarding-id="offline-account-entry"
            :disabled="loginDisabled"
            :aria-disabled="antiPiracyStatus.restricted"
            :class="{ 'opacity-50': antiPiracyStatus.restricted }"
            @click="showOfflineAccountModal()"
            ><PlusIcon />
            {{ formatMessage(messages.addOfflineAccount) }}
        </Button>
    </div>
    <Accordion
        v-else
        class="w-full mt-2 bg-surface-4 border border-solid border-surface-5 rounded-xl overflow-clip"
        button-class="button-base w-full bg-transparent px-3 py-2 border-0 cursor-pointer"
        :open-by-default="false"
    >
        <template #title>
            <div class="flex gap-2 w-full min-w-0">
                <Avatar
                    size="36px"
                    :src="selectedAccount ? avatarUrl : axolotlLogo"
                    :pixelated="Boolean(selectedAccount)"
                    :unframed-natural-width="72"
                />
                <div class="flex flex-col items-start w-full min-w-0">
                    <span class="truncate w-full text-left">{{
                        selectedAccount
                            ? selectedAccount.profile.name
                            : formatMessage(messages.selectAccount)
                    }}</span>
                    <span class="text-[var(--color-text-tertiary)] text-xs">
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
        <div class="bg-surface-4 pt-1 pb-2 border-0 border-t border-solid border-surface-5">
            <template v-if="accounts.length > 0">
                <div
                    v-for="account in accounts"
                    :key="account.account_id"
                    class="flex gap-1 items-center"
                >
                    <button
                        class="flex items-center flex-shrink flex-grow overflow-clip gap-2 p-2 border-0 bg-transparent cursor-pointer button-base min-w-0"
                        :aria-disabled="
                            account.account_type === 'offline' && antiPiracyStatus.restricted
                        "
                        :class="{
                            'opacity-50':
                                account.account_type === 'offline' && antiPiracyStatus.restricted,
                        }"
                        @click="setAccount(account)"
                    >
                        <RadioButtonCheckedIcon
                            v-if="
                                selectedAccount && selectedAccount.account_id === account.account_id
                            "
                            class="w-5 h-5 text-brand shrink-0"
                        />
                        <RadioButtonIcon
                            v-else
                            class="w-5 h-5 text-[var(--color-text-tertiary)] shrink-0"
                        />
                        <Avatar
                            :src="getAccountAvatarUrl(account)"
                            size="24px"
                            pixelated
                            :unframed-natural-width="72"
                        />
                        <div class="flex flex-1 min-w-0 flex-col text-left">
                            <p
                                class="m-0 truncate text-left"
                                :class="
                                    selectedAccount &&
                                    selectedAccount.account_id === account.account_id
                                        ? 'text-[var(--color-text-primary)] font-semibold'
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
                            class="text-[var(--color-text-tertiary)] text-xs shrink-0"
                        >
                            {{ formatMessage(messages.offlineBadge) }}
                        </span>
                        <span
                            v-else-if="account.account_type === 'microsoft'"
                            class="text-[var(--color-text-tertiary)] text-xs shrink-0"
                        >
                            {{ formatMessage(messages.officialBadge) }}
                        </span>
                        <span
                            v-else-if="account.account_type === 'yggdrasil'"
                            class="text-[var(--color-text-tertiary)] text-xs shrink-0"
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
                            class="button-base border-0 bg-transparent p-1.5 cursor-pointer text-[var(--color-text-tertiary)] hover:text-brand"
                            @click="copyAccountUuid(account)"
                        >
                            <CopyIcon />
                        </button>
                        <button
                            v-tooltip="formatMessage(messages.removeAccount)"
                            type="button"
                            class="button-base border-0 bg-transparent p-1.5 cursor-pointer text-[var(--color-text-tertiary)] hover:text-red"
                            @click="logout(account)"
                        >
                            <TrashIcon />
                        </button>
                    </div>
                </div>
            </template>
            <div class="flex flex-col gap-2 px-2 pt-2">
                <Button
                    v-if="accounts.length > 0 && !offline"
                    class="w-full"
                    :disabled="loginDisabled"
                    @click="login()"
                    ><PlusIcon />
                    {{ formatMessage(messages.addMicrosoftAccount) }}
                </Button>
                <Button
                    v-if="accounts.length > 0 && !offline"
                    class="w-full"
                    :disabled="loginDisabled"
                    @click="showYggdrasilAccountModal()"
                    ><PlusIcon />
                    {{ formatMessage(messages.addThirdPartyAccount) }}
                </Button>
                <Button
                    v-if="accounts.length > 0"
                    class="w-full"
                    data-onboarding-id="offline-account-entry"
                    :disabled="loginDisabled"
                    :aria-disabled="antiPiracyStatus.restricted"
                    :class="{ 'opacity-50': antiPiracyStatus.restricted }"
                    @click="showOfflineAccountModal()"
                    ><PlusIcon />
                    {{ formatMessage(messages.addOfflineAccount) }}
                </Button>
            </div>
        </div>
    </Accordion>
    <MinecraftLoginModal ref="minecraftLoginModal" @complete="onMicrosoftLogin" />
    <NewModal ref="offlineAccountModal" :header="formatMessage(messages.offlineModalTitle)">
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
    </NewModal>
    <NewModal ref="yggdrasilAccountModal" :header="formatMessage(messages.thirdPartyModalTitle)">
        <div class="flex min-w-[24rem] flex-col gap-4">
            <p class="m-0 text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.thirdPartyModalDescription) }}
            </p>
            <div v-if="savedYggdrasilLogins.length > 0" class="flex flex-col gap-2">
                <span class="font-semibold">{{ formatMessage(messages.savedLogins) }}</span>
                <div
                    v-for="savedLogin in savedYggdrasilLogins"
                    :key="`${savedLogin.api_root}:${savedLogin.login}`"
                    class="flex items-center gap-1 rounded-xl bg-surface-3 p-1"
                >
                    <button
                        class="flex min-w-0 flex-grow flex-col items-start border-0 bg-transparent px-3 py-2 text-left cursor-pointer"
                        :disabled="loginDisabled"
                        @click="selectSavedYggdrasilLogin(savedLogin)"
                    >
                        <span
                            class="w-full truncate font-semibold text-[var(--color-text-default)]"
                            >{{ savedLogin.login }}</span
                        >
                        <span class="w-full truncate text-xs text-[var(--color-text-tertiary)]">{{
                            savedLogin.api_root
                        }}</span>
                    </button>
                    <IconButton
                        v-tooltip="formatMessage(messages.removeSavedLogin)"
                        :label="formatMessage(messages.removeSavedLogin)"
                        type="quiet"
                        color="red"
                        interaction="filled"
                        :disabled="loginDisabled"
                        @click="removeSavedYggdrasilLogin(savedLogin)"
                    >
                        <TrashIcon />
                    </IconButton>
                </div>
            </div>
            <Button class="w-full" :disabled="loginDisabled" @click="useLittleSkinPreset()"
                >{{ formatMessage(messages.useLittleSkin) }}
            </Button>
            <label class="flex flex-col gap-2 font-semibold">
                {{ formatMessage(messages.apiRootLabel) }}
                <StyledInput
                    v-model="yggdrasilApiRoot"
                    :disabled="loginDisabled"
                    :placeholder="formatMessage(messages.apiRootPlaceholder)"
                    inputmode="url"
                    @blur="loadRememberedYggdrasilPassword()"
                />
            </label>
            <label class="flex flex-col gap-2 font-semibold">
                {{ formatMessage(messages.accountLabel) }}
                <StyledInput
                    v-model="yggdrasilLogin"
                    :disabled="loginDisabled"
                    :placeholder="formatMessage(messages.accountPlaceholder)"
                    autocomplete="username"
                    @blur="loadRememberedYggdrasilPassword()"
                />
            </label>
            <label class="flex flex-col gap-2 font-semibold">
                {{ formatMessage(messages.passwordLabel) }}
                <StyledInput
                    v-model="yggdrasilPassword"
                    type="password"
                    :disabled="loginDisabled"
                    autocomplete="current-password"
                    @keyup.enter="addYggdrasilAccount()"
                />
            </label>
            <Checkbox
                v-model="rememberYggdrasilPassword"
                :disabled="loginDisabled"
                :label="formatMessage(messages.rememberPassword)"
            />
            <div class="input-group push-right">
                <Button :disabled="loginDisabled" @click="yggdrasilAccountModal?.hide()"
                    >{{ formatMessage(commonMessages.cancelButton) }}
                </Button>
                <Button
                    type="colored"
                    color="brand"
                    :disabled="loginDisabled || !yggdrasilFormValid"
                    @click="addYggdrasilAccount()"
                    ><SpinnerIcon v-if="loginDisabled" class="animate-spin" />
                    <LogInIcon v-else />
                    {{ formatMessage(messages.signInButton) }}
                </Button>
            </div>
        </div>
    </NewModal>
    <NewModal ref="yggdrasilProfileModal" :header="formatMessage(messages.selectProfileTitle)">
        <div class="flex min-w-[22rem] flex-col gap-2">
            <p class="m-0 mb-2 text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.selectProfileDescription) }}
            </p>
            <Button
                v-for="profile in pendingYggdrasilProfiles"
                :key="profile.id"
                class="w-full"
                :disabled="loginDisabled"
                @click="selectYggdrasilProfile(profile.id)"
                ><SpinnerIcon v-if="loginDisabled" class="animate-spin" />
                <RadioButtonIcon v-else />
                {{ profile.name }}
            </Button>
        </div>
    </NewModal>
</template>

<script setup lang="ts">
import {
    CopyIcon,
    LogInIcon,
    PlusIcon,
    RadioButtonCheckedIcon,
    RadioButtonIcon,
    RefreshCwIcon,
    SpinnerIcon,
    TrashIcon,
} from '@modrinth/assets'
import {
    Accordion,
    Admonition,
    Avatar,
    Button,
    Checkbox,
    commonMessages,
    defineMessages,
    IconButton,
    injectNotificationManager,
    NewModal,
    StyledInput,
    useVIntl,
} from '@modrinth/ui'
import { useQueryClient } from '@tanstack/vue-query'
import { listen } from '@tauri-apps/api/event'
import type { Ref } from 'vue'
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { useRoute } from 'vue-router'

import axolotlLogo from '@/assets/axolotl.png'
import steveSkinTexture from '@/assets/skins/steve.png?inline'
import MinecraftLoginModal from '@/components/ui/MinecraftLoginModal.vue'
import {
    isOfflineAccountRestrictedError,
    useAntiPiracyStatus,
} from '@/composables/useAntiPiracyStatus'
import { useNetworkStatus } from '@/composables/useNetworkStatus'
import { preferredOnlineAccountId } from '@/helpers/account-selection'
import { compareMinecraftAccounts } from '@/helpers/accounts'
import { trackEvent } from '@/helpers/analytics'
import {
    add_offline_user,
    begin_yggdrasil_login,
    delete_yggdrasil_password,
    finish_yggdrasil_login,
    get_default_user,
    get_yggdrasil_password,
    list_yggdrasil_saved_logins,
    login as loginToMinecraft,
    remove_user,
    set_default_user,
    set_yggdrasil_password,
    users,
} from '@/helpers/auth'
import { process_listener } from '@/helpers/events'
import { mojangAuthKeys } from '@/helpers/mojang-auth/monitor'
import {
    getPlayerHeadUrl,
    retainPlayerHeadTextures,
} from '@/helpers/rendering/batch-skin-renderer.ts'
import type { Skin } from '@/helpers/skins'
import { get_available_skins } from '@/helpers/skins'
import { handleSevereError, useError } from '@/store/error.js'

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const { offline, refreshBrowserOffline } = useNetworkStatus()
const queryClient = useQueryClient()
const route = useRoute()
const { status: antiPiracyStatus, refresh: refreshAntiPiracyStatus } = useAntiPiracyStatus()
const errorStore = useError()
const refreshingNetwork = ref(false)

/**
 * Re-checks session server reachability on demand so users can leave offline
 * mode without restarting the launcher. Goes through the shared
 * application-owned Mojang query so the reachability state and the auth
 * warning banner stay consistent.
 */
async function refreshNetworkStatus() {
    if (refreshingNetwork.value) return
    refreshingNetwork.value = true
    try {
        refreshBrowserOffline()
        await nextTick()
        await queryClient.refetchQueries(
            { queryKey: mojangAuthKeys.status, exact: true },
            { cancelRefetch: false },
        )
    } finally {
        refreshingNetwork.value = false
    }
}

const emit = defineEmits<{
    change: []
}>()

type MinecraftCredential = {
    account_id: string
    account_type: 'microsoft' | 'offline' | 'yggdrasil'
    profile: {
        id: string
        name: string
        skins?: Array<{
            state: string
            url: string
            variant: Skin['variant']
            textureKey?: string
        }>
    }
    yggdrasil?: {
        api_root: string
        server_name: string
        login: string
    }
}

type YggdrasilProfile = {
    id: string
    name: string
}

type SavedYggdrasilLogin = {
    api_root: string
    login: string
}

type YggdrasilLoginResult =
    | { status: 'complete'; credentials: MinecraftCredential }
    | { status: 'select_profile'; flow_id: string; profiles: YggdrasilProfile[] }

const LITTLE_SKIN_API_ROOT = 'https://littleskin.cn/api/yggdrasil'

const accounts: Ref<MinecraftCredential[]> = ref([])
const loginDisabled = ref(false)
const defaultUser = ref<string | undefined>()
const equippedSkin = ref<Skin | null>(null)
const accountChangeRevision = ref(0)
const headUrlCache = ref(new Map<string, string>())
const accountHeadUrlCache = ref(new Map<string, string>())
const accountHeadTextureKeyCache = ref(new Map<string, string>())
let refreshGeneration = 0
let headRefreshTimer: ReturnType<typeof setTimeout> | undefined
let defaultUserUpdateQueue = Promise.resolve()
const minecraftLoginModal = ref<InstanceType<typeof MinecraftLoginModal> | null>(null)
const offlineAccountModal = ref<InstanceType<typeof NewModal> | null>(null)
const offlineUsername = ref('')
const offlineCustomUuid = ref(false)
const offlineUuid = ref('')
const offlineUuidDuplicate = ref(false)
const offlineUsernameValid = computed(() =>
    /^[\p{L}\p{N}_]{1,16}$/u.test(offlineUsername.value.trim()),
)
const offlineUuidValid = computed(() =>
    /^[a-fA-F0-9]{32}$/u.test(offlineUuid.value.replaceAll('-', '')),
)
const offlineFormValid = computed(
    () => offlineUsernameValid.value && (!offlineCustomUuid.value || offlineUuidValid.value),
)
watch([offlineUuid, offlineCustomUuid], () => {
    offlineUuidDuplicate.value = false
})
const offlineUsernameContainsChinese = computed(() =>
    /\p{Script=Han}/u.test(offlineUsername.value.trim()),
)
const yggdrasilAccountModal = ref<InstanceType<typeof NewModal> | null>(null)
const yggdrasilProfileModal = ref<InstanceType<typeof NewModal> | null>(null)
const yggdrasilApiRoot = ref(LITTLE_SKIN_API_ROOT)
const yggdrasilLogin = ref('')
const yggdrasilPassword = ref('')
const rememberYggdrasilPassword = ref(true)
const savedYggdrasilLogins = ref<SavedYggdrasilLogin[]>([])
const pendingYggdrasilFlowId = ref<string | undefined>()
const pendingYggdrasilProfiles = ref<YggdrasilProfile[]>([])
const yggdrasilFormValid = computed(
    () =>
        yggdrasilApiRoot.value.trim().length > 0 &&
        yggdrasilLogin.value.trim().length > 0 &&
        yggdrasilPassword.value.length > 0,
)

function createSkinHeadDataUrl(textureUrl: string) {
    const escapedTextureUrl = textureUrl
        .replaceAll('&', '&amp;')
        .replaceAll('"', '&quot;')
        .replaceAll('<', '&lt;')
        .replaceAll('>', '&gt;')
    const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" viewBox="0 0 8 8" shape-rendering="crispEdges"><image href="${escapedTextureUrl}" x="-8" y="-8" width="64" height="64" style="image-rendering:pixelated"/><image href="${escapedTextureUrl}" x="-40" y="-8" width="64" height="64" style="image-rendering:pixelated"/></svg>`

    return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`
}

const defaultSteveHeadUrl = createSkinHeadDataUrl(steveSkinTexture)

const HEAD_REFRESH_RETRY_DELAYS = [1500, 5000, 15000, 30000] as const
const HEAD_REFRESH_CONTINUOUS_DELAY = 60_000

function hasResolvedAccountHead(account: MinecraftCredential) {
    const skin = getAccountSkin(account)
    return Boolean(
        skin &&
        accountHeadUrlCache.value.has(account.account_id) &&
        accountHeadTextureKeyCache.value.get(account.account_id) === skin.texture_key,
    )
}

function hasMissingAccountHeads() {
    return accounts.value.some(
        (account) => account.account_type !== 'offline' && !hasResolvedAccountHead(account),
    )
}

function clearHeadRefreshRetry() {
    if (headRefreshTimer !== undefined) {
        clearTimeout(headRefreshTimer)
        headRefreshTimer = undefined
    }
}

function scheduleHeadRefreshRetry(generation: number, attempt: number) {
    if (offline.value || generation !== refreshGeneration) return
    const delay = HEAD_REFRESH_RETRY_DELAYS[attempt] ?? HEAD_REFRESH_CONTINUOUS_DELAY

    clearHeadRefreshRetry()
    headRefreshTimer = setTimeout(() => {
        headRefreshTimer = undefined
        if (generation !== refreshGeneration) return
        void refreshValues(Math.min(attempt + 1, HEAD_REFRESH_RETRY_DELAYS.length)).catch(
            (error) => {
                console.warn('Failed to refresh account heads:', error)
            },
        )
    }, delay)
}

async function refreshValues(
    headRefreshAttempt = 0,
    preferOnlineAccount = false,
    cachedOnly = false,
) {
    clearHeadRefreshRetry()
    const generation = ++refreshGeneration
    const [selectedUser, userList] = await Promise.all([
        get_default_user(offline.value).catch(handleError),
        users(offline.value, true).catch(handleError),
    ])
    if (generation !== refreshGeneration) return
    // The Rust backend returns a plain array that structurally matches
    // MinecraftCredential but the TS types from the Tauri IPC bridge do not
    // carry this refinement. The double cast is deliberate — the shape is
    // correct and verified at runtime by the backend.
    if (Array.isArray(userList)) {
        accounts.value = [...(userList as unknown as MinecraftCredential[])]
    }
    accounts.value.sort(compareMinecraftAccounts)
    let resolvedSelectedUser = selectedUser
    if (preferOnlineAccount && selectedUser) {
        resolvedSelectedUser = preferredOnlineAccountId(selectedUser, accounts.value)
    }
    defaultUser.value = resolvedSelectedUser
    if (resolvedSelectedUser && resolvedSelectedUser !== selectedUser) {
        await persistDefaultUser(resolvedSelectedUser)
        if (generation !== refreshGeneration) return
    }
    // Cached account metadata can resolve Suspense before any image download,
    // token refresh or online profile/skin request finishes.
    const localHeads = renderAccountHeads(accounts.value, generation)
    if (cachedOnly) {
        void localHeads.catch(console.warn)
        return
    }
    const updatedUsers = await users(offline.value).catch(handleError)
    if (generation !== refreshGeneration) return
    if (Array.isArray(updatedUsers)) {
        accounts.value = [...(updatedUsers as unknown as MinecraftCredential[])].sort(
            compareMinecraftAccounts,
        )
    }
    await Promise.all([localHeads, renderAccountHeads(accounts.value, generation)])
    if (generation !== refreshGeneration) return
    try {
        const skins = await get_available_skins()
        if (generation !== refreshGeneration) return
        equippedSkin.value = skins.find((skin) => skin.is_equipped) ?? null

        if (equippedSkin.value) {
            try {
                const headUrl = await getPlayerHeadUrl(equippedSkin.value)
                if (generation !== refreshGeneration) return
                headUrlCache.value = new Map(headUrlCache.value).set(
                    equippedSkin.value.texture_key,
                    headUrl,
                )
                if (resolvedSelectedUser) {
                    const selectedAccountSkin = getAccountSkin(
                        accounts.value.find(
                            (account) => account.account_id === resolvedSelectedUser,
                        ),
                    )
                    cacheAccountHead(
                        resolvedSelectedUser,
                        selectedAccountSkin ?? equippedSkin.value,
                        headUrl,
                    )
                }
            } catch (error) {
                console.warn('Failed to get head render for equipped skin:', error)
            }
        }
    } catch {
        equippedSkin.value = null
    }

    if (hasMissingAccountHeads()) {
        scheduleHeadRefreshRetry(generation, headRefreshAttempt)
    }
}

async function setEquippedSkin(skin: Skin) {
    const selectedUser = defaultUser.value
    equippedSkin.value = skin

    try {
        const headUrl = await getPlayerHeadUrl(skin)
        headUrlCache.value = new Map(headUrlCache.value).set(skin.texture_key, headUrl)
        if (selectedUser) {
            cacheAccountHead(selectedUser, skin, headUrl)
        }
    } catch (error) {
        console.warn('Failed to get head render for equipped skin:', error)
    }
}

function setLoginDisabled(value: boolean) {
    loginDisabled.value = value
}

defineExpose({
    accountChangeRevision,
    login,
    refreshValues,
    setEquippedSkin,
    setLoginDisabled,
    loginDisabled,
})

watch(
    () => [
        ...accounts.value.map((account) => getAccountSkin(account)?.texture_key),
        equippedSkin.value?.texture_key,
    ],
    (keys, _, onCleanup) => {
        onCleanup(retainPlayerHeadTextures(keys.filter((key): key is string => !!key)))
        const accountIds = new Set(accounts.value.map((account) => account.account_id))
        accountHeadUrlCache.value = new Map(
            [...accountHeadUrlCache.value].filter(([id]) => accountIds.has(id)),
        )
        accountHeadTextureKeyCache.value = new Map(
            [...accountHeadTextureKeyCache.value].filter(([id]) => accountIds.has(id)),
        )
        headUrlCache.value = new Map(
            [...headUrlCache.value].filter(([key]) => key === equippedSkin.value?.texture_key),
        )
    },
    { immediate: true, flush: 'sync' },
)

onMounted(() => {
    void refreshValues().catch(console.warn)
})

await refreshValues(0, false, true)

watch(offline, async (isOffline, wasOffline) => {
    await refreshValues(0, wasOffline === true && !isOffline)
    notifyAccountChange()
})

const selectedAccount = computed(() =>
    accounts.value.find((account) => account.account_id === defaultUser.value),
)

function notifyAccountChange() {
    accountChangeRevision.value += 1
    emit('change')
}

const duplicateAccountNames = computed(() => {
    const counts = new Map<string, number>()
    for (const account of accounts.value) {
        counts.set(account.profile.name, (counts.get(account.profile.name) ?? 0) + 1)
    }
    return new Set([...counts].filter(([, count]) => count > 1).map(([name]) => name))
})

watch(
    () => route.fullPath,
    () => {
        if (!hasMissingAccountHeads()) return
        void refreshValues().catch((error) => {
            console.warn('Failed to refresh account heads after navigation:', error)
        })
    },
)

function getAccountSkin(account: MinecraftCredential | undefined): Skin | undefined {
    if (!account || account.account_type === 'offline') return undefined
    const skin =
        account.profile.skins?.find((skin) => skin.state === 'ACTIVE') ?? account.profile.skins?.[0]
    if (!skin?.url) return undefined

    return {
        texture_key: skin.textureKey ?? `${account.profile.id}:${skin.url}`,
        variant: skin.variant ?? 'UNKNOWN',
        texture: skin.url,
        source: 'custom_external',
        is_equipped: true,
    }
}

function cacheAccountHead(accountId: string, skin: Skin, headUrl: string) {
    accountHeadUrlCache.value = new Map(accountHeadUrlCache.value).set(accountId, headUrl)
    accountHeadTextureKeyCache.value = new Map(accountHeadTextureKeyCache.value).set(
        accountId,
        skin.texture_key,
    )
}

async function renderAccountHeads(accountList: MinecraftCredential[], generation: number) {
    await Promise.all(
        accountList.map(async (account) => {
            const skin = getAccountSkin(account)
            if (!skin) return

            try {
                const headUrl = await getPlayerHeadUrl(skin)
                if (generation !== refreshGeneration) return
                const currentSkin = getAccountSkin(
                    accounts.value.find((current) => current.account_id === account.account_id),
                )
                if (currentSkin?.texture_key !== skin.texture_key) return
                cacheAccountHead(account.account_id, skin, headUrl)
            } catch (error) {
                console.warn(`Failed to render head for account ${account.account_id}:`, error)
            }
        }),
    )
}

const avatarUrl = computed(() => {
    if (selectedAccount.value) {
        const cachedHeadUrl = accountHeadUrlCache.value.get(selectedAccount.value.account_id)
        if (cachedHeadUrl) return cachedHeadUrl
    }
    if (equippedSkin.value?.texture_key) {
        const cachedUrl = headUrlCache.value.get(equippedSkin.value.texture_key)
        if (cachedUrl) {
            return cachedUrl
        }
    }
    return selectedAccount.value ? defaultSteveHeadUrl : axolotlLogo
})

function getAccountAvatarUrl(account: MinecraftCredential) {
    const cachedHeadUrl = accountHeadUrlCache.value.get(account.account_id)
    if (cachedHeadUrl) {
        return cachedHeadUrl
    }
    if (
        account.account_id === selectedAccount.value?.account_id &&
        equippedSkin.value?.texture_key
    ) {
        const cachedUrl = headUrlCache.value.get(equippedSkin.value.texture_key)
        if (cachedUrl) {
            return cachedUrl
        }
    }
    return defaultSteveHeadUrl
}

function persistDefaultUser(userId: string) {
    const update = defaultUserUpdateQueue.then(async () => {
        await set_default_user(userId)
    })
    defaultUserUpdateQueue = update.catch(() => {})
    return update
}

function getAccountId(account: MinecraftCredential): string {
    if (account.account_id) return account.account_id

    const profileId = account.profile.id
    if (account.account_type === 'yggdrasil' && account.yggdrasil) {
        return `${account.account_type}:${account.yggdrasil.api_root}:${profileId}`
    }
    return `${account.account_type}:${profileId}`
}

async function setAccount(account: MinecraftCredential) {
    if (account.account_type === 'offline' && antiPiracyStatus.value.restricted) {
        errorStore.showAntiPiracyNotice()
        return
    }
    const userId = getAccountId(account)
    try {
        await persistDefaultUser(userId)
    } catch (error) {
        if (isOfflineAccountRestrictedError(error)) errorStore.showAntiPiracyNotice()
        else handleError(error as Error)
        return
    }
    refreshGeneration += 1
    defaultUser.value = userId
    equippedSkin.value = null

    if (defaultUser.value !== userId) return
    await refreshValues()
    if (defaultUser.value === userId) notifyAccountChange()
}

async function login() {
    if (offline.value) return
    loginDisabled.value = true
    try {
        const account = await loginToMinecraft({
            trouble: formatMessage(messages.loginTrouble),
            browserLogin: formatMessage(messages.loginBrowser),
            deviceCode: formatMessage(messages.loginDeviceCode),
        })
        if (account) await onMicrosoftLogin(account)
    } catch (error) {
        handleSevereError(error)
    } finally {
        loginDisabled.value = false
    }
}

async function onMicrosoftLogin(account: MinecraftCredential) {
    loginDisabled.value = true
    try {
        await refreshAntiPiracyStatus()
        await setAccount(account)
        trackEvent('AccountLogIn')
    } catch (error) {
        handleSevereError(error)
    } finally {
        loginDisabled.value = false
    }
}

function showOfflineAccountModal() {
    if (antiPiracyStatus.value.restricted) {
        errorStore.showAntiPiracyNotice()
        return
    }
    offlineUsername.value = ''
    offlineCustomUuid.value = false
    offlineUuid.value = ''
    offlineUuidDuplicate.value = false
    offlineAccountModal.value?.show()
}

async function showYggdrasilAccountModal() {
    yggdrasilApiRoot.value = LITTLE_SKIN_API_ROOT
    yggdrasilLogin.value = ''
    yggdrasilPassword.value = ''
    rememberYggdrasilPassword.value = true
    pendingYggdrasilFlowId.value = undefined
    pendingYggdrasilProfiles.value = []
    await loadSavedYggdrasilLogins()
    yggdrasilAccountModal.value?.show()
}

async function loadSavedYggdrasilLogins() {
    const storedLogins = await list_yggdrasil_saved_logins().catch(handleError)
    const savedLogins: SavedYggdrasilLogin[] = Array.isArray(storedLogins) ? [...storedLogins] : []
    const savedLoginKeys = new Set(
        savedLogins.map((savedLogin) => `${savedLogin.api_root}\n${savedLogin.login}`),
    )

    for (const account of accounts.value) {
        if (!account.yggdrasil) continue
        const savedLogin = {
            api_root: account.yggdrasil.api_root,
            login: account.yggdrasil.login,
        }
        const key = `${savedLogin.api_root}\n${savedLogin.login}`
        if (savedLoginKeys.has(key)) continue

        try {
            const password = await get_yggdrasil_password(savedLogin.api_root, savedLogin.login)
            if (!password) continue
            await set_yggdrasil_password(savedLogin.api_root, savedLogin.login, password)
            savedLogins.push(savedLogin)
            savedLoginKeys.add(key)
        } catch {
            continue
        }
    }

    savedYggdrasilLogins.value = savedLogins.sort((left, right) =>
        left.login.localeCompare(right.login),
    )
}

async function selectSavedYggdrasilLogin(savedLogin: SavedYggdrasilLogin) {
    if (loginDisabled.value) return

    loginDisabled.value = true
    try {
        const password = await get_yggdrasil_password(savedLogin.api_root, savedLogin.login)
        if (!password) {
            await delete_yggdrasil_password(savedLogin.api_root, savedLogin.login)
            await loadSavedYggdrasilLogins()
            return
        }
        yggdrasilApiRoot.value = savedLogin.api_root
        yggdrasilLogin.value = savedLogin.login
        yggdrasilPassword.value = password
        rememberYggdrasilPassword.value = true
    } catch (error) {
        handleError(error as Error)
    } finally {
        loginDisabled.value = false
    }
}

async function removeSavedYggdrasilLogin(savedLogin: SavedYggdrasilLogin) {
    if (loginDisabled.value) return

    loginDisabled.value = true
    try {
        await delete_yggdrasil_password(savedLogin.api_root, savedLogin.login)
        savedYggdrasilLogins.value = savedYggdrasilLogins.value.filter(
            (entry) => entry.api_root !== savedLogin.api_root || entry.login !== savedLogin.login,
        )
        if (
            yggdrasilApiRoot.value === savedLogin.api_root &&
            yggdrasilLogin.value === savedLogin.login
        ) {
            yggdrasilPassword.value = ''
            rememberYggdrasilPassword.value = false
        }
    } catch (error) {
        handleError(error as Error)
    } finally {
        loginDisabled.value = false
    }
}

function useLittleSkinPreset() {
    yggdrasilApiRoot.value = LITTLE_SKIN_API_ROOT
}

async function loadRememberedYggdrasilPassword() {
    if (
        !rememberYggdrasilPassword.value ||
        !yggdrasilApiRoot.value.trim() ||
        !yggdrasilLogin.value.trim() ||
        yggdrasilPassword.value
    )
        return

    try {
        const password = await get_yggdrasil_password(
            yggdrasilApiRoot.value.trim(),
            yggdrasilLogin.value.trim(),
        )
        if (password) yggdrasilPassword.value = password
    } catch {
        return
    }
}

async function persistYggdrasilPasswordPreference() {
    try {
        if (rememberYggdrasilPassword.value) {
            await set_yggdrasil_password(
                yggdrasilApiRoot.value.trim(),
                yggdrasilLogin.value.trim(),
                yggdrasilPassword.value,
            )
        } else {
            await delete_yggdrasil_password(
                yggdrasilApiRoot.value.trim(),
                yggdrasilLogin.value.trim(),
            )
        }
    } catch (error) {
        handleError(error as Error)
    }
}

async function addYggdrasilAccount() {
    if (!yggdrasilFormValid.value || loginDisabled.value) return

    loginDisabled.value = true
    try {
        const result = (await begin_yggdrasil_login(
            yggdrasilApiRoot.value.trim(),
            yggdrasilLogin.value.trim(),
            yggdrasilPassword.value,
        )) as YggdrasilLoginResult
        if (result.status === 'complete') {
            await persistYggdrasilPasswordPreference()
            yggdrasilAccountModal.value?.hide()
            await setAccount(result.credentials)
            trackEvent('YggdrasilAccountAdd')
        } else {
            pendingYggdrasilFlowId.value = result.flow_id
            pendingYggdrasilProfiles.value = result.profiles
            yggdrasilAccountModal.value?.hide()
            yggdrasilProfileModal.value?.show()
        }
    } catch (error) {
        handleError(error as Error)
    } finally {
        loginDisabled.value = false
    }
}

async function selectYggdrasilProfile(profileId: string) {
    if (!pendingYggdrasilFlowId.value || loginDisabled.value) return

    loginDisabled.value = true
    try {
        const account = (await finish_yggdrasil_login(
            pendingYggdrasilFlowId.value,
            profileId,
        )) as MinecraftCredential
        await persistYggdrasilPasswordPreference()
        yggdrasilProfileModal.value?.hide()
        await setAccount(account)
        trackEvent('YggdrasilAccountAdd')
    } catch (error) {
        handleError(error as Error)
    } finally {
        loginDisabled.value = false
    }
}

async function addOfflineAccount() {
    if (!offlineFormValid.value || loginDisabled.value) return
    if (antiPiracyStatus.value.restricted) {
        errorStore.showAntiPiracyNotice()
        return
    }

    loginDisabled.value = true
    offlineUuidDuplicate.value = false
    try {
        const account = await add_offline_user(
            offlineUsername.value.trim(),
            offlineCustomUuid.value ? offlineUuid.value.replaceAll('-', '') : undefined,
        )
        offlineAccountModal.value?.hide()
        await setAccount(account)
        trackEvent('OfflineAccountAdd')
    } catch (error) {
        offlineUuidDuplicate.value = isDuplicateUuidError(error)
        if (isOfflineAccountRestrictedError(error)) errorStore.showAntiPiracyNotice()
        else if (!offlineUuidDuplicate.value) handleError(error as Error)
    } finally {
        loginDisabled.value = false
    }
}

function isDuplicateUuidError(error: unknown) {
    const rawMessage =
        error instanceof Error
            ? error.message
            : typeof error === 'string'
              ? error
              : JSON.stringify(error)
    return rawMessage?.includes('An account with this UUID already exists') ?? false
}

async function logout(account: MinecraftCredential) {
    await remove_user(getAccountId(account)).catch(handleError)
    await refreshValues()
    if (!selectedAccount.value && accounts.value.length > 0) {
        await setAccount(accounts.value[0])
    } else {
        notifyAccountChange()
    }
    trackEvent('AccountLogOut')
}

async function copyAccountUuid(account: MinecraftCredential) {
    try {
        await navigator.clipboard.writeText(account.profile.id)
    } catch (error) {
        handleError(error as Error)
    }
}

const unlisten = await process_listener(async (e) => {
    if (e.event === 'launched') {
        await refreshValues()
    }
})
const unlistenDeviceLogin = await listen('minecraft-device-login-requested', () => {
    minecraftLoginModal.value?.showDeviceLogin()
})

onUnmounted(() => {
    refreshGeneration += 1
    clearHeadRefreshRetry()
    unlisten()
    unlistenDeviceLogin()
})

const messages = defineMessages({
    offlineMode: {
        id: 'minecraft-account.offline-mode',
        defaultMessage: 'Offline mode',
    },
    offlineModeDescription: {
        id: 'minecraft-account.offline-mode.description',
        defaultMessage:
            'Only offline accounts are available. You can launch fully downloaded instances.',
    },
    refreshNetworkStatus: {
        id: 'minecraft-account.offline-mode.refresh',
        defaultMessage: 'Refresh connection status',
    },
    notSignedIn: {
        id: 'minecraft-account.not-signed-in',
        defaultMessage: 'Not signed in',
    },
    addMicrosoftAccount: {
        id: 'minecraft-account.add-microsoft-account',
        defaultMessage: 'Add Microsoft account',
    },
    addThirdPartyAccount: {
        id: 'minecraft-account.add-third-party-account',
        defaultMessage: 'Add third-party account',
    },
    thirdPartyAccount: {
        id: 'minecraft-account.third-party-account',
        defaultMessage: 'Third-party Minecraft account',
    },
    thirdPartyBadge: {
        id: 'minecraft-account.third-party-badge',
        defaultMessage: 'Third-party',
    },
    thirdPartyModalTitle: {
        id: 'minecraft-account.third-party-modal.title',
        defaultMessage: 'Sign in with a third-party service',
    },
    thirdPartyModalDescription: {
        id: 'minecraft-account.third-party-modal.description',
        defaultMessage: 'Use LittleSkin or another compatible Yggdrasil authentication service.',
    },
    useLittleSkin: {
        id: 'minecraft-account.third-party-modal.littleskin',
        defaultMessage: 'Use LittleSkin',
    },
    apiRootLabel: {
        id: 'minecraft-account.third-party-modal.api-root',
        defaultMessage: 'Yggdrasil API address',
    },
    apiRootPlaceholder: {
        id: 'minecraft-account.third-party-modal.api-root-placeholder',
        defaultMessage: 'https://example.com/api/yggdrasil',
    },
    accountLabel: {
        id: 'minecraft-account.third-party-modal.account',
        defaultMessage: 'Account or email',
    },
    accountPlaceholder: {
        id: 'minecraft-account.third-party-modal.account-placeholder',
        defaultMessage: 'Enter your account or email',
    },
    passwordLabel: {
        id: 'minecraft-account.third-party-modal.password',
        defaultMessage: 'Password',
    },
    rememberPassword: {
        id: 'minecraft-account.third-party-modal.remember-password',
        defaultMessage: 'Save this login on this device',
    },
    savedLogins: {
        id: 'minecraft-account.third-party-modal.saved-logins',
        defaultMessage: 'Saved logins',
    },
    removeSavedLogin: {
        id: 'minecraft-account.third-party-modal.remove-saved-login',
        defaultMessage: 'Remove saved login',
    },
    signInButton: {
        id: 'minecraft-account.third-party-modal.sign-in',
        defaultMessage: 'Sign in',
    },
    selectProfileTitle: {
        id: 'minecraft-account.third-party-profile.title',
        defaultMessage: 'Select a profile',
    },
    selectProfileDescription: {
        id: 'minecraft-account.third-party-profile.description',
        defaultMessage: 'Choose the Minecraft profile to use with this account.',
    },
    addOfflineAccount: {
        id: 'minecraft-account.add-offline-account',
        defaultMessage: 'Add offline account',
    },
    offlineAccount: {
        id: 'minecraft-account.offline-account',
        defaultMessage: 'Offline Minecraft account',
    },
    offlineBadge: {
        id: 'minecraft-account.offline-badge',
        defaultMessage: 'Offline',
    },
    officialBadge: {
        id: 'minecraft-account.official-badge',
        defaultMessage: 'Official',
    },
    offlineModalTitle: {
        id: 'minecraft-account.offline-modal.title',
        defaultMessage: 'Add offline account',
    },
    offlineModalDescription: {
        id: 'minecraft-account.offline-modal.description',
        defaultMessage:
            'Choose the username used in offline games. This account can only join servers that allow offline players.',
    },
    usernameLabel: {
        id: 'minecraft-account.offline-modal.username-label',
        defaultMessage: 'Minecraft username',
    },
    usernamePlaceholder: {
        id: 'minecraft-account.offline-modal.username-placeholder',
        defaultMessage: 'Enter a username',
    },
    usernameValidation: {
        id: 'minecraft-account.offline-modal.username-validation',
        defaultMessage: 'Use 1–16 letters, numbers, or underscores, including Chinese characters.',
    },
    chineseUsernameWarning: {
        id: 'minecraft-account.offline-modal.chinese-username-warning',
        defaultMessage:
            'Minecraft 1.18 and newer may reject Chinese usernames when entering singleplayer worlds or servers. Use this account with an older version, or choose an English username for newer versions.',
    },
    customUuidLabel: {
        id: 'minecraft-account.offline-modal.custom-uuid',
        defaultMessage: 'Custom UUID',
    },
    customUuidDuplicate: {
        id: 'minecraft-account.offline-modal.custom-uuid-duplicate',
        defaultMessage: 'An account with this UUID already exists. Please use a different UUID.',
    },
    customUuidWarning: {
        id: 'minecraft-account.offline-modal.custom-uuid-warning',
        defaultMessage: "If you don't understand what this is, do not enable this feature.",
    },
    customUuidInputLabel: {
        id: 'minecraft-account.offline-modal.custom-uuid-input-label',
        defaultMessage: 'UUID',
    },
    customUuidPlaceholder: {
        id: 'minecraft-account.offline-modal.custom-uuid-placeholder',
        defaultMessage: '00000000-0000-0000-0000-000000000000',
    },
    customUuidValidation: {
        id: 'minecraft-account.offline-modal.custom-uuid-validation',
        defaultMessage: 'Use 32 hexadecimal characters. Hyphens are optional.',
    },
    createOfflineAccount: {
        id: 'minecraft-account.offline-modal.create',
        defaultMessage: 'Create account',
    },
    copyUuid: {
        id: 'minecraft-account.copy-uuid',
        defaultMessage: 'Copy UUID',
    },
    removeAccount: {
        id: 'minecraft-account.remove-account',
        defaultMessage: 'Remove account',
    },
    selectAccount: {
        id: 'minecraft-account.select-account',
        defaultMessage: 'Select account',
    },
    minecraftAccount: {
        id: 'minecraft-account.label',
        defaultMessage: 'Minecraft account',
    },
    signInToMinecraft: {
        id: 'minecraft-account.sign-in',
        defaultMessage: 'Sign in to Minecraft',
    },
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
})
</script>
