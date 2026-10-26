<script setup lang="ts">
import { defineMessages, useVIntl } from '@modrinth/ui'

const { formatMessage } = useVIntl()

const messages = defineMessages({
    features: { id: 'axolotl-site.navigation.features', defaultMessage: 'Features' },
    faq: { id: 'axolotl-site.navigation.faq', defaultMessage: 'FAQ' },
    downloads: { id: 'axolotl-site.footer.downloads', defaultMessage: 'Downloads' },
    changelog: { id: 'axolotl-site.footer.release-history', defaultMessage: 'Changelog' },
    community: { id: 'axolotl-site.navigation.community', defaultMessage: 'Community' },
    terms: { id: 'axolotl-site.navigation.terms', defaultMessage: 'Terms of Service' },
    privacy: { id: 'axolotl-site.navigation.privacy', defaultMessage: 'Privacy Policy' },
    developer: { id: 'axolotl-site.footer.developer', defaultMessage: 'Axolotl Launcher Team' },
    openSourcePrefix: {
        id: 'axolotl-site.footer.open-source-prefix',
        defaultMessage: 'Axolotl is',
    },
    openSourceLink: {
        id: 'axolotl-site.footer.open-source-link',
        defaultMessage: 'open source',
    },
    // The hero used to carry the tech stack; it lives here now, where a visitor
    // looking for implementation details will scroll to.
    techStack: {
        id: 'axolotl-site.footer.tech-stack',
        defaultMessage: 'Built with Tauri v2, Rust, and Vue 3',
    },
    disclaimer: {
        id: 'axolotl-site.footer.minecraft-disclaimer',
        defaultMessage:
            'NOT AN OFFICIAL MINECRAFT SERVICE. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.',
    },
})
</script>

<template>
    <footer
        class="site-footer relative border-t border-divider bg-surface-1 text-[var(--color-base)]"
    >
        <div class="footer-inner">
            <div
                class="footer-legal flex flex-wrap gap-x-[1.4rem] gap-y-[0.4rem] text-[0.78rem] text-[var(--color-secondary)]"
            >
                <p>© {{ new Date().getFullYear() }} {{ formatMessage(messages.developer) }}</p>
                <p class="open-source">
                    <span>
                        {{ formatMessage(messages.openSourcePrefix) }}
                        <a
                            href="https://github.com/Mystic-Stars/Axolotl"
                            target="_blank"
                            rel="noopener"
                        >
                            {{ formatMessage(messages.openSourceLink) }} </a
                        >.
                    </span>
                </p>
                <p>{{ formatMessage(messages.techStack) }}</p>
            </div>

            <nav
                class="footer-links flex flex-wrap justify-end gap-x-[1.3rem] gap-y-[0.35rem]"
                aria-label="Footer"
            >
                <NuxtLink to="/#features">{{ formatMessage(messages.features) }}</NuxtLink>
                <NuxtLink to="/#faq">{{ formatMessage(messages.faq) }}</NuxtLink>
                <a href="#download">{{ formatMessage(messages.downloads) }}</a>
                <NuxtLink to="/changelog">{{ formatMessage(messages.changelog) }}</NuxtLink>
                <NuxtLink to="/#community">{{ formatMessage(messages.community) }}</NuxtLink>
                <NuxtLink to="/terms">{{ formatMessage(messages.terms) }}</NuxtLink>
                <NuxtLink to="/privacy">{{ formatMessage(messages.privacy) }}</NuxtLink>
            </nav>

            <p
                class="col-span-full m-0 text-center text-[0.8125rem] tracking-[0.015em] text-[var(--color-secondary)]"
            >
                {{ formatMessage(messages.disclaimer) }}
            </p>
        </div>
    </footer>
</template>

<style scoped lang="scss">
.site-footer {
    &::before {
        position: absolute;
        top: -1px;
        left: 50%;
        width: min(76rem, calc(100% - 3rem));
        height: 1px;
        transform: translateX(-50%);
        background: linear-gradient(90deg, transparent, var(--color-brand), transparent);
        content: '';
        opacity: 0.6;
    }
}

.footer-inner {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: 0.8rem 2.5rem;
    width: min(76rem, calc(100% - 3rem));
    margin: 0 auto;
    padding: 1.75rem 0;
}

.footer-legal {
    flex-direction: column;
    align-items: flex-start;

    .open-source {
        display: flex;
        align-items: center;
        flex-wrap: wrap;
        gap: 0.5rem;
    }

    p {
        margin: 0;
    }

    a {
        color: var(--color-base);
        text-underline-offset: 0.15rem;
    }
}

.footer-links {
    a {
        color: var(--color-secondary);
        font-size: 0.82rem;
        font-weight: 500;
        text-decoration: none;
        transition: color 140ms ease;

        &:hover {
            color: var(--color-brand);
        }
    }
}

@media (max-width: 900px) {
    .footer-inner {
        grid-template-columns: 1fr;
    }

    .footer-links {
        justify-content: flex-start;
    }
}

@media (max-width: 560px) {
    .footer-inner {
        width: calc(100% - 2rem);
    }
}
</style>
