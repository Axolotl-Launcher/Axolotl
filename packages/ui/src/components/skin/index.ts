import { defineAsyncComponent } from 'vue'

export const ArmorPreviewControls = defineAsyncComponent({
    loader: () => import('./ArmorPreviewControls.vue'),
    suspensible: false,
})
export const ArmorTrimPanel = defineAsyncComponent({
    loader: () => import('./ArmorTrimPanel.vue'),
    suspensible: false,
})
export { default as CapeButton } from './CapeButton.vue'
export { default as CapeLikeTextButton } from './CapeLikeTextButton.vue'
export { default as SkinButton } from './SkinButton.vue'
export { default as SkinLikeTextButton } from './SkinLikeTextButton.vue'
export const SkinPreviewRenderer = defineAsyncComponent({
    loader: () => import('./SkinPreviewRenderer.vue'),
    suspensible: false,
})
