import { onScopeDispose, type Ref, watch } from 'vue'

import type ContentInstallPreviewModal from '@/components/ui/ContentInstallPreviewModal.vue'
import type ResourceOperationPreviewModal from '@/components/ui/ResourceOperationPreviewModal.vue'
import type { ContentInstallContext } from '@/providers/content-install-types'
import type { ContentSelectionContext } from '@/providers/content-selection-types'

export function setupContentInstallPreviews({
    resourcePreview,
    contentPreview,
    contentSelection,
    contentInstall,
}: {
    resourcePreview: Ref<InstanceType<typeof ResourceOperationPreviewModal> | null>
    contentPreview: Ref<InstanceType<typeof ContentInstallPreviewModal> | null>
    contentSelection: Pick<ContentSelectionContext, 'setResourceInstallPreview' | 'setPreviewModal'>
    contentInstall: Pick<ContentInstallContext, 'setResourceInstallPreview'>
}) {
    watch(contentPreview, (modal) => contentSelection.setPreviewModal(modal), {
        immediate: true,
        flush: 'sync',
    })
    watch(
        resourcePreview,
        (modal) => {
            contentSelection.setResourceInstallPreview(modal)
            contentInstall.setResourceInstallPreview(modal)
        },
        { immediate: true, flush: 'sync' },
    )
    onScopeDispose(() => {
        contentSelection.setPreviewModal(null)
        contentSelection.setResourceInstallPreview(null)
        contentInstall.setResourceInstallPreview(null)
    })
}
