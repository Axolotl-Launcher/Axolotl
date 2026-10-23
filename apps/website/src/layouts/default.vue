<script setup lang="ts">
import AxolotlFooter from '~/components/ui/AxolotlFooter.vue'
import AxolotlHeader from '~/components/ui/AxolotlHeader.vue'
import SiteSettingsModal from '~/components/ui/SiteSettingsModal.vue'

const settingsOpen = ref(false)
</script>

<template>
    <div class="site-layout">
        <AxolotlHeader @open-settings="settingsOpen = true" />
        <main id="main">
            <slot />
        </main>
        <AxolotlFooter />
        <SiteSettingsModal v-model="settingsOpen" />
    </div>
</template>

<style lang="scss">
@import '~/assets/styles/global.scss';

html.accent-pink.light-mode {
    --color-on-brand: #ffffff;
    --landing-color-heading: #281b24;
    --landing-color-subheading: #65515e;
    --landing-transition-gradient-end: var(--surface-1);
    --landing-border-color: rgb(166 107 138 / 32%);
}

html.accent-pink.dark-mode {
    --color-on-brand: #1a1018;
    --landing-color-heading: #fff8fc;
    --landing-color-subheading: #bdb0ba;
    --landing-transition-gradient-end: var(--surface-1);
    --landing-border-color: rgb(230 170 204 / 24%);
}

// OLED 把 html 的类切成 `accent-pink oled-mode`，既不是 light-mode 也不是 dark-mode。
// `.oled-mode` 在共享变量里 `@extend .dark-mode`，品牌色仍是粉，遮罩文字沿用深色值；
// `--surface-1` 在 OLED 下是纯黑，过渡带末端因此也要跟着走变量而不是写死。
html.accent-pink.oled-mode {
    --color-on-brand: #1a1018;
    --landing-transition-gradient-end: var(--surface-1);
}

html {
    scroll-behavior: smooth;
    scroll-padding-top: 5rem;
}

.site-layout {
    // svh 以小视口（工具栏展开时）为基准，避免移动端 Firefox/Chrome 动态工具栏导致的高度偏差
    min-height: 100vh;
    min-height: 100svh;
    background: linear-gradient(rgb(255 255 255 / 1%) 1px, transparent 1px), var(--color-bg);
    background-size: 100% 8rem;
}

.site-layout > main {
    min-height: calc(100vh - 4.5rem);
    min-height: calc(100svh - 4.5rem);
}

.reduced-effects * {
    backdrop-filter: none !important;
}

.reduced-motion,
.reduced-motion * {
    scroll-behavior: auto !important;
    animation-duration: 0.001ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.001ms !important;
}

@media (prefers-reduced-motion: reduce) {
    *,
    *::before,
    *::after {
        scroll-behavior: auto !important;
        animation-duration: 0.001ms !important;
        animation-iteration-count: 1 !important;
        transition-duration: 0.001ms !important;
    }
}
</style>
