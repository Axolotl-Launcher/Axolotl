import { defineMessages } from '@modrinth/ui/src/composables/i18n.ts'

import zhCN from '~/locales/launcher-preview/lab-skin-editor.zh-CN'

import { registerPreviewMessages } from '../../../lib/i18n'

registerPreviewMessages('zh-CN', zhCN)

/** Strings of the app's LabSkinEditor page. */
export const skinEditorMessages = defineMessages({
    title: { id: 'app.lab.skin-editor.title', defaultMessage: 'Skin editor' },
    loading: { id: 'app.lab.skin-editor.loading', defaultMessage: 'Loading skin editor' },
    loadErrorTitle: {
        id: 'app.lab.skin-editor.load-error-title',
        defaultMessage: 'Skin editor could not be loaded',
    },
    loadErrorDescription: {
        id: 'app.lab.skin-editor.load-error-description',
        defaultMessage: 'The embedded editor did not finish loading. Try again.',
    },
    retry: { id: 'app.lab.skin-editor.retry', defaultMessage: 'Try again' },
    exportSkin: { id: 'app.lab.skin-editor.export-skin', defaultMessage: 'Minecraft skin PNG' },
})
