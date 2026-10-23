import { type Component, defineAsyncComponent } from 'vue'

import zhCN from '~/locales/launcher-preview/about-games.zh-CN'

import { registerPreviewMessages } from '../../../../lib/i18n'

registerPreviewMessages('zh-CN', zhCN)

/**
 * The games hidden behind a long press on a development-team card
 * (apps/app-frontend/src/components/ui/settings/about-member-experiences.ts).
 * Each one loads only when it is opened.
 */
export type AboutMemberExperience = {
    component: Component
    longPressDuration: number
}

const memberExperiences: Readonly<Record<string, AboutMemberExperience>> = {
    'axolotl-merge': {
        component: defineAsyncComponent(() => import('./AboutMergeGame.vue')),
        longPressDuration: 800,
    },
    'axolotl-run': {
        component: defineAsyncComponent(() => import('./AboutAxolotlRun.vue')),
        longPressDuration: 800,
    },
}

export function getAboutMemberExperience(experience: unknown): AboutMemberExperience | undefined {
    return typeof experience === 'string' ? memberExperiences[experience] : undefined
}
