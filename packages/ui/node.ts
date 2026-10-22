// Node test entrypoint. Keep this independent from the browser component tree
// and from extensionless TypeScript imports that Node cannot resolve directly.
export function defineMessage(descriptor) {
    return descriptor
}

export function defineMessages(descriptors) {
    return descriptors
}

const messageProxy = new Proxy(
    {},
    {
        get: (_target, key) => ({
            id: `ui.${String(key)}`,
            defaultMessage: String(key),
        }),
    },
)

export const commonMessages = messageProxy
export const formFieldLabels = messageProxy
export const formFieldPlaceholders = messageProxy

export type { ArmorPreviewConfig } from './src/composables/skin-rendering/armor-preview-types.ts'
export {
    ARMOR_SLOTS,
    ARMOR_TRIM_MATERIALS,
    ARMOR_TRIM_PATTERNS,
    armorMaterialsForSlot,
    createDefaultArmorPreviewConfig,
} from './src/composables/skin-rendering/armor-preview-types.ts'
export { confirmedContentGroupKey } from './src/layouts/shared/content-tab/composables/content-duplicates.ts'
