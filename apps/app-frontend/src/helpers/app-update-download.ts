/** Opens the checked release, including prereleases, instead of a site's default channel. */
export async function openManualAppUpdate(
    version: string,
    openUrl: (url: string) => Promise<void> | void,
): Promise<void> {
    await openUrl(
        `https://github.com/Axolotl-Launcher/Axolotl/releases/tag/${encodeURIComponent(`v${version}`)}`,
    )
}
