import { defineMessages } from '@modrinth/ui/src/composables/i18n.ts'

import zhCN from '~/locales/launcher-preview/accounts.zh-CN'

import { registerPreviewMessages } from '../lib/i18n'

registerPreviewMessages('zh-CN', zhCN)

/** Ids and English text of the desktop app's AccountsCard and MinecraftLoginModal. */
export const accountsMessages = defineMessages({
    notSignedIn: { id: 'minecraft-account.not-signed-in', defaultMessage: 'Not signed in' },
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
    signInButton: {
        id: 'minecraft-account.third-party-modal.sign-in',
        defaultMessage: 'Sign in',
    },
    thirdPartyDesktopOnly: {
        id: 'launcher-preview.accounts.third-party-desktop-only',
        defaultMessage:
            'Signing in to an authentication server needs the Axolotl desktop app. This preview does not send your login anywhere.',
    },
    addOfflineAccount: {
        id: 'minecraft-account.add-offline-account',
        defaultMessage: 'Add offline account',
    },
    offlineAccount: {
        id: 'minecraft-account.offline-account',
        defaultMessage: 'Offline Minecraft account',
    },
    offlineBadge: { id: 'minecraft-account.offline-badge', defaultMessage: 'Offline' },
    officialBadge: { id: 'minecraft-account.official-badge', defaultMessage: 'Official' },
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
    copyUuid: { id: 'minecraft-account.copy-uuid', defaultMessage: 'Copy UUID' },
    removeAccount: { id: 'minecraft-account.remove-account', defaultMessage: 'Remove account' },
    selectAccount: { id: 'minecraft-account.select-account', defaultMessage: 'Select account' },
    minecraftAccount: { id: 'minecraft-account.label', defaultMessage: 'Minecraft account' },
    signInToMinecraft: {
        id: 'minecraft-account.sign-in',
        defaultMessage: 'Sign in to Minecraft',
    },
})

export { messages as accountMessages }

export const minecraftLoginMessages = defineMessages({
    title: { id: 'minecraft-login.title', defaultMessage: 'Sign in to Minecraft' },
    desktopOnly: {
        id: 'launcher-preview.minecraft-login.desktop-only',
        defaultMessage:
            'Signing in with Microsoft needs the Axolotl desktop app. This preview does not contact Microsoft.',
    },
})
