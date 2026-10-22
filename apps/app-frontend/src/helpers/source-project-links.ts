import type { SourceId } from './resources'

const projectBases: Partial<Record<SourceId, string>> = {
    modrinth: 'https://modrinth.com/project/',
    curseforge: 'https://www.curseforge.com/projects/',
    mcarchive: 'https://mcarchive.net/mods/',
}

export function sourceProjectUrl(source: SourceId, locator: string): string | null {
    const base = projectBases[source]
    return base && locator ? `${base}${encodeURIComponent(locator)}` : null
}
