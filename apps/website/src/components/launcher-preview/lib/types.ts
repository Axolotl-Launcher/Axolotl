/** Data shapes of the launcher preview, mirroring apps/app-frontend/src/helpers/types.d.ts. */
export type InstanceLoader =
    | 'vanilla'
    | 'forge'
    | 'fabric'
    | 'quilt'
    | 'neoforge'
    | 'optifine'
    | 'lite_loader'
    | 'cleanroom'
    | 'legacy_fabric'
    | 'babric'

export type ReleaseChannel = 'release' | 'beta' | 'alpha'

export type InstallStage =
    'installed' | 'minecraft_installing' | 'pack_installed' | 'pack_installing' | 'not_installed'

export interface LoaderComponent {
    instanceId: string
    kind: InstanceLoader | 'optifabric'
    version?: string | null
    role: 'primary' | 'adjunct'
}

type InstanceLinkIdentity = {
    project_id?: string | null
    version_id?: string | null
    server_project_id?: string | null
    content_project_id?: string | null
    content_version_id?: string | null
}

/** `InstanceLink` of the app's `helpers/types.d.ts`. */
export type InstanceLink = InstanceLinkIdentity &
    (
        | {
              type: 'modrinth_modpack'
              project_id: string
              version_id: string
          }
        | {
              type: 'curseforge_modpack'
              project_id: string
              version_id: string
          }
        | {
              type: 'server_project'
              project_id: string
          }
        | {
              type: 'server_project_modpack'
              server_project_id: string
              content_project_id?: string | null
              content_version_id: string
              project_id?: string
              version_id?: string
          }
        | {
              type: 'imported_modpack'
              project_id?: string | null
              version_id?: string | null
              name?: string | null
              version_number?: string | null
              filename?: string | null
          }
        | {
              type: 'shared_instance'
              shared_instance_id: string
          }
    )

export interface GameInstance {
    id: string
    path: string
    install_stage: InstallStage
    launcher_feature_version: string
    name: string
    icon_path?: string
    symlink_target?: string | null
    game_version: string
    loader: InstanceLoader
    loader_version?: string
    loader_components: LoaderComponent[]
    groups: string[]
    link?: InstanceLink | null
    update_channel: ReleaseChannel
    created: string
    modified: string
    last_played?: string
    pinned_at?: string
    submitted_time_played: number
    recent_time_played: number
    java_path?: string | null
    extra_launch_args?: string[] | null
    memory?: { maximum: number; automatic: boolean; optimize_before_launch: boolean } | null
    force_fullscreen?: boolean | null
    maximize_window?: boolean | null
    game_resolution?: [number, number] | null
    window_title?: string | null
    hooks: { pre_launch?: string | null; wrapper?: string | null; post_exit?: string | null }
}

export interface AccountProfile {
    id: string
    name: string
    skins?: unknown[]
    cape?: unknown
}

export type MinecraftAccount =
    | {
          account_type: 'microsoft'
          account_id: string
          profile: AccountProfile
          access_token?: string
      }
    | {
          account_type: 'offline'
          account_id: string
          profile: AccountProfile
      }
    | {
          account_type: 'yggdrasil'
          account_id: string
          profile: AccountProfile
          yggdrasil?: { server_name?: string; server_url?: string }
      }

export interface RunningProcess {
    uuid: string
    instance: GameInstance
    started_at: string
}

export interface World {
    name: string
    path: string
    instance_id: string
    game_mode: string
    last_played: string
    icon?: string | null
    seed?: number | null
    size: number
    kind: 'singleplayer' | 'server'
    address?: string
}

export interface Screenshot {
    path: string
    instance_id: string
    taken_at: string
    width: number
    height: number
    group?: string | null
}

export interface ServerSummary {
    id: string
    name: string
    icon?: string | null
    address: string
    online: boolean
    players_online: number
    players_max: number
    ping_ms?: number | null
    version?: string | null
    motd?: string | null
}

export interface LocalServer {
    id: string
    name: string
    instance_id: string | null
    server_type: 'vanilla' | 'fabric' | 'forge' | 'neoforge' | 'quilt' | 'paper' | 'purpur'
    game_version: string
    loader_version?: string | null
    port: number
    memory: number
    status: 'stopped' | 'starting' | 'running' | 'stopping' | 'installing' | 'failed'
    created: string
    pinned?: boolean
    motd?: string
    difficulty?: string
    java_path?: string
    jvm_args?: string
    pre_launch?: string
}

export interface ContentItem {
    id: string
    project_id: string
    version_id: string
    name: string
    file_name: string
    enabled: boolean
    icon?: string | null
    version_number: string
    project_type: 'mod' | 'resourcepack' | 'shader' | 'datapack'
}
