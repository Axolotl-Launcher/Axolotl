import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed } from 'vue'

import { messages } from './messages'
import {
    FAVORITES_GROUP_ID,
    type InstanceGroupDefinition,
    SAMPLE_GROUP_IDS,
    useLibrary,
} from './state'

/**
 * `list_groups()` as the components see it: the favorites group first, then
 * the custom groups in their stored order. The two groups the preview starts
 * with read in the visitor's language until they are renamed.
 */
export function useLibraryGroups() {
    const library = useLibrary()
    const { formatMessage } = useVIntl()

    const sampleNames = computed<Record<string, string>>(() => ({
        [SAMPLE_GROUP_IDS.adventure]: formatMessage(messages.sampleGroupAdventure),
        [SAMPLE_GROUP_IDS.performance]: formatMessage(messages.sampleGroupPerformance),
    }))

    const libraryGroups = computed<InstanceGroupDefinition[]>(() =>
        library
            .listGroups()
            .map((group) =>
                group.id === FAVORITES_GROUP_ID || library.groupNames.has(group.id)
                    ? group
                    : { id: group.id, name: sampleNames.value[group.id] ?? group.name },
            ),
    )

    const orderedLibraryGroupIds = computed(() => libraryGroups.value.map((group) => group.id))

    return { library, libraryGroups, orderedLibraryGroupIds }
}
