/**
 * Extensions the backend routes to the modpack installer. Keep in sync with
 * `parse_command` in `packages/app-lib/src/api/handler.rs`, which also sniffs
 * plain archives for the other pack formats.
 */
const MODPACK_EXTENSIONS = ['.mrpack', '.zip']

/**
 * The shell hands over the name the file was saved with, so the comparison has
 * to ignore case: `.MRPACK` reaches the backend as a modpack too, and dropping
 * the event here would leave the user with nothing happening.
 */
export function isModpackFileName(path: string): boolean {
    const name = path.toLowerCase()
    return MODPACK_EXTENSIONS.some((extension) => name.endsWith(extension))
}
