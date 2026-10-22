/** Accept saved alternate-CDN links without changing their path or query encoding. */
export function normalizeModrinthCdnUrl(input: string): string {
    let url: URL
    try {
        url = new URL(input)
    } catch {
        return input
    }
    if (
        url.protocol !== 'https:' ||
        url.hostname !== 'cdn-alt.modrinth.com' ||
        (url.port !== '' && url.port !== '443') ||
        url.username !== '' ||
        url.password !== ''
    )
        return input
    return input.replace(
        /^(https:\/\/)cdn-alt\.modrinth\.com(?::443)?(?=[/?#]|$)/i,
        '$1cdn.modrinth.com',
    )
}
