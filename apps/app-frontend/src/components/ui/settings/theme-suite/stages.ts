import { commonMessages, type StageConfigInput } from '@modrinth/ui'
import { markRaw } from 'vue'

import ColorsStage from './ColorsStage.vue'
import type { ThemeSuiteContext } from './context'
import ImportStage from './ImportStage.vue'
import { themeSuiteMessages } from './messages'
import OverviewStage from './OverviewStage.vue'

export const themeSuiteStages: StageConfigInput<ThemeSuiteContext>[] = [
    {
        id: 'overview',
        stageContent: markRaw(OverviewStage),
        title: (ctx) => ctx.formatMessage(themeSuiteMessages.overviewTitle),
        maxWidth: 'min(56rem, calc(100vw - 3rem))',
        nonProgressStage: true,
        leftButtonConfig: null,
        rightButtonConfig: (ctx) => ({
            label: ctx.formatMessage(themeSuiteMessages.done),
            color: 'brand',
            onClick: () => ctx.modal.value?.hide(),
        }),
    },
    {
        id: 'import',
        stageContent: markRaw(ImportStage),
        title: (ctx) => ctx.formatMessage(themeSuiteMessages.importTitle),
        maxWidth: 'min(42rem, calc(100vw - 3rem))',
        nonProgressStage: true,
        leftButtonConfig: (ctx) => ({
            label: ctx.formatMessage(commonMessages.backButton),
            onClick: () => ctx.modal.value?.prevStage(),
        }),
        rightButtonConfig: (ctx) => ({
            label: ctx.formatMessage(
                ctx.installing.value ? themeSuiteMessages.installing : themeSuiteMessages.install,
            ),
            color: 'brand',
            disabled: !ctx.importPath.value,
            loading: ctx.installing.value,
            onClick: () => void ctx.install(),
        }),
    },
    {
        id: 'colors',
        stageContent: markRaw(ColorsStage),
        title: (ctx) => ctx.formatMessage(themeSuiteMessages.colorsTitle),
        maxWidth: 'min(42rem, calc(100vw - 3rem))',
        nonProgressStage: true,
        leftButtonConfig: (ctx) => ({
            label: ctx.formatMessage(commonMessages.backButton),
            onClick: () => ctx.modal.value?.prevStage(),
        }),
        rightButtonConfig: (ctx) => ({
            label: ctx.formatMessage(themeSuiteMessages.saveColors),
            color: 'brand',
            onClick: () => void ctx.saveColors(),
        }),
    },
]
