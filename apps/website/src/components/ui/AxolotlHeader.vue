<script setup lang="ts">
import GithubIcon from '@modrinth/assets/external/github.svg?component'
import DownloadIcon from '@modrinth/assets/icons/download.svg?component'
import HamburgerIcon from '@modrinth/assets/icons/hamburger.svg?component'
import SettingsIcon from '@modrinth/assets/icons/settings.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import { ButtonLink, defineMessages, IconButton, useVIntl } from '@modrinth/ui'
import { type ComponentPublicInstance, resolveComponent } from 'vue'

import AxolotlWordmark from '~/components/brand/AxolotlWordmark.vue'

const NuxtLink = resolveComponent('NuxtLink')

const emit = defineEmits<{
    openSettings: []
}>()

const mobileMenuOpen = ref(false)
const mobileMenuRef = ref<HTMLElement | null>(null)
const mobileMenuButtonRef = ref<ComponentPublicInstance | null>(null)
const { formatMessage } = useVIntl()

// 点击菜单与开关之外的区域时收起移动端菜单
function handleOutsideClick(event: MouseEvent) {
    if (!mobileMenuOpen.value) return

    const target = event.target as Node
    const buttonElement = mobileMenuButtonRef.value?.$el
    if (!mobileMenuRef.value?.contains(target) && !buttonElement?.contains(target)) {
        mobileMenuOpen.value = false
    }
}

onMounted(() => {
    document.addEventListener('click', handleOutsideClick)
})

onBeforeUnmount(() => {
    document.removeEventListener('click', handleOutsideClick)
})

const messages = defineMessages({
    home: { id: 'axolotl-site.navigation.home', defaultMessage: 'Axolotl Launcher home' },
    primary: { id: 'axolotl-site.navigation.primary', defaultMessage: 'Primary navigation' },
    mobile: { id: 'axolotl-site.navigation.mobile', defaultMessage: 'Mobile navigation' },
    features: { id: 'axolotl-site.navigation.features', defaultMessage: 'Features' },
    faq: { id: 'axolotl-site.navigation.faq', defaultMessage: 'FAQ' },
    changelog: { id: 'axolotl-site.navigation.changelog', defaultMessage: 'Changelog' },
    community: { id: 'axolotl-site.navigation.community', defaultMessage: 'Community' },
    openSource: { id: 'axolotl-site.navigation.open-source', defaultMessage: 'Open source' },
    download: { id: 'axolotl-site.navigation.download', defaultMessage: 'Download' },
    openSettings: {
        id: 'axolotl-site.navigation.open-settings',
        defaultMessage: 'Open display settings',
    },
    openMenu: { id: 'axolotl-site.navigation.open-menu', defaultMessage: 'Open navigation' },
    closeMenu: { id: 'axolotl-site.navigation.close-menu', defaultMessage: 'Close navigation' },
})

function openSettings() {
    mobileMenuOpen.value = false
    emit('openSettings')
}
</script>

<template>
    <header class="pointer-events-none absolute left-0 top-0 z-40 w-full">
        <div class="header-inner">
            <NuxtLink
                to="/"
                :aria-label="formatMessage(messages.home)"
                class="button-animation w-fit no-underline"
            >
                <AxolotlWordmark />
            </NuxtLink>

            <nav
                class="desktop-navigation pointer-events-auto hidden items-center gap-1 lg:flex"
                :aria-label="formatMessage(messages.primary)"
            >
                <ButtonLink :as="NuxtLink" to="/#features" type="quiet">
                    {{ formatMessage(messages.features) }}
                </ButtonLink>
                <ButtonLink :as="NuxtLink" to="/#faq" type="quiet">
                    {{ formatMessage(messages.faq) }}
                </ButtonLink>
                <ButtonLink :as="NuxtLink" to="/changelog" type="quiet">
                    {{ formatMessage(messages.changelog) }}
                </ButtonLink>
                <ButtonLink :as="NuxtLink" to="/#community" type="quiet">
                    {{ formatMessage(messages.community) }}
                </ButtonLink>
                <ButtonLink
                    href="https://github.com/Mystic-Stars/Axolotl"
                    target="_blank"
                    rel="noopener"
                    type="quiet"
                >
                    <GithubIcon width="20" height="20" aria-hidden="true" />
                    {{ formatMessage(messages.openSource) }}
                </ButtonLink>
            </nav>

            <div class="header-actions pointer-events-auto flex items-center gap-1">
                <ButtonLink
                    class="desktop-download hidden lg:flex"
                    href="#download"
                    type="colored"
                    color="brand"
                >
                    <DownloadIcon width="20" height="20" aria-hidden="true" />
                    {{ formatMessage(messages.download) }}
                </ButtonLink>
                <IconButton
                    class="header-icon-button"
                    type="quiet"
                    :label="formatMessage(messages.openSettings)"
                    @click="openSettings"
                    ><SettingsIcon width="20" height="20" aria-hidden="true" />
                </IconButton>
                <IconButton
                    ref="mobileMenuButtonRef"
                    class="header-icon-button hidden max-lg:flex"
                    type="quiet"
                    :label="formatMessage(mobileMenuOpen ? messages.closeMenu : messages.openMenu)"
                    :aria-expanded="mobileMenuOpen"
                    @click="mobileMenuOpen = !mobileMenuOpen"
                    ><XIcon v-if="mobileMenuOpen" width="20" height="20" aria-hidden="true" />
                    <HamburgerIcon v-else width="20" height="20" aria-hidden="true" />
                </IconButton>
            </div>
        </div>

        <Transition name="mobile-menu">
            <nav
                v-if="mobileMenuOpen"
                ref="mobileMenuRef"
                class="mobile-navigation hidden max-lg:flex"
                :aria-label="formatMessage(messages.mobile)"
            >
                <NuxtLink to="/#features" @click="mobileMenuOpen = false">
                    {{ formatMessage(messages.features) }}
                </NuxtLink>
                <NuxtLink to="/#faq" @click="mobileMenuOpen = false">
                    {{ formatMessage(messages.faq) }}
                </NuxtLink>
                <NuxtLink to="/changelog" @click="mobileMenuOpen = false">
                    {{ formatMessage(messages.changelog) }}
                </NuxtLink>
                <NuxtLink to="/#community" @click="mobileMenuOpen = false">
                    {{ formatMessage(messages.community) }}
                </NuxtLink>
                <a
                    href="https://github.com/Mystic-Stars/Axolotl"
                    target="_blank"
                    rel="noopener"
                    @click="mobileMenuOpen = false"
                >
                    {{ formatMessage(messages.openSource) }}
                </a>
                <a class="mobile-download" href="#download" @click="mobileMenuOpen = false">
                    <DownloadIcon width="20" height="20" aria-hidden="true" />
                    {{ formatMessage(messages.download) }}
                </a>
            </nav>
        </Transition>
    </header>
</template>

<style scoped lang="scss">
.header-inner {
    display: grid;
    grid-template-columns: 1fr auto;
    align-items: center;
    gap: 0.5rem;
    max-width: 1360px;
    margin: 0 auto;
    padding: 1.15rem 1.5rem;
    pointer-events: auto;
}

.desktop-navigation {
    grid-column: 1 / -1;
    grid-row: 2;
    justify-content: center;

    :deep([data-button]) {
        min-height: 2.25rem;
        border-radius: 0;
        color: var(--color-secondary);
        font-size: 0.78rem;
        font-weight: 700;
        letter-spacing: 0.04em;
        text-transform: uppercase;

        &:hover {
            color: var(--color-contrast);
        }
    }
}

.header-actions {
    grid-column: 2;
    grid-row: 1;
    justify-content: flex-end;
}

.mobile-navigation {
    position: absolute;
    top: 100%;
    right: 1rem;
    width: min(22rem, calc(100% - 2rem));
    flex-direction: column;
    gap: 0.25rem;
    padding: 0.75rem;
    border: 1px solid var(--color-divider);
    border-radius: 1rem;
    background: color-mix(in srgb, var(--surface-3) 92%, transparent);
    box-shadow: 0 1.25rem 3rem rgb(0 0 0 / 22%);
    backdrop-filter: blur(20px) saturate(150%);

    a {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        padding: 0.75rem 0.875rem;
        border-radius: var(--radius-md);
        color: var(--color-base);
        font-weight: 600;
        text-decoration: none;

        &:hover {
            background: var(--surface-3);
        }
    }

    .mobile-download {
        justify-content: center;
        margin-top: 0.25rem;
        background: var(--color-brand);
        color: var(--color-on-brand);
    }
}

.mobile-menu-enter-active,
.mobile-menu-leave-active {
    transition: 160ms ease;
}

.mobile-menu-enter-from,
.mobile-menu-leave-to {
    transform: translateY(-0.5rem) scale(0.98);
    opacity: 0;
}

@media (min-width: 1024px) {
    .header-inner {
        grid-template-columns: auto 1fr auto;
        padding-top: 1.25rem;
    }

    .desktop-navigation {
        grid-column: 2;
        grid-row: 1;
    }

    .header-actions {
        grid-column: 3;
    }
}

@media (max-width: 1024px) {
    .header-actions :deep(.header-icon-button) {
        min-width: 2.75rem;
        min-height: 2.75rem;
    }
}

@media (max-width: 1023px) {
    .header-inner {
        grid-template-columns: 1fr auto;
        padding: 0.875rem 1rem;
    }
}
</style>
