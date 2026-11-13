<script setup lang="ts">
import { ExternalIcon } from '@modrinth/assets'
import { Button, defineMessages, NewModal, useVIntl } from '@modrinth/ui'
import { openUrl } from '@tauri-apps/plugin-opener'
import { computed, onUnmounted, ref } from 'vue'

import { begin_device_login, poll_device_login } from '@/helpers/auth'
import type { MinecraftCredential } from '@/helpers/auth-types'
import { errorText } from '@/helpers/error-text'

type DeviceLoginFlow = {
    device_code: string
    user_code: string
    verification_uri: string
    expires_in: number
    interval: number
}

type DeviceLoginPoll =
    | { status: 'pending'; slow_down: boolean }
    | { status: 'complete'; credentials: MinecraftCredential }

const emit = defineEmits<{
    complete: [credentials: MinecraftCredential]
}>()

const { formatMessage } = useVIntl()
const modal = ref<InstanceType<typeof NewModal> | null>(null)
const busy = ref(false)
const deviceFlow = ref<DeviceLoginFlow | null>(null)
const deviceError = ref<string | null>(null)
let devicePollTimer: ReturnType<typeof setTimeout> | undefined
let deviceExpiresAt = 0

const messages = defineMessages({
    title: { id: 'minecraft-login.title', defaultMessage: 'Sign in to Minecraft' },
    deviceStarting: {
        id: 'minecraft-login.device-starting',
        defaultMessage: 'Requesting a device code...',
    },
    deviceDescription: {
        id: 'minecraft-login.device-description',
        defaultMessage: 'Open the page below on any device, then enter this code.',
    },
    deviceExpired: {
        id: 'minecraft-login.device-expired',
        defaultMessage: 'This device code expired. Start again to receive a new code.',
    },
    openVerification: {
        id: 'minecraft-login.open-verification',
        defaultMessage: 'Open verification page',
    },
})

const verificationUrl = computed(() => deviceFlow.value?.verification_uri ?? '')

function clearDevicePolling() {
    if (devicePollTimer !== undefined) {
        clearTimeout(devicePollTimer)
        devicePollTimer = undefined
    }
}

function resetDeviceLogin() {
    clearDevicePolling()
    deviceFlow.value = null
    deviceError.value = null
    deviceExpiresAt = 0
}

function hide() {
    resetDeviceLogin()
    modal.value?.hide()
}

function showDeviceLogin() {
    resetDeviceLogin()
    modal.value?.show()
    void startDeviceLogin()
}

async function finishLogin(credentials: MinecraftCredential) {
    emit('complete', credentials)
    hide()
}

async function pollDeviceLogin(delay: number) {
    const deviceCode = deviceFlow.value?.device_code
    if (!deviceCode || Date.now() >= deviceExpiresAt) {
        deviceError.value = formatMessage(messages.deviceExpired)
        return
    }

    try {
        const result = (await poll_device_login(deviceCode)) as DeviceLoginPoll
        if (deviceFlow.value?.device_code !== deviceCode) return
        if (result.status === 'complete') {
            await finishLogin(result.credentials)
            return
        }
        const nextDelay = result.slow_down ? delay + 5000 : delay
        devicePollTimer = setTimeout(() => void pollDeviceLogin(nextDelay), nextDelay)
    } catch (error) {
        deviceError.value = errorText(error)
    }
}

async function openDeviceVerification() {
    if (!verificationUrl.value) return
    await openUrl(verificationUrl.value)
}

async function startDeviceLogin() {
    if (busy.value) return
    busy.value = true
    deviceError.value = null
    try {
        const flow = (await begin_device_login()) as DeviceLoginFlow
        deviceFlow.value = flow
        deviceExpiresAt = Date.now() + flow.expires_in * 1000
        void pollDeviceLogin(Math.max(flow.interval, 1) * 1000)
    } catch (error) {
        deviceError.value = errorText(error)
    } finally {
        busy.value = false
    }
}

onUnmounted(clearDevicePolling)

defineExpose({ showDeviceLogin, hide })
</script>

<template>
    <NewModal ref="modal" :header="formatMessage(messages.title)" :on-hide="resetDeviceLogin">
        <div class="flex min-w-[24rem] flex-col gap-4">
            <template v-if="deviceFlow">
                <p class="m-0 text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.deviceDescription) }}
                </p>
                <Button @click="openDeviceVerification"
                    ><ExternalIcon /> {{ formatMessage(messages.openVerification) }}
                </Button>
                <code
                    class="rounded-xl bg-surface-3 px-4 py-3 text-center text-xl font-bold tracking-[0.18em] text-[var(--color-text-primary)]"
                >
                    {{ deviceFlow.user_code }}
                </code>
                <p v-if="deviceError" class="m-0 text-sm text-red">{{ deviceError }}</p>
            </template>
            <template v-else>
                <p v-if="busy" class="m-0 text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.deviceStarting) }}
                </p>
                <p v-if="deviceError" class="m-0 text-sm text-red">{{ deviceError }}</p>
            </template>
        </div>
    </NewModal>
</template>
