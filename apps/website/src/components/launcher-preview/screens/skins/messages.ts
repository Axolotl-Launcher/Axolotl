import zhCN from '~/locales/launcher-preview/skins.zh-CN'

import { registerPreviewMessages } from '../../lib/i18n'

registerPreviewMessages('zh-CN', zhCN)

/**
 * Strings of the skin selector. Ids and English text are the desktop app's own
 * (apps/app-frontend/src/pages/Skins.vue, components/ui/skin/**,
 * packages/ui/src/components/skin/**); the `skin.preview.*` ids belong to the
 * shared preview renderer and armour panel.
 */
export const messages = {
    skinSelectorTitle: {
        id: 'app.skins.title',
        defaultMessage: 'Skin selector',
    },
    createSkinButton: {
        id: 'app.skins.create-skin',
        defaultMessage: 'Create skin',
    },
    modrinthSection: {
        id: 'app.skins.section.modrinth',
        defaultMessage: 'Modrinth',
    },
    defaultSkinsSection: {
        id: 'app.skins.section.default-skins',
        defaultMessage: 'Default skins',
    },
    mineconEarth2017Section: {
        id: 'app.skins.section.minecon-earth-2017',
        defaultMessage: 'MINECON Earth 2017',
    },
    buildersAndBiomesSection: {
        id: 'app.skins.section.builders-and-biomes',
        defaultMessage: 'Builders & Biomes',
    },
    stridingHeroSection: {
        id: 'app.skins.section.striding-hero',
        defaultMessage: 'Striding Hero',
    },
    theGardenAwakensSection: {
        id: 'app.skins.section.the-garden-awakens',
        defaultMessage: 'The Garden Awakens',
    },
    chaseTheSkiesSection: {
        id: 'app.skins.section.chase-the-skies',
        defaultMessage: 'Chase the Skies',
    },
    theCopperAgeSection: {
        id: 'app.skins.section.the-copper-age',
        defaultMessage: 'The Copper Age',
    },
    mountsOfMayhemSection: {
        id: 'app.skins.section.mounts-of-mayhem',
        defaultMessage: 'Mounts of Mayhem',
    },
    tinyTakeoverSection: {
        id: 'app.skins.section.tiny-takeover',
        defaultMessage: 'Tiny Takeover',
    },
    chaosCubedSection: {
        id: 'app.skins.section.chaos-cubed',
        defaultMessage: 'Chaos Cubed',
    },
    savedSkinsSection: {
        id: 'app.skins.section.saved-skins',
        defaultMessage: 'Saved skins',
    },
    rateLimitTitle: {
        id: 'app.skins.rate-limit.title',
        defaultMessage: 'Slow down!',
    },
    rateLimitText: {
        id: 'app.skins.rate-limit.text',
        defaultMessage:
            "You're changing your skin too frequently. Mojang's servers have temporarily blocked further requests. Please wait a moment before trying again.",
    },
    droppedFileErrorTitle: {
        id: 'app.skins.dropped-file-error.title',
        defaultMessage: 'Error processing file',
    },
    droppedFileErrorText: {
        id: 'app.skins.dropped-file-error.text',
        defaultMessage: 'Failed to read the dropped file.',
    },
    reorderSkinErrorTitle: {
        id: 'app.skins.reorder-error.title',
        defaultMessage: 'Failed to reorder skins',
    },
    reorderSkinErrorText: {
        id: 'app.skins.reorder-error.text',
        defaultMessage: 'Your skin order could not be saved.',
    },
    deleteSkinTitle: {
        id: 'app.skins.delete-modal.title',
        defaultMessage: 'Are you sure you want to delete this skin?',
    },
    deleteSkinDescription: {
        id: 'app.skins.delete-modal.description',
        defaultMessage:
            'This will permanently delete the selected skin. This action cannot be undone.',
    },
    previewingBadge: {
        id: 'app.skins.previewing-badge',
        defaultMessage: 'Previewing',
    },
    applyButton: {
        id: 'app.skins.apply-button',
        defaultMessage: 'Apply',
    },
    previewEditButton: {
        id: 'app.skins.preview.edit-button',
        defaultMessage: 'Edit skin',
    },
    editSkinButton: {
        id: 'app.skins.edit-button',
        defaultMessage: 'Edit skin',
    },
    deleteSkinButton: {
        id: 'app.skins.delete-button',
        defaultMessage: 'Delete skin',
    },
    addSkinButton: {
        id: 'app.skins.add-button',
        defaultMessage: 'Add skin',
    },
    dragAndDropSubtitle: {
        id: 'app.skins.add-button.drag-and-drop',
        defaultMessage: 'Drag and drop',
    },
    axolotlAlt: {
        id: 'app.skins.sign-in.axolotl-alt',
        defaultMessage: 'Axolotl Launcher',
    },
    signInTitle: {
        id: 'app.skins.sign-in.title',
        defaultMessage: 'Please sign in',
    },
    signInDescription: {
        id: 'app.skins.sign-in.description',
        defaultMessage:
            'Please sign into your Minecraft account to use the skin management features of Axolotl Launcher.',
    },
    signInButton: {
        id: 'app.skins.sign-in.button',
        defaultMessage: 'Sign In',
    },
    offlineCompatibilityTitle: {
        id: 'app.skins.offline-account.compatibility-title',
        defaultMessage: 'Offline skin compatibility',
    },
    offlineCompatibility: {
        id: 'app.skins.offline-account.compatibility',
        defaultMessage:
            'Offline skins are applied locally with a resource pack. Compatibility is best on Minecraft 1.6–1.19.2; newer versions are supported on a best-effort basis. The skin is not uploaded to Mojang, may also replace other default-skinned players on your client, and some skin mods or server plugins may override it.',
    },
    thirdPartyManagementTitle: {
        id: 'app.skins.third-party-account.title',
        defaultMessage: 'Third-party skin management',
    },
    thirdPartyManagementDescription: {
        id: 'app.skins.third-party-account.description',
        defaultMessage:
            'Skins for this account are managed by its Yggdrasil provider. Open the provider website to change skins or capes.',
    },
    savedTab: {
        id: 'app.skins.tabs.saved',
        defaultMessage: 'Saved skins',
    },
    defaultTab: {
        id: 'app.skins.tabs.default',
        defaultMessage: 'Official skins',
    },
    armorTitle: {
        id: 'app.skins.armor.title',
        defaultMessage: 'Armour and trims',
    },
    armorDescription: {
        id: 'app.skins.armor.description',
        defaultMessage: 'Try armour and trims on your skin. Nothing is stored until you save it.',
    },
    armorUnsaved: {
        id: 'app.skins.armor.unsaved',
        defaultMessage: 'Unsaved changes',
    },
    armorResetToSaved: {
        id: 'app.skins.armor.reset-to-saved',
        defaultMessage: 'Reset to saved',
    },
    armorRestoreDefaults: {
        id: 'app.skins.armor.restore-defaults',
        defaultMessage: 'Restore defaults',
    },
    armorSave: {
        id: 'app.skins.armor.save',
        defaultMessage: 'Save',
    },
    armorClose: {
        id: 'app.skins.armor.close',
        defaultMessage: 'Close',
    },
    armorPreviewButton: {
        id: 'skin.preview.armor.open',
        defaultMessage: 'Armor and trims',
    },
    dragToRotate: {
        id: 'skin.preview.drag-to-rotate',
        defaultMessage: 'Drag to rotate',
    },
    editSkinTitle: {
        id: 'app.skins.modal.edit-title',
        defaultMessage: 'Editing skin',
    },
    addSkinTitle: {
        id: 'app.skins.modal.add-title',
        defaultMessage: 'Adding a skin',
    },
    textureSection: {
        id: 'app.skins.modal.texture-section',
        defaultMessage: 'Texture',
    },
    replaceTextureButton: {
        id: 'app.skins.modal.replace-texture-button',
        defaultMessage: 'Replace texture',
    },
    armStyleSection: {
        id: 'app.skins.modal.arm-style-section',
        defaultMessage: 'Arm style',
    },
    wideArmStyle: {
        id: 'app.skins.modal.arm-style-wide',
        defaultMessage: 'Wide',
    },
    slimArmStyle: {
        id: 'app.skins.modal.arm-style-slim',
        defaultMessage: 'Slim',
    },
    capeSection: {
        id: 'app.skins.modal.cape-section',
        defaultMessage: 'Cape',
    },
    noCapeTooltip: {
        id: 'app.skins.modal.no-cape-tooltip',
        defaultMessage: 'No cape',
    },
    noneCapeOption: {
        id: 'app.skins.modal.none-cape-option',
        defaultMessage: 'None',
    },
    capeFallbackName: {
        id: 'app.skins.modal.cape-fallback-name',
        defaultMessage: 'Cape',
    },
    savingTooltip: {
        id: 'app.skins.modal.saving-tooltip',
        defaultMessage: 'Saving...',
    },
    uploadSkinFirstTooltip: {
        id: 'app.skins.modal.upload-skin-first-tooltip',
        defaultMessage: 'Upload a skin first!',
    },
    makeEditFirstTooltip: {
        id: 'app.skins.modal.make-edit-first-tooltip',
        defaultMessage: 'Make an edit to the skin first!',
    },
    addSkinModalButton: {
        id: 'app.skins.modal.add-skin-button',
        defaultMessage: 'Add skin',
    },
    saveSkinButton: {
        id: 'app.skins.modal.save-skin-button',
        defaultMessage: 'Save skin',
    },
}

export type MessageKey = keyof typeof messages
