<script setup lang="ts">
import { CircleAlertIcon, GlobeIcon, MoreVerticalIcon, SpinnerIcon } from '@modrinth/assets'
import {
    Button,
    ConfirmActionModal,
    defineMessages,
    injectNotificationManager,
    TeleportOverflowMenu,
    useVIntl,
} from '@modrinth/ui'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { openUrl } from '@tauri-apps/plugin-opener'
import { computed, ref, watch } from 'vue'

import {
    beginThirdPartyAuthorization,
    cancelThirdPartyAuthorization,
    checkThirdPartyAccount,
    claimThirdPartyAuthorizationNotification,
    disconnectThirdPartyAccount,
    getThirdPartyAccount,
    thirdPartyAccountKey,
} from '@/helpers/third-party-accounts'

import SettingsSection from './SettingsSection.vue'

const messages = defineMessages({
    description: {
        id: 'app.third-party.description',
        defaultMessage: 'Connect external accounts for use in the launcher.',
    },
    shimu: { id: 'app.third-party.shimu', defaultMessage: 'Shimu Space' },
    purpose: {
        id: 'app.third-party.shimu-purpose',
        defaultMessage: 'Connect your Shimu Space account.',
    },
    identity: {
        id: 'app.third-party.identity',
        defaultMessage: '{nickname} · {username}',
    },
    connected: { id: 'app.third-party.connected', defaultMessage: 'Connected' },
    connectedNotification: {
        id: 'app.third-party.connected-notification',
        defaultMessage: 'Connected to Shimu Space',
    },
    disconnected: { id: 'app.third-party.disconnected', defaultMessage: 'Not connected' },
    expired: { id: 'app.third-party.expired', defaultMessage: 'Sign in again' },
    waiting: { id: 'app.third-party.waiting', defaultMessage: 'Waiting for authorization' },
    exchanging: { id: 'app.third-party.exchanging', defaultMessage: 'Completing connection' },
    connect: { id: 'app.third-party.connect', defaultMessage: 'Connect' },
    reconnect: { id: 'app.third-party.reconnect', defaultMessage: 'Reconnect' },
    cancel: { id: 'app.third-party.cancel', defaultMessage: 'Cancel' },
    more: { id: 'app.third-party.more', defaultMessage: 'Shimu Space account options' },
    check: { id: 'app.third-party.check', defaultMessage: 'Check connection' },
    disconnect: { id: 'app.third-party.disconnect', defaultMessage: 'Disconnect' },
    disconnectTitle: {
        id: 'app.third-party.disconnect-title',
        defaultMessage: 'Disconnect Shimu Space?',
    },
    disconnectDescription: {
        id: 'app.third-party.disconnect-description',
        defaultMessage:
            'This removes the account credential from this launcher. To revoke the authorization at Shimu Space, manage it on their website.',
    },
    denied: {
        id: 'app.third-party.denied',
        defaultMessage: 'Authorization was declined. You can connect again.',
    },
    timedOut: {
        id: 'app.third-party.timed-out',
        defaultMessage: 'Authorization timed out. Connect again to continue.',
    },
    failed: {
        id: 'app.third-party.failed',
        defaultMessage: 'Could not complete the connection. Please try again.',
    },
    loadFailed: {
        id: 'app.third-party.load-failed',
        defaultMessage: 'Could not read the account status.',
    },
    checkFailed: {
        id: 'app.third-party.check-failed',
        defaultMessage: 'Could not check the connection. Please try again.',
    },
    actionFailed: {
        id: 'app.third-party.action-failed',
        defaultMessage: 'The account operation failed. Please try again.',
    },
    retry: { id: 'app.third-party.retry', defaultMessage: 'Retry' },
    unavailable: {
        id: 'app.third-party.unavailable',
        defaultMessage: 'Account connection is unavailable in this build.',
    },
    permissions: {
        id: 'app.third-party.permissions',
        defaultMessage:
            'Authorization: read your profile and posts, and download resources. Authorization expires after 3 days; you will need to sign in again.',
    },
    browser: {
        id: 'app.third-party.browser',
        defaultMessage: 'Complete the authorization in your browser.',
    },
})

const { formatMessage } = useVIntl()
const { addNotification } = injectNotificationManager()
const queryClient = useQueryClient()
const disconnectModal = ref<InstanceType<typeof ConfirmActionModal> | null>(null)
const accountQuery = useQuery({
    queryKey: thirdPartyAccountKey,
    queryFn: getThirdPartyAccount,
    retry: false,
    staleTime: 0,
    refetchInterval: (query) =>
        ['waiting', 'exchanging'].includes(query.state.data?.authorization ?? '') ? 1000 : 30_000,
})
const account = accountQuery.data
const accountLabel = computed(() => {
    const user = account.value?.user
    if (!user) return formatMessage(messages.purpose)
    const nickName = user.nickName?.trim()
    const userName = user.userName.trim()
    return nickName && nickName !== userName
        ? formatMessage(messages.identity, { nickname: nickName, username: userName })
        : userName
})
watch(
    account,
    (value) => {
        if (value && claimThirdPartyAuthorizationNotification(value)) {
            addNotification({
                title: formatMessage(messages.connectedNotification),
                text: accountLabel.value,
                type: 'success',
            })
        }
    },
    { immediate: true },
)
const authorizing = computed(() =>
    ['waiting', 'exchanging'].includes(account.value?.authorization ?? ''),
)
const action = useMutation({
    mutationFn: async (operation: 'connect' | 'cancel' | 'disconnect' | 'check') => {
        if (operation === 'connect') {
            const flow = await beginThirdPartyAuthorization()
            try {
                await openUrl(flow.authorizationUrl)
            } catch (error) {
                await cancelThirdPartyAuthorization(flow.flowId)
                throw error
            }
        } else if (operation === 'cancel') {
            if (account.value?.flowId) await cancelThirdPartyAuthorization(account.value.flowId)
        } else if (operation === 'disconnect') {
            await disconnectThirdPartyAccount()
        } else {
            const checked = await checkThirdPartyAccount()
            queryClient.setQueryData(thirdPartyAccountKey, checked)
        }
    },
    onSettled: () => queryClient.invalidateQueries({ queryKey: thirdPartyAccountKey }),
})
const busy = action.isPending
const status = computed(() => {
    const authorization = account.value?.authorization
    if (authorization === 'waiting' || authorization === 'exchanging') return authorization
    return account.value?.status ?? 'disconnected'
})
const errorMessage = computed(() => {
    if (accountQuery.isError.value) return messages.loadFailed
    if (action.isError.value)
        return action.variables.value === 'check' ? messages.checkFailed : messages.actionFailed
    const authorization = account.value?.authorization
    if (authorization === 'denied') return messages.denied
    if (authorization === 'timedOut') return messages.timedOut
    if (authorization === 'failed') return messages.failed
    return null
})
const menuOptions = computed(() => [
    { id: 'check', label: formatMessage(messages.check), action: () => action.mutate('check') },
    {
        id: 'reconnect',
        label: formatMessage(messages.reconnect),
        action: () => action.mutate('connect'),
        shown: account.value?.available,
    },
    { divider: true },
    {
        id: 'disconnect',
        label: formatMessage(messages.disconnect),
        color: 'red' as const,
        action: () => disconnectModal.value?.show(),
    },
])
</script>

<template>
    <div class="flex flex-col gap-4">
        <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
            {{ formatMessage(messages.description) }}
        </p>
        <SettingsSection>
            <div
                id="settings-target-third-party-shimu"
                tabindex="-1"
                class="flex flex-col gap-4 p-4"
            >
                <div class="flex flex-wrap items-center gap-4">
                    <GlobeIcon
                        class="size-8 shrink-0 text-[var(--color-text-tertiary)]"
                        aria-hidden="true"
                    />
                    <div class="min-w-0 flex-1 basis-48">
                        <h2 class="m-0 text-base font-semibold text-[var(--color-text-primary)]">
                            {{ formatMessage(messages.shimu) }}
                        </h2>
                        <p class="m-0 mt-1 break-words text-sm text-[var(--color-text-tertiary)]">
                            {{ accountLabel }}
                        </p>
                    </div>
                    <span
                        v-if="account"
                        role="status"
                        class="flex items-center gap-2 text-sm"
                        :class="
                            status === 'expired'
                                ? 'text-orange'
                                : 'text-[var(--color-text-tertiary)]'
                        "
                    >
                        <SpinnerIcon
                            v-if="authorizing"
                            class="size-4 animate-spin motion-reduce:animate-none"
                            aria-hidden="true"
                        />
                        <CircleAlertIcon
                            v-else-if="status === 'expired'"
                            class="size-4"
                            aria-hidden="true"
                        />
                        <span
                            v-else
                            class="size-2 rounded-full border border-solid"
                            :class="
                                status === 'connected' ? 'border-green bg-green' : 'border-current'
                            "
                            aria-hidden="true"
                        />
                        {{ formatMessage(messages[status]) }}
                    </span>
                    <div class="flex shrink-0 items-center gap-2">
                        <Button
                            v-if="authorizing"
                            :disabled="busy"
                            @click="action.mutate('cancel')"
                            >{{ formatMessage(messages.cancel) }}</Button
                        >
                        <Button
                            v-else-if="account && account.status !== 'connected'"
                            :disabled="busy || !account.available"
                            @click="action.mutate('connect')"
                        >
                            {{
                                formatMessage(
                                    account.status === 'expired'
                                        ? messages.reconnect
                                        : messages.connect,
                                )
                            }}
                        </Button>
                        <TeleportOverflowMenu
                            v-if="account?.user"
                            :options="menuOptions"
                            :label="formatMessage(messages.more)"
                            :disabled="busy || authorizing"
                        >
                            <MoreVerticalIcon aria-hidden="true" />
                        </TeleportOverflowMenu>
                        <SpinnerIcon
                            v-if="busy || accountQuery.isPending.value"
                            class="size-4 animate-spin motion-reduce:animate-none"
                            aria-hidden="true"
                        />
                    </div>
                </div>
                <p v-if="authorizing" class="m-0 text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.browser) }}
                </p>
                <p v-if="account && !account.available" class="m-0 text-sm text-orange">
                    {{ formatMessage(messages.unavailable) }}
                </p>
                <p
                    v-if="account && account.status !== 'connected' && !authorizing"
                    class="m-0 text-sm text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.permissions) }}
                </p>
                <div v-if="errorMessage" role="alert" class="flex flex-wrap items-center gap-3">
                    <p class="m-0 text-sm text-red">{{ formatMessage(errorMessage) }}</p>
                    <Button
                        v-if="accountQuery.isError.value"
                        :disabled="accountQuery.isFetching.value"
                        @click="accountQuery.refetch()"
                        >{{ formatMessage(messages.retry) }}</Button
                    >
                </div>
            </div>
        </SettingsSection>
        <ConfirmActionModal
            ref="disconnectModal"
            :header="formatMessage(messages.disconnectTitle)"
            :confirm-label="formatMessage(messages.disconnect)"
            :confirm-action="() => action.mutateAsync('disconnect')"
            :confirm-disabled="busy"
            confirm-color="red"
        >
            <p class="m-0">{{ formatMessage(messages.disconnectDescription) }}</p>
        </ConfirmActionModal>
    </div>
</template>
