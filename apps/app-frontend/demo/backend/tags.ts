import tags from '../data/tags.json'
import type { Handlers } from './types'

/** Modrinth's tag endpoints, recorded (see data/sources.json). */
export const tagHandlers: Handlers = {
    'plugin:tags|tags_get_game_versions': () => tags.gameVersions,
    'plugin:tags|tags_get_loaders': () => tags.loaders,
    'plugin:tags|tags_get_categories': () => tags.categories,
    'plugin:tags|tags_get_licenses': () => tags.licenses,
}
