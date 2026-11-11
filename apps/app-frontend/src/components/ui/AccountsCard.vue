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
            <Draggable
                v-if="accounts.length > 0"
                :list="accounts"
                item-key="account_id"
                tag="div"
                :animation="200"
                easing="cubic-bezier(0.22, 1, 0.36, 1)"
                filter=".account-row-actions"
                :force-fallback="true"
                :fallback-on-body="false"
                fallback-class="account-row-fallback"
                ghost-class="account-row-ghost"
                chosen-class="account-row-chosen"
                :scroll="true"
                :scroll-sensitivity="48"
                :scroll-speed="16"
                :bubble-scroll="false"
                @start="startAccountDrag"
                @end="finishAccountDrag"
            >
                <template #item="{ element: account }">
                    <div class="flex gap-1 items-center" @pointerdown="rememberAccountDragTap">
                        <button
                            :aria-description="formatMessage(messages.dragToReorder)"
                            class="flex items-center flex-shrink flex-grow overflow-clip gap-2 p-2 border-0 bg-transparent cursor-pointer button-base min-w-0"
                            :aria-disabled="
                                account.account_type === 'offline' && antiPiracyStatus.restricted
                            "
                            :class="{
                                'opacity-50':
                                    account.account_type === 'offline' &&
                                    antiPiracyStatus.restricted,
                            }"
                            @click="selectAccountRow(account)"
                            @keydown.alt.up.prevent="moveAccount(account, -1)"
                            @keydown.alt.down.prevent="moveAccount(account, 1)"
                        >
                            <RadioButtonCheckedIcon
                                v-if="
                                    selectedAccount &&
                                    selectedAccount.account_id === account.account_id
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
                        <div class="account-row-actions flex shrink-0 items-center">
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
            </Draggable>
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
                    :maxlength="16"
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
                    :spellcheck="false"
                    :maxlength="36"
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
import Draggable from 'vuedraggable'

import axolotlLogo from '@/assets/axolotl.png'
import steveSkinTexture from '@/assets/skins/steve.png?inline'
import MinecraftLoginModal from '@/components/ui/MinecraftLoginModal.vue'
import {
    isOfflineAccountRestrictedError,
    useAntiPiracyStatus,
} from '@/composables/useAntiPiracyStatus'
import { useNetworkStatus } from '@/composables/useNetworkStatus'
import {
    readAccountOrder,
    sortMinecraftAccountsWithOrder,
    writeAccountOrder,
} from '@/helpers/account-order'
import { preferredOnlineAccountId } from '@/helpers/account-selection'
import { accountSelectionRevision } from '@/helpers/account-state'
import { minecraftAccountId } from '@/helpers/accounts'
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
import type { MinecraftCredential } from '@/helpers/auth-types'
import { process_listener } from '@/helpers/events'
import { mojangAuthKeys } from '@/helpers/mojang-auth/monitor'
import {
    getPlayerHeadUrl,
    retainPlayerHeadTextures,
} from '@/helpers/rendering/batch-skin-renderer.ts'
import { createSkinHeadDataUrl, getAccountSkin } from '@/helpers/rendering/skin-head'
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
const accountOrder = ref<string[]>(readAccountOrder())
const draggingAccount = ref(false)
// A drag ends on mouseup, and the click the browser still synthesizes for the
// row follows it. Keep that click from changing the selected account.
let suppressAccountClickUntil = 0
// The order the drag started from, so a drag that lands where it began does not
// pin the default ordering as if the user had chosen it.
let orderAtDragStart: string[] = []
// Sortable's fallback ghost follows the pointer on both axes. Accounts are a
// vertical list, so its horizontal offset is clamped back to zero while the
// drag is live.
let ghostLockObserver: MutationObserver | null = null
// The pointer position the drag started from, and where it is now: the ghost is
// positioned from these rather than from Sortable's accumulated delta, so a
// pointer that leaves the box and comes back meets the row again instead of
// keeping the offset it picked up outside.
let dragTapY: number | null = null
let dragPointerY: number | null = null
let dragBox: { top: number; bottom: number } | null = null
let cancelledAccountDrag = false
// How far past the list a drag may go before it is dropped instead of clamped.
const ACCOUNT_DRAG_ESCAPE_MARGIN = 64
const loginDisabled = ref(false)
const defaultUser = ref<string | undefined>()
const equippedSkin = ref<Skin | null>(null)
const accountChangeRevision = ref(0)
const headUrlCache = ref(new Map<string, string>())
const accountHeadUrlCache = ref(new Map<string, string>())
const accountHeadTextureKeyCache = ref(new Map<string, string>())
let refreshGeneration = 0
let ownAccountWrites = 0
let unclaimedSelectionChanges = 0
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
        accounts.value = sortMinecraftAccountsWithOrder(
            userList as unknown as MinecraftCredential[],
            accountOrder.value,
        )
    }
    let resolvedSelectedUser = selectedUser
    if (preferOnlineAccount && selectedUser) {
        resolvedSelectedUser = preferredOnlineAccountId(selectedUser, accounts.value)
    }
    defaultUser.value = resolvedSelectedUser ?? undefined
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
        accounts.value = sortMinecraftAccountsWithOrder(
            updatedUsers as unknown as MinecraftCredential[],
            accountOrder.value,
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

// The card stays mounted behind first-run setup, so an account chosen elsewhere
// has to reach it here. Its own writes refresh in the order they need, and a
// second refresh would supersede theirs. The revision moves synchronously with
// the write, which is why this watcher must be sync to see the counter.
watch(
    accountSelectionRevision,
    () => {
        if (ownAccountWrites > 0) {
            unclaimedSelectionChanges += 1
            return
        }
        refreshAfterOutsideSelection()
    },
    { flush: 'sync' },
)

function refreshAfterOutsideSelection() {
    void refreshValues()
        .then(notifyAccountChange)
        .catch((error) => {
            console.warn('Failed to refresh accounts after a selection change:', error)
        })
}

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

/**
 * Runs one of the card's own account writes. `write` has to be a call from
 * `helpers/auth` that signals exactly one selection change when it succeeds,
 * because that signal is what is subtracted from the changes seen meanwhile.
 */
async function ownAccountWrite<T>(write: () => Promise<T>): Promise<T> {
    ownAccountWrites += 1
    try {
        const result = await write()
        unclaimedSelectionChanges -= 1
        return result
    } finally {
        ownAccountWrites -= 1
        if (ownAccountWrites === 0) {
            // Another component changed the selection during the write, and a
            // failed write has no refresh of its own to pick that up.
            if (unclaimedSelectionChanges > 0) refreshAfterOutsideSelection()
            unclaimedSelectionChanges = 0
        }
    }
}

function persistDefaultUser(userId: string) {
    const update = defaultUserUpdateQueue.then(async () => {
        await ownAccountWrite(() => set_default_user(userId))
    })
    defaultUserUpdateQueue = update.catch(() => {})
    return update
}

function selectAccountRow(account: MinecraftCredential) {
    if (draggingAccount.value || Date.now() < suppressAccountClickUntil) return
    void setAccount(account)
}

function pointerClientY(event: Event | undefined): number | null {
    if (!event) return null

    const touchEvent = event as TouchEvent
    if (touchEvent.touches?.length) return touchEvent.touches[0]?.clientY ?? null

    const mouseEvent = event as MouseEvent
    return typeof mouseEvent.clientY === 'number' ? mouseEvent.clientY : null
}

function rememberAccountDragTap(event: PointerEvent) {
    if (event.button !== 0) return
    if ((event.target as HTMLElement | null)?.closest('.account-row-actions')) return
    dragTapY = event.clientY
}

function startAccountDrag(event?: { originalEvent?: Event }) {
    draggingAccount.value = true
    suppressAccountClickUntil = Date.now() + 300
    orderAtDragStart = accounts.value.map(minecraftAccountId)
    cancelledAccountDrag = false
    dragPointerY = pointerClientY(event?.originalEvent) ?? dragTapY
    lockDraggedGhostToVertical()
    document.addEventListener('pointermove', trackAccountDragPointer, true)
    document.addEventListener('mousemove', trackAccountDragPointer, true)
}

function trackAccountDragPointer(event: Event) {
    if (!draggingAccount.value || cancelledAccountDrag) return

    const pointerY = pointerClientY(event)
    if (pointerY === null) return
    dragPointerY = pointerY

    if (!dragBox) return
    const outside =
        pointerY < dragBox.top - ACCOUNT_DRAG_ESCAPE_MARGIN ||
        pointerY > dragBox.bottom + ACCOUNT_DRAG_ESCAPE_MARGIN
    if (!outside) return

    // Past the allowed margin the drag is dropped rather than clamped. A
    // synthetic release runs Sortable's own drop path, which removes the ghost
    // and fires `end`; `finishAccountDrag` then restores the starting order.
    cancelledAccountDrag = true
    document.dispatchEvent(
        new MouseEvent('mouseup', {
            bubbles: true,
            cancelable: true,
            clientX: (event as MouseEvent).clientX ?? 0,
            clientY: pointerY,
            button: 0,
        }),
    )
}

function finishAccountDrag() {
    draggingAccount.value = false
    suppressAccountClickUntil = Date.now() + 250
    ghostLockObserver?.disconnect()
    ghostLockObserver = null
    document.removeEventListener('pointermove', trackAccountDragPointer, true)
    document.removeEventListener('mousemove', trackAccountDragPointer, true)
    dragBox = null
    dragTapY = null
    dragPointerY = null

    if (cancelledAccountDrag) {
        cancelledAccountDrag = false
        accounts.value = sortMinecraftAccountsWithOrder(accounts.value, orderAtDragStart)
        return
    }

    const next = accounts.value.map(minecraftAccountId)
    const unchanged =
        next.length === orderAtDragStart.length &&
        next.every((id, index) => id === orderAtDragStart[index])
    if (!unchanged) persistAccountOrder(next)
}

/**
 * Keeps the dragged row inside the list: the fallback clone sticks to the
 * list's vertical axis, follows the pointer's own position, and is clamped to
 * the list's box. A sideways or overshooting pointer therefore cannot throw the
 * row out of the sidebar, and a pointer that leaves the box and returns finds
 * the row at the cursor again instead of keeping the offset it picked up
 * outside.
 *
 * Sortable positions its fallback clone with a `matrix(…)` transform whose
 * fifth component is the horizontal delta and sixth the vertical one.
 */
function lockDraggedGhostToVertical() {
    ghostLockObserver?.disconnect()
    ghostLockObserver = null

    const ghost = document.querySelector<HTMLElement>('.account-row-fallback')
    const container = ghost?.parentElement
    if (!ghost || !container) return

    const ghostRect = ghost.getBoundingClientRect()
    const containerRect = container.getBoundingClientRect()
    const baseTop = ghostRect.top
    const minOffset = containerRect.top - baseTop
    const maxOffset = Math.max(minOffset, containerRect.bottom - ghostRect.height - baseTop)
    dragBox = { top: containerRect.top, bottom: containerRect.bottom }

    const constrain = () => {
        const match = /matrix\(([^)]*)\)/.exec(ghost.style.transform)
        if (!match) return

        const values = match[1].split(',').map((value) => Number.parseFloat(value))
        if (values.length !== 6 || !Number.isFinite(values[4]) || !Number.isFinite(values[5])) {
            return
        }

        const offset =
            dragTapY !== null && dragPointerY !== null ? dragPointerY - dragTapY : values[5]
        values[4] = 0
        values[5] = Math.min(Math.max(offset, minOffset), maxOffset)

        ghost.style.transform = `matrix(${values.join(', ')})`
    }

    constrain()
    ghostLockObserver = new MutationObserver(constrain)
    ghostLockObserver.observe(ghost, { attributes: true, attributeFilter: ['style'] })
}

/**
 * Saves the order the list is showing now. A failed write leaves the previous
 * order in place rather than letting a restart disagree with what is on screen.
 */
function persistAccountOrder(next: string[] = accounts.value.map(minecraftAccountId)) {
    const previous = accountOrder.value
    try {
        writeAccountOrder(next)
        accountOrder.value = next
    } catch (error) {
        accountOrder.value = previous
        accounts.value = sortMinecraftAccountsWithOrder(accounts.value, previous)
        handleError(error as Error)
    }
}

/** Keyboard equivalent of dragging: move one account up or down. */
function moveAccount(account: MinecraftCredential, offset: number) {
    const id = minecraftAccountId(account)
    const index = accounts.value.findIndex((entry) => minecraftAccountId(entry) === id)
    const target = index + offset
    if (index < 0 || target < 0 || target >= accounts.value.length) return

    const next = [...accounts.value]
    const [moved] = next.splice(index, 1)
    next.splice(target, 0, moved)
    accounts.value = next
    persistAccountOrder()
}

async function setAccount(account: MinecraftCredential) {
    if (account.account_type === 'offline' && antiPiracyStatus.value.restricted) {
        errorStore.showAntiPiracyNotice()
        return
    }
    const userId = minecraftAccountId(account)
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
        const account = await ownAccountWrite(() =>
            add_offline_user(
                offlineUsername.value.trim(),
                offlineCustomUuid.value ? offlineUuid.value.replaceAll('-', '') : undefined,
            ),
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
    await ownAccountWrite(() => remove_user(minecraftAccountId(account))).catch(handleError)
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

const unlisten = await process_listener(async (e: { event: string }) => {
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
    ghostLockObserver?.disconnect()
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
    dragToReorder: {
        id: 'minecraft-account.drag-to-reorder',
        defaultMessage: 'Drag to reorder, or press Alt with the arrow keys',
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

<style scoped>
.account-row-ghost {
    opacity: 0.4;
}

.account-row-chosen {
    cursor: grabbing;
}

.account-row-fallback {
    border-radius: 8px;
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.35);
}
</style>
