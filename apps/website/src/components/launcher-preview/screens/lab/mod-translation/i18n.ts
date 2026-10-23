import { defineMessages } from '@modrinth/ui/src/composables/i18n.ts'

import zhCN from '~/locales/launcher-preview/lab-a.zh-CN'

import { registerPreviewMessages } from '../../../lib/i18n'

registerPreviewMessages('zh-CN', zhCN)

/** The page's strings of `lab/mod-translation/i18n.ts`. */
export const modTranslationMessages = defineMessages({
    title: { id: 'app.lab.mod-translation.title', defaultMessage: 'Mod translation' },
    description: {
        id: 'app.lab.mod-translation.description',
        defaultMessage: 'Translate any Minecraft mod JAR into Simplified Chinese.',
    },
    inputSection: {
        id: 'app.lab.mod-translation.input-section',
        defaultMessage: 'Input',
    },
    aiSection: {
        id: 'app.lab.mod-translation.ai-section',
        defaultMessage: 'AI and options',
    },
    jobsSection: {
        id: 'app.lab.mod-translation.jobs-section',
        defaultMessage: 'Jobs',
    },
    startHint: {
        id: 'app.lab.mod-translation.start-hint',
        defaultMessage: 'Choose a JAR and an AI model to start.',
    },
    outputPath: {
        id: 'app.lab.mod-translation.output-path',
        defaultMessage: 'Output: {path}',
    },
    selectFile: {
        id: 'app.lab.mod-translation.select-file',
        defaultMessage: 'Choose a mod JAR',
    },
    analyze: {
        id: 'app.lab.mod-translation.analyze',
        defaultMessage: 'Analyze',
    },
    start: {
        id: 'app.lab.mod-translation.start',
        defaultMessage: 'Start translation',
    },
    aiNotConfigured: {
        id: 'app.lab.mod-translation.ai-not-configured',
        defaultMessage:
            'AI is not configured. Open the AI settings to enable a provider and model.',
    },
})
