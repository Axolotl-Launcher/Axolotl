import type { StageConfigInput } from '@modrinth/ui/src/components/base/MultiStageModal.vue'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { markRaw } from 'vue'

import { exportMessages } from '../messages'
import CategoriesStage from './CategoriesStage.vue'
import type { ExportContext } from './context'
import FilesStage from './FilesStage.vue'
import PackageStage from './PackageStage.vue'
// The app's `100vw` is the 1280px launcher window.
export const exportStages: StageConfigInput<ExportContext>[] = [
    {
        id: 'categories',
        stageContent: markRaw(CategoriesStage),
        title: (ctx) => ctx.formatMessage(exportMessages.categories),
        maxWidth: 'min(52rem, calc(1280px - 2rem))',
        cannotNavigateForward: (ctx) => ctx.scanning.value || !ctx.snapshot.value,
        leftButtonConfig: null,
        rightButtonConfig: (ctx) => ({
            label: ctx.formatMessage(exportMessages.next),
            color: 'brand',
            disabled: ctx.scanning.value || !ctx.snapshot.value,
            onClick: () => ctx.modal.value?.nextStage(),
        }),
    },
    {
        id: 'files',
        stageContent: markRaw(FilesStage),
        title: (ctx) => ctx.formatMessage(exportMessages.files),
        maxWidth: 'min(58rem, calc(1280px - 2rem))',
        cannotNavigateForward: (ctx) => !ctx.snapshot.value,
        leftButtonConfig: (ctx) => ({
            label: ctx.formatMessage(commonMessages.backButton),
            onClick: () => ctx.modal.value?.prevStage(),
        }),
        rightButtonConfig: (ctx) => ({
            label: ctx.formatMessage(exportMessages.next),
            color: 'brand',
            disabled: !ctx.snapshot.value,
            onClick: () => ctx.modal.value?.nextStage(),
        }),
    },
    {
        id: 'package',
        stageContent: markRaw(PackageStage),
        title: (ctx) => ctx.formatMessage(exportMessages.package),
        maxWidth: 'min(46rem, calc(1280px - 2rem))',
        disableClose: (ctx) => ctx.exporting.value,
        leftButtonConfig: (ctx) => ({
            label: ctx.formatMessage(commonMessages.backButton),
            disabled: ctx.exporting.value,
            onClick: () => ctx.modal.value?.prevStage(),
        }),
        rightButtonConfig: (ctx) => ({
            label: ctx.formatMessage(exportMessages.export),
            color: 'brand',
            loading: ctx.exporting.value,
            disabled:
                ctx.exporting.value ||
                !ctx.canExport.value ||
                !ctx.packageInfo.value.name.trim() ||
                !ctx.packageInfo.value.version.trim(),
            onClick: () => {
                void ctx.exportPack()
            },
        }),
    },
]
