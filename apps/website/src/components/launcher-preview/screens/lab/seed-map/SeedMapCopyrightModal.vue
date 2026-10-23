<script setup lang="ts">
import CodeIcon from '@modrinth/assets/icons/code.svg?component'
import ExternalIcon from '@modrinth/assets/icons/external.svg?component'
import ImageIcon from '@modrinth/assets/icons/image.svg?component'
import InfoIcon from '@modrinth/assets/icons/info.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { useTemplateRef } from 'vue'

import { openUrl } from '../../../lib/platform/opener.ts'
import NewModal from '../../../overlays/PreviewModal.vue'
import cubiomesLicense from './LICENSE.cubiomes.txt?raw'

const { formatMessage } = useVIntl()
const modal = useTemplateRef<InstanceType<typeof NewModal>>('modal')

const messages = defineMessages({
    title: {
        id: 'app.lab.seed-map.copyright.title',
        defaultMessage: 'Copyright and attribution',
    },
    iconHeading: {
        id: 'app.lab.seed-map.copyright.icons-heading',
        defaultMessage: 'Map artwork',
    },
    iconBody: {
        id: 'app.lab.seed-map.copyright.icons-body',
        defaultMessage:
            'Some structure and biome icons used by this tool were sourced from MinecraftSearch. MinecraftSearch and the respective creators retain their rights in that artwork.',
    },
    visitMinecraftSearch: {
        id: 'app.lab.seed-map.copyright.visit-minecraft-search',
        defaultMessage: 'Visit MinecraftSearch',
    },
    engineHeading: {
        id: 'app.lab.seed-map.copyright.engine-heading',
        defaultMessage: 'Local map engine',
    },
    engineBody: {
        id: 'app.lab.seed-map.copyright.engine-body',
        defaultMessage:
            'Map predictions are computed locally with xpple/cubiomes, based on cubiomes by Cubitect (MIT License), together with the Axolotl native integration.',
    },
    viewLicense: {
        id: 'app.lab.seed-map.copyright.view-license',
        defaultMessage: 'View MIT License',
    },
    viewCubiomes: {
        id: 'app.lab.seed-map.copyright.view-cubiomes',
        defaultMessage: 'View cubiomes',
    },
    disclaimerHeading: {
        id: 'app.lab.seed-map.copyright.disclaimer-heading',
        defaultMessage: 'Unofficial tool',
    },
    disclaimerBody: {
        id: 'app.lab.seed-map.copyright.disclaimer-body',
        defaultMessage:
            'Minecraft is a trademark of Mojang Synergies AB. Axolotl Launcher is not affiliated with or endorsed by Mojang or MinecraftSearch.',
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
                <ImageIcon aria-hidden="true" />
                <div>
                    <h3>{{ formatMessage(messages.iconHeading) }}</h3>
                    <p>{{ formatMessage(messages.iconBody) }}</p>
                    <Button
                        type="outlined"
                        size="2xs"
                        @click="openUrl('https://minecraftsearch.com')"
                        >{{ formatMessage(messages.visitMinecraftSearch) }}
                        <ExternalIcon />
                    </Button>
                </div>
            </section>

            <section
                class="notice-section grid grid-cols-[1.5rem_minmax(0,1fr)] gap-3 border-b border-surface-5 pb-4"
            >
                <CodeIcon aria-hidden="true" />
                <div>
                    <h3>{{ formatMessage(messages.engineHeading) }}</h3>
                    <p>{{ formatMessage(messages.engineBody) }}</p>
                    <details class="mb-3 rounded-lg border border-surface-4 bg-surface-3 p-2">
                        <summary class="cursor-pointer text-sm">
                            {{ formatMessage(messages.viewLicense) }}
                        </summary>
                        <pre
                            class="mb-0 max-h-48 overflow-y-auto whitespace-pre-wrap break-words text-xs"
                            >{{ cubiomesLicense }}</pre>
                    </details>
                    <Button
                        type="outlined"
                        size="2xs"
                        @click="openUrl('https://github.com/xpple/cubiomes')"
                        >{{ formatMessage(messages.viewCubiomes) }}
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
