import { useQuery } from '@tanstack/vue-query'
import { computed, type Ref } from 'vue'

import { get_project_v3_many } from '@/helpers/cache.js'
import { instanceServerProjectId, isServerProjectLink } from '@/helpers/instance-project'
import type { GameInstance } from '@/helpers/types'

export function useLinkedServerProjects(instances: Ref<GameInstance[]>) {
    const projectIds = computed(() =>
        [
            ...new Set(
                instances.value
                    .filter((instance) => !isServerProjectLink(instance.link))
                    .map((instance) => instanceServerProjectId(instance.link))
                    .filter((id): id is string => !!id),
            ),
        ].sort(),
    )
    const { data } = useQuery({
        queryKey: computed(() => ['instance-server-projects', 'modrinth', projectIds.value]),
        queryFn: () => get_project_v3_many(projectIds.value, 'must_revalidate'),
        enabled: computed(() => projectIds.value.length > 0),
    })
    const serverProjectIds = computed(
        () =>
            new Set<string>(
                (data.value ?? [])
                    .filter((project) => project?.minecraft_server != null)
                    .map((project) => project.id),
            ),
    )
    return (instance: GameInstance) =>
        isServerProjectLink(instance.link) ||
        serverProjectIds.value.has(instanceServerProjectId(instance.link) ?? '')
}
