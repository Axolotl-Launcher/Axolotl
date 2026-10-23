<script setup lang="ts">
import CodeIcon from '@modrinth/assets/icons/code.svg?component'
import ExternalIcon from '@modrinth/assets/icons/external.svg?component'
import ImageIcon from '@modrinth/assets/icons/image.svg?component'
import InfoIcon from '@modrinth/assets/icons/info.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { useTemplateRef } from 'vue'

import { openUrl } from '../../../../lib/platform/opener.ts'
import NewModal from '../../../../overlays/PreviewModal.vue'

const { formatMessage } = useVIntl()
const modal = useTemplateRef<InstanceType<typeof NewModal>>('modal')

const messages = defineMessages({
    title: {
        id: 'app.lab.recipe-generator.copyright.title',
        defaultMessage: 'Copyright and attribution',
    },
    tagsHeading: {
        id: 'app.lab.recipe-generator.copyright.tags-heading',
        defaultMessage: 'Vanilla tags',
    },
    tagsBody: {
        id: 'app.lab.recipe-generator.copyright.tags-body',
        defaultMessage:
            'Legacy expanded tags are sourced from the crafting generator by destruc7i0n (MIT License). Minecraft 26.3 tags come from vanilla data archived by mcmeta.',
    },
    viewTags: {
        id: 'app.lab.recipe-generator.copyright.view-tags',
        defaultMessage: 'View vanilla tags',
    },
    texturesHeading: {
        id: 'app.lab.recipe-generator.copyright.textures-heading',
        defaultMessage: 'Item textures and metadata',
    },
    texturesBody: {
        id: 'app.lab.recipe-generator.copyright.textures-body',
        defaultMessage:
            'Item identifiers, readable names, and icons are sourced from minecraft-textures by destruc7i0n, provided under the GNU General Public License v3.',
    },
    viewTextures: {
        id: 'app.lab.recipe-generator.copyright.view-textures',
        defaultMessage: 'View minecraft-textures',
    },
    disclaimerHeading: {
        id: 'app.lab.recipe-generator.copyright.disclaimer-heading',
        defaultMessage: 'Unofficial tool',
    },
    disclaimerBody: {
        id: 'app.lab.recipe-generator.copyright.disclaimer-body',
        defaultMessage:
            'Minecraft assets are Copyright Mojang Studios / Microsoft and are used only to identify compatible content. Axolotl Launcher is not affiliated with or endorsed by Mojang Studios or Microsoft.',
    },
})

defineExpose({
    show: (event?: MouseEvent) => modal.value?.show(event),
})
</script>

<template>
    <NewModal ref="modal" :header="formatMessage(messages.title)">
        <div class="copyright-notice">
            <section
                class="notice-section grid grid-cols-[1.5rem_minmax(0,1fr)] gap-3 border-b border-surface-5 pb-4"
            >
                <CodeIcon aria-hidden="true" />
                <div>
                    <h3>{{ formatMessage(messages.tagsHeading) }}</h3>
                    <p>{{ formatMessage(messages.tagsBody) }}</p>
                    <Button
                        type="outlined"
                        size="2xs"
                        @click="openUrl('https://github.com/misode/mcmeta')"
                        >{{ formatMessage(messages.viewTags) }}
                        <ExternalIcon />
                    </Button>
                </div>
            </section>

            <section
                class="notice-section grid grid-cols-[1.5rem_minmax(0,1fr)] gap-3 border-b border-surface-5 pb-4"
            >
                <ImageIcon aria-hidden="true" />
                <div>
                    <h3>{{ formatMessage(messages.texturesHeading) }}</h3>
                    <p>{{ formatMessage(messages.texturesBody) }}</p>
                    <Button
                        type="outlined"
                        size="2xs"
                        @click="openUrl('https://github.com/destruc7i0n/minecraft-textures')"
                        >{{ formatMessage(messages.viewTextures) }}
                        <ExternalIcon />
                    </Button>
                </div>
            </section>

            <section
                class="notice-section grid grid-cols-[1.5rem_minmax(0,1fr)] gap-3 border-b border-surface-5 pb-4"
            >
                <InfoIcon aria-hidden="true" />
                <div>
                    <h3>{{ formatMessage(messages.disclaimerHeading) }}</h3>
                    <p>{{ formatMessage(messages.disclaimerBody) }}</p>
                </div>
            </section>
        </div>
    </NewModal>
</template>

<style scoped>
.copyright-notice {
    display: flex;
    width: min(34rem, calc(1280px - 3rem));
    flex-direction: column;
    gap: 1rem;
}

.notice-section:last-child {
    border-bottom: 0;
    padding-bottom: 0;
}

.notice-section > svg {
    width: 1.25rem;
    height: 1.25rem;
    margin-top: 0.1rem;
    color: var(--color-text-tertiary);
}

.notice-section h3 {
    margin: 0;
    color: var(--color-text-primary);
    font-size: 0.95rem;
}

.notice-section p {
    margin: 0.35rem 0 0.75rem;
    color: var(--color-text-tertiary);
    font-size: 0.8rem;
    line-height: 1.5;
}
</style>
