<script setup>
import {
    CheckIcon,
    CopyIcon,
    DownloadIcon,
    DropdownIcon,
    HammerIcon,
    LogInIcon,
    UpdatedIcon,
    WrenchIcon,
    XIcon,
} from '@modrinth/assets'
import {
    Button,
    ButtonLink,
    Collapsible,
    commonMessages,
    defineMessages,
    injectNotificationManager,
    NewModal,
    useVIntl,
} from '@modrinth/ui'
import { useClipboard } from '@vueuse/core'
import { computed, ref } from 'vue'

import { ChatIcon } from '@/assets/icons'
import { AxolotlBrandConfig } from '@/config'
import { trackEvent } from '@/helpers/analytics'
import { login as login_flow, set_default_user } from '@/helpers/auth.js'
import { install_existing_instance } from '@/helpers/install'
import { cancel_directory_change } from '@/helpers/settings.ts'
import { exportErrorLogs } from '@/helpers/utils'
import { handleSevereError } from '@/store/error.js'

const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()

const messages = defineMessages({
    genericTitle: { id: 'app.error.generic-title', defaultMessage: 'An error occurred' },
    minecraftAuthTitle: {
        id: 'app.error.minecraft-auth-title',
        defaultMessage: 'Unable to sign in to Minecraft',
    },
    minecraftSignInTitle: {
        id: 'app.error.minecraft-sign-in-title',
        defaultMessage: 'Sign in to Minecraft',
    },
    directoryTitle: {
        id: 'app.error.directory-title',
        defaultMessage: 'Could not change app directory',
    },
    loaderTitle: { id: 'app.error.loader-title', defaultMessage: 'No loader selected' },
    stateTitle: {
        id: 'app.error.state-title',
        defaultMessage: 'Error initializing Axolotl Launcher',
    },
    networkIssues: { id: 'app.error.network-issues', defaultMessage: 'Network issues' },
    networkDescription: {
        id: 'app.error.network-description',
        defaultMessage:
            'Axolotl Launcher had trouble connecting to Microsoft services. This is often caused by a poor connection. Try again, and use our support article if the issue persists.',
    },
    hostsDescription: {
        id: 'app.error.hosts-description',
        defaultMessage:
            'The connection to Microsoft, Xbox, or Minecraft services was rejected. These services may be blocked by your hosts file. See our support article for steps to fix the issue.',
    },
    supportArticle: { id: 'app.error.support-article', defaultMessage: 'Support article' },
    tryAnotherAccount: {
        id: 'app.error.try-another-account',
        defaultMessage: 'Try another Microsoft account',
    },
    accountDescription: {
        id: 'app.error.account-description',
        defaultMessage:
            'Check that you signed in with the correct account. You may own Minecraft on another Microsoft account.',
    },
    tryAnotherAccountButton: {
        id: 'app.error.try-another-account-button',
        defaultMessage: 'Try another account',
    },
    officialLauncherTitle: {
        id: 'app.error.official-launcher-title',
        defaultMessage: 'Using PC Game Pass, coming from Bedrock, or just bought the game?',
    },
    officialLauncherBefore: {
        id: 'app.error.official-launcher-before',
        defaultMessage: 'Try signing in with the',
    },
    officialLauncher: {
        id: 'app.error.official-launcher',
        defaultMessage: 'official Minecraft Launcher',
    },
    officialLauncherAfter: {
        id: 'app.error.official-launcher-after',
        defaultMessage: 'first. When that is complete, return here and sign in.',
    },
    tryAgain: { id: 'app.error.try-sign-in-again', defaultMessage: 'Try signing in again' },
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
    permissionsTitle: {
        id: 'app.error.permissions-title',
        defaultMessage: 'Change directory permissions',
    },
    permissionsDescription: {
        id: 'app.error.permissions-description',
        defaultMessage:
            'Axolotl Launcher cannot write to the selected directory. Adjust its permissions and try again, or cancel the directory change.',
    },
    spaceTitle: { id: 'app.error.space-title', defaultMessage: 'Not enough space' },
    spaceDescription: {
        id: 'app.error.space-description',
        defaultMessage:
            'The disk containing the selected directory does not have enough free space. Free some space and try again, or cancel the directory change.',
    },
    directoryDescription: {
        id: 'app.error.directory-description',
        defaultMessage:
            'Axolotl Launcher cannot migrate to the selected directory. Contact support for help or cancel the directory change.',
    },
    retryDirectory: {
        id: 'app.error.retry-directory',
        defaultMessage: 'Retry directory change',
    },
    cancelDirectory: {
        id: 'app.error.cancel-directory',
        defaultMessage: 'Cancel directory change',
    },
    minecraftRequired: {
        id: 'app.error.minecraft-required',
        defaultMessage:
            'You are not logged in to any account. Please log in below. If you do not have a licensed account, you can create an offline account or log in with a third-party service in the sidebar.',
    },
    stateDescription: {
        id: 'app.error.state-description',
        defaultMessage:
            'Axolotl Launcher failed to load correctly. A file may be corrupted or an essential file may be missing.',
    },
    stateFixIntro: {
        id: 'app.error.state-fix-intro',
        defaultMessage: 'Try one of the following:',
    },
    stateFixInternet: {
        id: 'app.error.state-fix-internet',
        defaultMessage: 'Check your internet connection, then restart the app.',
    },
    stateFixRedownload: {
        id: 'app.error.state-fix-redownload',
        defaultMessage: 'Download and install the app again.',
    },
    stateDatabaseNewerDescription: {
        id: 'app.error.state-database-newer-description',
        defaultMessage:
            'The database was last opened by a newer version of Axolotl, which upgraded it to a format this version cannot read. Your instances and settings are still there.',
    },
    stateDatabaseNewerFixIntro: {
        id: 'app.error.state-database-newer-fix-intro',
        defaultMessage: 'To start the launcher again, either:',
    },
    stateDatabaseNewerFixUpdate: {
        id: 'app.error.state-database-newer-fix-update',
        defaultMessage: 'Reinstall the newer version of Axolotl - it opens this database as it is.',
    },
    stateDatabaseNewerFixDowngrade: {
        id: 'app.error.state-database-newer-fix-downgrade',
        defaultMessage:
            'If you are trying out a build rather than using the launcher normally, revert the database with scripts/axolotl/downgrade-app-db.mjs instead.',
    },
    loaderDescription: {
        id: 'app.error.loader-description',
        defaultMessage: 'Axolotl Launcher could not find a loader version for this instance.',
    },
    loaderFix: {
        id: 'app.error.loader-fix',
        defaultMessage: 'Repair the instance using the button below.',
    },
    repairInstance: { id: 'app.error.repair-instance', defaultMessage: 'Repair instance' },
    supportDescription: {
        id: 'app.error.support-description',
        defaultMessage:
            'If you still need help, visit our support page and provide the following debug information.',
    },
    getSupport: { id: 'app.error.get-support', defaultMessage: 'Get support' },
    debugInformation: { id: 'app.error.debug-information', defaultMessage: 'Debug information' },
    copyDebugInfo: { id: 'app.error.copy-debug-info', defaultMessage: 'Copy debug information' },
    exportLogs: { id: 'app.error.export-logs', defaultMessage: 'Export error logs' },
    noErrorMessage: { id: 'app.error.no-error-message', defaultMessage: 'No error message.' },
})

const errorModal = ref()
const error = ref()
const closable = ref(true)
const errorCollapsed = ref(false)

const title = ref(formatMessage(messages.genericTitle))
const errorType = ref('unknown')
const supportLink = ref(AxolotlBrandConfig.supportUrl)
const metadata = ref({})

defineExpose({
    async show(errorVal, context, canClose = true, source = null) {
        console.log(errorVal, context, canClose, source)
        closable.value = canClose

        if (errorVal.message && errorVal.message.includes('Minecraft authentication error:')) {
            title.value = formatMessage(messages.minecraftAuthTitle)
            errorType.value = 'minecraft_auth'
            supportLink.value = AxolotlBrandConfig.supportUrl

            if (
                errorVal.message.includes('existing connection was forcibly closed') ||
                errorVal.message.includes('error sending request for url')
            ) {
                metadata.value.network = true
            }
            if (errorVal.message.includes('because the target machine actively refused it')) {
                metadata.value.hostsFile = true
            }
        } else if (errorVal.message && errorVal.message.includes('User is not logged in')) {
            title.value = formatMessage(messages.minecraftSignInTitle)
            errorType.value = 'minecraft_sign_in'
            supportLink.value = AxolotlBrandConfig.supportUrl
        } else if (errorVal.message && errorVal.message.includes('Move directory error:')) {
            title.value = formatMessage(messages.directoryTitle)
            errorType.value = 'directory_move'
            supportLink.value = AxolotlBrandConfig.supportUrl

            if (errorVal.message.includes('directory is not writable')) {
                metadata.value.readOnly = true
            }

            if (errorVal.message.includes('Not enough space')) {
                metadata.value.notEnoughSpace = true
            }
        } else if (
            errorVal.message &&
            errorVal.message.includes('No loader version selected for')
        ) {
            title.value = formatMessage(messages.loaderTitle)
            errorType.value = 'no_loader_version'
            supportLink.value = AxolotlBrandConfig.supportUrl
            metadata.value.instanceId = context.instanceId
        } else if (
            errorVal.message &&
            errorVal.message.includes('was created by a newer Axolotl build')
        ) {
            title.value = formatMessage(messages.stateTitle)
            errorType.value = 'state_db_newer'
            supportLink.value = AxolotlBrandConfig.supportUrl
        } else if (source === 'state_init') {
            title.value = formatMessage(messages.stateTitle)
            errorType.value = 'state_init'
            supportLink.value = AxolotlBrandConfig.supportUrl
        } else {
            title.value = formatMessage(messages.genericTitle)
            errorType.value = 'unknown'
            supportLink.value = AxolotlBrandConfig.supportUrl
            metadata.value = {}
        }

        error.value = errorVal
        errorModal.value.show()
    },
})

const loadingMinecraft = ref(false)
async function loginMinecraft() {
    try {
        loadingMinecraft.value = true
        const loggedIn = await login_flow({
            trouble: formatMessage(messages.loginTrouble),
            browserLogin: formatMessage(messages.loginBrowser),
            deviceCode: formatMessage(messages.loginDeviceCode),
        })

        if (loggedIn) {
            await set_default_user(loggedIn.account_id).catch(handleError)
        }

        await trackEvent('AccountLogIn', { source: 'ErrorModal' })
        loadingMinecraft.value = false
        errorModal.value.hide()
    } catch (err) {
        loadingMinecraft.value = false
        handleSevereError(err)
    }
}

async function cancelDirectoryChange() {
    try {
        await cancel_directory_change()
        window.location.reload()
    } catch (err) {
        handleError(err)
    }
}

function retryDirectoryChange() {
    window.location.reload()
}

const loadingRepair = ref(false)
async function repairInstance() {
    loadingRepair.value = true
    try {
        await install_existing_instance(metadata.value.instanceId, false)
        errorModal.value.hide()
    } catch (err) {
        handleSevereError(err)
    }
    loadingRepair.value = false
}

const hasDebugInfo = computed(
    () =>
        errorType.value === 'directory_move' ||
        errorType.value === 'minecraft_auth' ||
        errorType.value === 'state_init' ||
        errorType.value === 'state_db_newer' ||
        errorType.value === 'no_loader_version',
)

const debugInfo = computed(
    () => error.value.message ?? error.value ?? formatMessage(messages.noErrorMessage),
)

const { copy: copyToClipboard, copied } = useClipboard({ copiedDuring: 3000 })

const exportingLogs = ref(false)

async function exportLogs() {
    exportingLogs.value = true
    try {
        await exportErrorLogs(debugInfo.value)
    } catch (err) {
        handleError(err)
    } finally {
        exportingLogs.value = false
    }
}
</script>

<template>
    <NewModal ref="errorModal" :header="title" :closable="closable">
        <div class="modal-body flex flex-col gap-3 max-w-[550px]">
            <div class="markdown-body">
                <template v-if="errorType === 'minecraft_auth'">
                    <template v-if="metadata.network">
                        <h3>{{ formatMessage(messages.networkIssues) }}</h3>
                        <p>
                            {{ formatMessage(messages.networkDescription) }}
                            <a :href="AxolotlBrandConfig.supportUrl">
                                {{ formatMessage(messages.supportArticle) }}
                            </a>
                        </p>
                    </template>
                    <template v-else-if="metadata.hostsFile">
                        <h3>{{ formatMessage(messages.networkIssues) }}</h3>
                        <p>
                            {{ formatMessage(messages.hostsDescription) }}
                            <a :href="AxolotlBrandConfig.supportUrl">
                                {{ formatMessage(messages.supportArticle) }}
                            </a>
                        </p>
                    </template>
                    <template v-else>
                        <h3>{{ formatMessage(messages.tryAnotherAccount) }}</h3>
                        <p>
                            {{ formatMessage(messages.accountDescription) }}
                        </p>
                        <div class="flex items-center justify-center p-2 gap-2">
                            <button
                                class="btn btn-primary"
                                :disabled="loadingMinecraft"
                                @click="loginMinecraft"
                            >
                                <LogInIcon /> {{ formatMessage(messages.tryAnotherAccountButton) }}
                            </button>
                        </div>
                        <h3>{{ formatMessage(messages.officialLauncherTitle) }}</h3>
                        <p>
                            {{ formatMessage(messages.officialLauncherBefore) }}
                            <a href="https://www.minecraft.net/en-us/download">
                                {{ formatMessage(messages.officialLauncher) }}
                            </a>
                            {{ formatMessage(messages.officialLauncherAfter) }}
                        </p>
                    </template>
                    <div class="flex items-center justify-center p-2 gap-2">
                        <button
                            class="btn btn-primary"
                            :disabled="loadingMinecraft"
                            @click="loginMinecraft"
                        >
                            <LogInIcon /> {{ formatMessage(messages.tryAgain) }}
                        </button>
                    </div>
                </template>
                <template v-if="errorType === 'directory_move'">
                    <template v-if="metadata.readOnly">
                        <h3>{{ formatMessage(messages.permissionsTitle) }}</h3>
                        <p>
                            {{ formatMessage(messages.permissionsDescription) }}
                        </p>
                    </template>
                    <template v-else-if="metadata.notEnoughSpace">
                        <h3>{{ formatMessage(messages.spaceTitle) }}</h3>
                        <p>
                            {{ formatMessage(messages.spaceDescription) }}
                        </p>
                    </template>
                    <template v-else>
                        <p>
                            {{ formatMessage(messages.directoryDescription) }}
                        </p>
                    </template>

                    <div class="flex items-center justify-center p-2 gap-2">
                        <button class="btn" @click="retryDirectoryChange">
                            <UpdatedIcon /> {{ formatMessage(messages.retryDirectory) }}
                        </button>
                        <button class="btn btn-danger" @click="cancelDirectoryChange">
                            <XIcon /> {{ formatMessage(messages.cancelDirectory) }}
                        </button>
                    </div>
                </template>
                <div v-else-if="errorType === 'minecraft_sign_in'">
                    <p>
                        {{ formatMessage(messages.minecraftRequired) }}
                    </p>
                    <div class="flex items-center justify-center p-2 gap-2">
                        <button
                            class="btn btn-primary"
                            :disabled="loadingMinecraft"
                            @click="loginMinecraft"
                        >
                            <LogInIcon /> {{ formatMessage(messages.minecraftSignInTitle) }}
                        </button>
                    </div>
                </div>
                <template v-else-if="errorType === 'state_db_newer'">
                    <p>
                        {{ formatMessage(messages.stateDatabaseNewerDescription) }}
                    </p>
                    <p>{{ formatMessage(messages.stateDatabaseNewerFixIntro) }}</p>
                    <ul>
                        <li>{{ formatMessage(messages.stateDatabaseNewerFixUpdate) }}</li>
                        <li>{{ formatMessage(messages.stateDatabaseNewerFixDowngrade) }}</li>
                    </ul>
                </template>
                <template v-else-if="errorType === 'state_init'">
                    <p>
                        {{ formatMessage(messages.stateDescription) }}
                    </p>
                    <p>{{ formatMessage(messages.stateFixIntro) }}</p>
                    <ul>
                        <li>{{ formatMessage(messages.stateFixInternet) }}</li>
                        <li>{{ formatMessage(messages.stateFixRedownload) }}</li>
                    </ul>
                </template>
                <template v-else-if="errorType === 'no_loader_version'">
                    <p>{{ formatMessage(messages.loaderDescription) }}</p>
                    <p>{{ formatMessage(messages.loaderFix) }}</p>
                    <div class="flex items-center justify-center p-2 gap-2">
                        <button
                            class="btn btn-primary"
                            :disabled="loadingRepair"
                            @click="repairInstance"
                        >
                            <HammerIcon /> {{ formatMessage(messages.repairInstance) }}
                        </button>
                    </div>
                </template>
                <template v-else>
                    {{ debugInfo }}
                </template>
                <template v-if="hasDebugInfo">
                    <div class="w-full h-[1px] bg-surface-5 mb-3"></div>
                    <p>
                        {{ formatMessage(messages.supportDescription) }}
                    </p>
                </template>
            </div>
            <div class="flex items-center gap-2">
                <ButtonLink :href="supportLink" @click="errorModal.hide()">
                    <ChatIcon /> {{ formatMessage(messages.getSupport) }}
                </ButtonLink>
                <Button :disabled="exportingLogs" @click="exportLogs"
                    ><DownloadIcon /> {{ formatMessage(messages.exportLogs) }}
                </Button>
                <Button v-if="closable" @click="errorModal.hide()"
                    ><XIcon /> {{ formatMessage(commonMessages.closeButton) }}
                </Button>
            </div>
            <template v-if="hasDebugInfo">
                <div class="flex flex-col gap-2">
                    <div class="w-full h-[1px] bg-surface-5"></div>

                    <div class="overflow-clip">
                        <button
                            class="flex items-center justify-between w-full bg-transparent border-0 py-4 cursor-pointer"
                            @click="errorCollapsed = !errorCollapsed"
                        >
                            <span
                                class="flex items-center gap-2 text-[var(--color-text-primary)] font-extrabold m-0"
                            >
                                <WrenchIcon class="h-4 w-4" />
                                {{ formatMessage(messages.debugInformation) }}
                            </span>
                            <DropdownIcon
                                class="h-5 w-5 text-[var(--color-text-tertiary)] transition-transform"
                                :class="{ 'rotate-180': !errorCollapsed }"
                            />
                        </button>
                        <Collapsible :collapsed="errorCollapsed">
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
            </template>
        </div>
    </NewModal>
</template>

<style>
.light-mode {
    --color-orange-bg-opaque: rgb(255 163 71);
    --color-orange-bg: color-mix(
        in srgb,
        var(--color-orange-bg-opaque) var(--opacity-ratio-keep-20, 20%),
        transparent
    );
}

.dark-mode,
.oled-mode {
    --color-orange-bg-opaque: rgb(224 131 37);
    --color-orange-bg: color-mix(
        in srgb,
        var(--color-orange-bg-opaque) var(--opacity-ratio-keep-20, 20%),
        transparent
    );
}
</style>

<style scoped lang="scss">
.markdown-body {
    overflow: auto;
}
</style>
