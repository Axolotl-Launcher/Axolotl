import { invoke } from '@tauri-apps/api/core'

import { resourceProjectProtocol } from './resources'

export type PlanetMinecraftDownload = {
    pageUrl: string
    fileName?: string | null
    directUrl?: string | null
    sha256?: string | null
}

export type PlanetMinecraftVersion = {
    id: string
    name: string
    gameVersions: string[]
    download: PlanetMinecraftDownload
}

export type PlanetMinecraftProject = {
    id: string
    title: string
    pageUrl: string
    summary?: string | null
    versions: PlanetMinecraftVersion[]
}

export function planetMinecraftConnectorAvailable() {
    return invoke<boolean>('plugin:planet-minecraft|planet_minecraft_connector_available')
}

export function searchPlanetMinecraftProjects(query: string, gameVersion?: string | null) {
    return invoke<PlanetMinecraftProject[]>(
        'plugin:planet-minecraft|planet_minecraft_search_projects',
        {
            query,
            gameVersion: gameVersion ?? null,
        },
    )
}

export function getPlanetMinecraftProject(id: string, signal?: AbortSignal) {
    return resourceProjectProtocol<PlanetMinecraftProject>('planet_minecraft', id, signal)
}
