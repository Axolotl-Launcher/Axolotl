import { type QueryKey, skipToken, useQuery } from '@tanstack/vue-query'
import type { MaybeRef } from 'vue'

import type { GameInstance } from '@/helpers/types'

export function retainInstanceView(queryKey: MaybeRef<QueryKey>) {
    useQuery({ queryKey, queryFn: skipToken, gcTime: 30 * 60_000 })
}

export function instanceDirectoryScope(instance: GameInstance) {
    return [
        instance.id,
        instance.path,
        instance.symlink_target,
        instance.game_dir_override,
        instance.linked_dot_minecraft,
        instance.linked_launcher_root,
        instance.linked_version_id,
        instance.linked_game_dir_mode,
        instance.linked_version_json_path,
    ] as const
}

export function instanceContentKeys(instanceId: string) {
    return ['instance-content-view', instanceId] as const
}

export function instanceContentKey(instance: GameInstance) {
    return [
        ...instanceContentKeys(instance.id),
        instanceDirectoryScope(instance),
        instance.link,
        instance.game_version,
        instance.loader,
        instance.update_channel,
    ] as const
}
