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
