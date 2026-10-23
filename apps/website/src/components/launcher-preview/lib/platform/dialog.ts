/**
 * `@tauri-apps/plugin-dialog` `save` for the preview: a page cannot show the
 * native save panel, so the file goes where the browser saves downloads, under
 * the name the app proposes.
 */
export async function save(options: { defaultPath?: string } = {}): Promise<string | null> {
    return options.defaultPath ?? 'download'
}
