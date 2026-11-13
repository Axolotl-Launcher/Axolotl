<script setup lang="ts">
import {
    CheckIcon,
    ChevronRightIcon,
    GlobeIcon,
    LockIcon,
    LogInIcon,
    SpinnerIcon,
    UserIcon,
} from '@modrinth/assets'
import {
    Avatar,
    Button,
    Checkbox,
    defineMessages,
    injectNotificationManager,
    StyledInput,
    useVIntl,
} from '@modrinth/ui'
import { computed, ref, watch } from 'vue'

import {
    isOfflineAccountRestrictedError,
    useAntiPiracyStatus,
} from '@/composables/useAntiPiracyStatus'
import { useNetworkStatus } from '@/composables/useNetworkStatus'
import {
    add_offline_user,
    begin_yggdrasil_login,
    delete_yggdrasil_password,
    finish_yggdrasil_login,
    login as loginToMinecraft,
    set_default_user,
    set_yggdrasil_password,
} from '@/helpers/auth'
import { handleSevereError, useError } from '@/store/error.js'

import { useOobeStep } from '../../context'
import OobeChoice from '../../parts/OobeChoice.vue'
import { type OobeAccount, showPicked, useOobeAccounts } from './accounts'

type Account = OobeAccount
type YggdrasilProfile = { id: string; name: string }
type YggdrasilLoginResult =
    | { status: 'complete'; credentials: Account }
    | { status: 'select_profile'; flow_id: string; profiles: YggdrasilProfile[] }
type Method = 'microsoft' | 'yggdrasil' | 'offline'

const LITTLE_SKIN_API_ROOT = 'https://littleskin.cn/api/yggdrasil'

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const { offline } = useNetworkStatus()
const { status: antiPiracy, refresh: refreshAntiPiracy } = useAntiPiracyStatus()
const errorStore = useError()
const step = useOobeStep()

const messages = defineMessages({
    added: { id: 'app.oobe.account.added', defaultMessage: 'On this device' },
    addTitle: { id: 'app.oobe.account.add-title', defaultMessage: 'Add an account' },
    addAnother: { id: 'app.oobe.account.add-another', defaultMessage: 'Add another account' },
    microsoft: { id: 'app.oobe.account.microsoft', defaultMessage: 'Microsoft account' },
    microsoftDetail: {
        id: 'app.oobe.account.microsoft-detail',
        defaultMessage: 'Sign in with the account that owns Minecraft: Java Edition.',
    },
    recommended: { id: 'app.oobe.account.recommended', defaultMessage: 'Recommended' },
    yggdrasil: { id: 'app.oobe.account.third-party', defaultMessage: 'Third-party account' },
    yggdrasilDetail: {
        id: 'app.oobe.account.third-party-detail',
        defaultMessage: 'LittleSkin or another Yggdrasil authentication service.',
    },
    offline: { id: 'app.oobe.account.offline', defaultMessage: 'Offline account' },
    offlineDetail: {
        id: 'app.oobe.account.offline-detail',
        defaultMessage: 'Just a player name. Only works on servers that allow offline players.',
    },
    offlineRestricted: {
        id: 'app.oobe.account.offline-restricted',
        defaultMessage: 'Sign in with a Microsoft account on this device first.',
    },
    networkOffline: {
        id: 'minecraft-account.offline-mode.description',
        defaultMessage:
            'Only offline accounts are available. You can launch fully downloaded instances.',
    },
    add: { id: 'app.oobe.account.add', defaultMessage: 'Add' },
    loginTrouble: { id: 'minecraft-login.trouble', defaultMessage: 'Having trouble?' },
    loginBrowser: { id: 'minecraft-login.browser', defaultMessage: 'Use browser login' },
    loginDeviceCode: { id: 'minecraft-login.device-code', defaultMessage: 'Use device code' },
    officialBadge: { id: 'minecraft-account.official-badge', defaultMessage: 'Official' },
    offlineBadge: { id: 'minecraft-account.offline-badge', defaultMessage: 'Offline' },
    thirdPartyBadge: { id: 'minecraft-account.third-party-badge', defaultMessage: 'Third-party' },
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
    apiRootLabel: {
        id: 'minecraft-account.third-party-modal.api-root',
        defaultMessage: 'Yggdrasil API address',
    },
    useLittleSkin: {
        id: 'minecraft-account.third-party-modal.littleskin',
        defaultMessage: 'Use LittleSkin',
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
    signIn: { id: 'minecraft-account.third-party-modal.sign-in', defaultMessage: 'Sign in' },
    selectProfile: {
        id: 'minecraft-account.third-party-profile.description',
        defaultMessage: 'Choose the Minecraft profile to use with this account.',
    },
})

const { accounts, defaultAccount, selected, refresh, headOf } = useOobeAccounts()
// A pick left over from an earlier visit is stale; show what the launcher holds.
showPicked(undefined)
const open = ref<Exclude<Method, 'microsoft'> | null>(null)
const working = ref<Method | null>(null)

const username = ref('')
const usernameValid = computed(() => /^[\p{L}\p{N}_]{1,16}$/u.test(username.value.trim()))
const usernameHasHan = computed(() => /\p{Script=Han}/u.test(username.value.trim()))

const apiRoot = ref(LITTLE_SKIN_API_ROOT)
const login = ref('')
const password = ref('')
const remember = ref(true)
const pendingFlow = ref<string | null>(null)
const pendingProfiles = ref<YggdrasilProfile[]>([])
const yggdrasilValid = computed(
    () =>
        apiRoot.value.trim().length > 0 &&
        login.value.trim().length > 0 &&
        password.value.length > 0,
)

watch(
    () => accounts.value.length > 0,
    (ready) => step.setReady(ready),
    { immediate: true },
)
watch(working, (method) => step.setBusy(method !== null))

function badgeOf(account: Account) {
    if (account.account_type === 'offline') return formatMessage(messages.offlineBadge)
    if (account.account_type === 'yggdrasil')
        return account.yggdrasil?.server_name || formatMessage(messages.thirdPartyBadge)
    return formatMessage(messages.officialBadge)
}

let defaultUserWrites: Promise<void> = Promise.resolve()
/** Moves with every pick, so only the latest one sets the checkmark. */
let selection = 0

async function makeDefault(account: Account) {
    if (account.account_type === 'offline' && antiPiracy.value.restricted) {
        errorStore.showAntiPiracyNotice()
        return
    }
    const mine = ++selection
    showPicked(account.account_id)
    const changed = defaultAccount.value !== account.account_id
    // Writes run one at a time, as in the accounts menu, so the backend ends on the last pick.
    const write = defaultUserWrites.then(() => set_default_user(account.account_id))
    defaultUserWrites = write.then(
        () => {},
        () => {},
    )
    try {
        await write
        if (changed) step.markChanged()
        if (mine === selection) defaultAccount.value = account.account_id
    } catch (error) {
        if (isOfflineAccountRestrictedError(error)) errorStore.showAntiPiracyNotice()
        else handleError(error as Error)
        // An earlier pick may have been stored meanwhile; show what the backend now holds.
        if (mine === selection) {
            showPicked(undefined)
            await refresh()
        }
    }
}

async function adopt(account: Account) {
    step.markChanged()
    await makeDefault(account)
    await refresh()
    open.value = null
}

async function signInMicrosoft() {
    if (offline.value || working.value) return
    working.value = 'microsoft'
    try {
        const account = (await loginToMinecraft({
            trouble: formatMessage(messages.loginTrouble),
            browserLogin: formatMessage(messages.loginBrowser),
            deviceCode: formatMessage(messages.loginDeviceCode),
        })) as Account | null
        if (account) {
            await refreshAntiPiracy()
            await adopt(account)
        }
    } catch (error) {
        handleSevereError(error)
    } finally {
        working.value = null
    }
}

function toggle(method: Exclude<Method, 'microsoft'>) {
    if (method === 'offline' && antiPiracy.value.restricted) {
        errorStore.showAntiPiracyNotice()
        return
    }
    open.value = open.value === method ? null : method
}

async function addOffline() {
    if (!usernameValid.value || working.value) return
    if (antiPiracy.value.restricted) {
        errorStore.showAntiPiracyNotice()
        return
    }
    working.value = 'offline'
    try {
        const account = (await add_offline_user(username.value.trim())) as unknown as Account
        await adopt(account)
        username.value = ''
    } catch (error) {
        if (isOfflineAccountRestrictedError(error)) errorStore.showAntiPiracyNotice()
        else handleError(error as Error)
    } finally {
        working.value = null
    }
}

/** Saves the login, or forgets one saved before, as the checkbox says; the account menu does the same. */
async function rememberLogin() {
    const root = apiRoot.value.trim()
    const account = login.value.trim()
    try {
        if (remember.value) await set_yggdrasil_password(root, account, password.value)
        else await delete_yggdrasil_password(root, account)
    } catch (error) {
        handleError(error as Error)
    }
}

async function signInYggdrasil() {
    if (!yggdrasilValid.value || working.value) return
    working.value = 'yggdrasil'
    try {
        const result = (await begin_yggdrasil_login(
            apiRoot.value.trim(),
            login.value.trim(),
            password.value,
        )) as YggdrasilLoginResult
        if (result.status === 'complete') {
            await rememberLogin()
            await adopt(result.credentials)
            password.value = ''
        } else {
            pendingFlow.value = result.flow_id
            pendingProfiles.value = result.profiles
        }
    } catch (error) {
        handleError(error as Error)
    } finally {
        working.value = null
    }
}

async function chooseProfile(profile: YggdrasilProfile) {
    if (!pendingFlow.value || working.value) return
    working.value = 'yggdrasil'
    try {
        const account = (await finish_yggdrasil_login(
            pendingFlow.value,
            profile.id,
        )) as unknown as Account
        await rememberLogin()
        pendingFlow.value = null
        pendingProfiles.value = []
        password.value = ''
        await adopt(account)
    } catch (error) {
        handleError(error as Error)
    } finally {
        working.value = null
    }
}
</script>

<template>
    <div class="flex flex-col gap-6">
        <section v-if="accounts.length > 0" class="flex flex-col gap-3">
            <h2 class="m-0 text-base font-semibold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.added) }}
            </h2>
            <div
                role="radiogroup"
                :aria-label="formatMessage(messages.added)"
                class="flex flex-col gap-1.5"
            >
                <OobeChoice
                    v-for="account in accounts"
                    :key="account.account_id"
                    role="radio"
                    :selected="account.account_id === selected"
                    :class="{
                        'opacity-60': account.account_type === 'offline' && antiPiracy.restricted,
                    }"
                    @select="makeDefault(account)"
                >
                    <template #leading>
                        <Avatar :src="headOf(account)" size="36px" pixelated />
                    </template>
                    <span class="truncate font-semibold text-[var(--color-text-primary)]">
                        {{ account.profile.name }}
                    </span>
                    <span class="truncate text-sm text-[var(--color-text-tertiary)]">
                        {{ badgeOf(account) }}
                    </span>
                    <template #trailing>
                        <CheckIcon
                            v-if="account.account_id === selected"
                            class="size-5 shrink-0 text-brand"
                        />
                    </template>
                </OobeChoice>
            </div>
        </section>

        <section class="flex flex-col gap-3">
            <h2 class="m-0 text-base font-semibold text-[var(--color-text-primary)]">
                {{ formatMessage(accounts.length > 0 ? messages.addAnother : messages.addTitle) }}
            </h2>
            <p
                v-if="offline"
                class="m-0 rounded-[var(--radius-lg)] border border-solid border-orange bg-highlight-orange px-4 py-3 text-sm text-[var(--color-text-primary)]"
            >
                {{ formatMessage(messages.networkOffline) }}
            </p>

            <OobeChoice :disabled="offline || working !== null" @select="signInMicrosoft">
                <template #leading>
                    <span
                        class="flex size-9 shrink-0 items-center justify-center rounded-full bg-brand text-[var(--color-accent-contrast)]"
                    >
                        <LogInIcon class="size-4" />
                    </span>
                </template>
                <span
                    class="flex items-center gap-2 font-semibold text-[var(--color-text-primary)]"
                >
                    {{ formatMessage(messages.microsoft) }}
                    <span
                        class="rounded-full bg-brand-highlight px-2 py-0.5 text-xs font-semibold text-brand"
                    >
                        {{ formatMessage(messages.recommended) }}
                    </span>
                </span>
                <span class="text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.microsoftDetail) }}
                </span>
                <template #trailing>
                    <SpinnerIcon
                        v-if="working === 'microsoft'"
                        class="size-5 shrink-0 animate-spin"
                    />
                    <ChevronRightIcon
                        v-else
                        class="size-5 shrink-0 text-[var(--color-text-tertiary)]"
                    />
                </template>
            </OobeChoice>

            <div class="flex flex-col">
                <OobeChoice
                    :disabled="offline || (working !== null && working !== 'yggdrasil')"
                    :aria-expanded="open === 'yggdrasil'"
                    @select="toggle('yggdrasil')"
                >
                    <template #leading>
                        <span
                            class="flex size-9 shrink-0 items-center justify-center rounded-full bg-surface-4 text-[var(--color-text-primary)]"
                        >
                            <GlobeIcon class="size-4" />
                        </span>
                    </template>
                    <span class="font-semibold text-[var(--color-text-primary)]">
                        {{ formatMessage(messages.yggdrasil) }}
                    </span>
                    <span class="text-sm text-[var(--color-text-tertiary)]">
                        {{ formatMessage(messages.yggdrasilDetail) }}
                    </span>
                    <template #trailing>
                        <ChevronRightIcon
                            class="size-5 shrink-0 text-[var(--color-text-tertiary)] transition-transform"
                            :class="{ 'rotate-90': open === 'yggdrasil' }"
                        />
                    </template>
                </OobeChoice>
                <div
                    v-if="open === 'yggdrasil'"
                    class="oobe-reveal mt-1.5 flex flex-col gap-3 rounded-[var(--radius-lg)] border border-solid border-surface-4 bg-surface-2.5 p-4"
                >
                    <template v-if="pendingProfiles.length > 0">
                        <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                            {{ formatMessage(messages.selectProfile) }}
                        </p>
                        <div class="flex flex-wrap gap-2">
                            <Button
                                v-for="profile in pendingProfiles"
                                :key="profile.id"
                                :disabled="working !== null"
                                @click="chooseProfile(profile)"
                            >
                                <UserIcon />
                                {{ profile.name }}
                            </Button>
                        </div>
                    </template>
                    <template v-else>
                        <label class="flex flex-col gap-1.5 text-sm font-semibold">
                            <span class="flex items-center justify-between">
                                {{ formatMessage(messages.apiRootLabel) }}
                                <button
                                    v-if="apiRoot !== LITTLE_SKIN_API_ROOT"
                                    type="button"
                                    class="cursor-pointer border-0 bg-transparent p-0 text-sm font-medium text-brand"
                                    @click="apiRoot = LITTLE_SKIN_API_ROOT"
                                >
                                    {{ formatMessage(messages.useLittleSkin) }}
                                </button>
                            </span>
                            <StyledInput
                                v-model="apiRoot"
                                inputmode="url"
                                :disabled="working !== null"
                            />
                        </label>
                        <div class="grid grid-cols-2 gap-3">
                            <label class="flex flex-col gap-1.5 text-sm font-semibold">
                                {{ formatMessage(messages.accountLabel) }}
                                <StyledInput
                                    v-model="login"
                                    autocomplete="username"
                                    :placeholder="formatMessage(messages.accountPlaceholder)"
                                    :disabled="working !== null"
                                />
                            </label>
                            <label class="flex flex-col gap-1.5 text-sm font-semibold">
                                {{ formatMessage(messages.passwordLabel) }}
                                <StyledInput
                                    v-model="password"
                                    type="password"
                                    autocomplete="current-password"
                                    :disabled="working !== null"
                                    @keyup.enter="signInYggdrasil"
                                />
                            </label>
                        </div>
                        <div class="flex items-center justify-between gap-3">
                            <Checkbox
                                v-model="remember"
                                :disabled="working !== null"
                                :label="formatMessage(messages.rememberPassword)"
                            />
                            <Button
                                type="colored"
                                color="brand"
                                :disabled="!yggdrasilValid || working !== null"
                                @click="signInYggdrasil"
                            >
                                <SpinnerIcon v-if="working === 'yggdrasil'" class="animate-spin" />
                                <LogInIcon v-else />
                                {{ formatMessage(messages.signIn) }}
                            </Button>
                        </div>
                    </template>
                </div>
            </div>

            <div class="flex flex-col">
                <OobeChoice
                    :disabled="working !== null && working !== 'offline'"
                    :aria-expanded="open === 'offline'"
                    @select="toggle('offline')"
                >
                    <template #leading>
                        <span
                            class="flex size-9 shrink-0 items-center justify-center rounded-full bg-surface-4 text-[var(--color-text-primary)]"
                        >
                            <LockIcon v-if="antiPiracy.restricted" class="size-4" />
                            <UserIcon v-else class="size-4" />
                        </span>
                    </template>
                    <span class="font-semibold text-[var(--color-text-primary)]">
                        {{ formatMessage(messages.offline) }}
                    </span>
                    <span class="text-sm text-[var(--color-text-tertiary)]">
                        {{
                            formatMessage(
                                antiPiracy.restricted
                                    ? messages.offlineRestricted
                                    : messages.offlineDetail,
                            )
                        }}
                    </span>
                    <template #trailing>
                        <ChevronRightIcon
                            class="size-5 shrink-0 text-[var(--color-text-tertiary)] transition-transform"
                            :class="{ 'rotate-90': open === 'offline' }"
                        />
                    </template>
                </OobeChoice>
                <div
                    v-if="open === 'offline'"
                    class="oobe-reveal mt-1.5 flex flex-col gap-2 rounded-[var(--radius-lg)] border border-solid border-surface-4 bg-surface-2.5 p-4"
                >
                    <label class="flex flex-col gap-1.5 text-sm font-semibold">
                        {{ formatMessage(messages.usernameLabel) }}
                        <span class="flex gap-2">
                            <StyledInput
                                v-model="username"
                                class="flex-1"
                                autocomplete="off"
                                :maxlength="16"
                                :placeholder="formatMessage(messages.usernamePlaceholder)"
                                :disabled="working !== null"
                                @keyup.enter="addOffline"
                            />
                            <Button
                                type="colored"
                                color="brand"
                                :disabled="!usernameValid || working !== null"
                                @click="addOffline"
                            >
                                <SpinnerIcon v-if="working === 'offline'" class="animate-spin" />
                                {{ formatMessage(messages.add) }}
                            </Button>
                        </span>
                    </label>
                    <p v-if="username.length > 0 && !usernameValid" class="m-0 text-sm text-red">
                        {{ formatMessage(messages.usernameValidation) }}
                    </p>
                    <p
                        v-if="usernameHasHan"
                        class="m-0 rounded-[var(--radius-md)] border border-solid border-orange bg-highlight-orange p-3 text-sm text-[var(--color-text-primary)]"
                    >
                        {{ formatMessage(messages.chineseUsernameWarning) }}
                    </p>
                </div>
            </div>
        </section>
    </div>
</template>

<style scoped lang="scss">
.oobe-reveal {
    animation: oobe-reveal 220ms cubic-bezier(0.22, 1, 0.36, 1);
}

@keyframes oobe-reveal {
    from {
        opacity: 0;
        transform: translateY(-6px);
    }
}

@media (prefers-reduced-motion: reduce) {
    .oobe-reveal {
        animation: none;
    }
}
</style>
