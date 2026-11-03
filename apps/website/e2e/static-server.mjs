import { createReadStream, existsSync, statSync } from 'node:fs'
import { createServer } from 'node:http'
import { extname, join, normalize, resolve, sep } from 'node:path'
import { fileURLToPath } from 'node:url'

/**
 * A dependency-free static server for the generated site.
 *
 * Page-level tests must observe the artifact that ships, not a dev server: a dev
 * server renders unminified styles, injects its own client, and serves whatever
 * is in memory, so a passing run there says nothing about the built output. The
 * static preset produces `.output/public` with no server entry point, which is
 * why `nuxi preview` cannot serve it.
 */

const WEBSITE_DIR = fileURLToPath(new URL('..', import.meta.url))
const ROOT = resolve(WEBSITE_DIR, '.output/public')
// Kept in step with `baseURL` in playwright.config.ts. It is deliberately not the
// dev server's port, so a developer running `pnpm website:dev` cannot be mistaken
// for the build output these tests are meant to observe.
const PORT = 4173

if (!existsSync(join(ROOT, 'index.html'))) {
    console.error(
        `The static site is missing at ${ROOT}.\n` +
            'Page-level tests run against the build output, so build it first:\n' +
            '  pnpm --filter @axolotl/website build',
    )
    process.exit(1)
}

const CONTENT_TYPES = new Map([
    ['.css', 'text/css; charset=utf-8'],
    ['.html', 'text/html; charset=utf-8'],
    ['.ico', 'image/x-icon'],
    ['.js', 'text/javascript; charset=utf-8'],
    ['.json', 'application/json; charset=utf-8'],
    ['.mjs', 'text/javascript; charset=utf-8'],
    ['.png', 'image/png'],
    ['.svg', 'image/svg+xml'],
    ['.txt', 'text/plain; charset=utf-8'],
    ['.webp', 'image/webp'],
    ['.woff2', 'font/woff2'],
    ['.xml', 'application/xml; charset=utf-8'],
])

/** Resolve a request path inside the site root, or null when it escapes it. */
function resolveRequestPath(pathname) {
    const decoded = decodeURIComponent(pathname)
    const candidate = resolve(ROOT, normalize(decoded).replace(/^[/\\]+/, ''))
    if (candidate !== ROOT && !candidate.startsWith(ROOT + sep)) return null
    return candidate
}

function send(res, status, headers, body) {
    res.writeHead(status, headers)
    if (body === undefined) res.end()
    else res.end(body)
}

createServer((req, res) => {
    const { pathname } = new URL(req.url ?? '/', 'http://localhost')
    const requested = resolveRequestPath(pathname)

    if (requested === null) {
        send(res, 403, { 'content-type': 'text/plain; charset=utf-8' }, 'Forbidden')
        return
    }

    let filePath = requested
    if (existsSync(filePath) && statSync(filePath).isDirectory()) {
        filePath = join(filePath, 'index.html')
    }

    if (!existsSync(filePath) || !statSync(filePath).isFile()) {
        send(res, 404, { 'content-type': 'text/plain; charset=utf-8' }, 'Not Found')
        return
    }

    const type = CONTENT_TYPES.get(extname(filePath).toLowerCase()) ?? 'application/octet-stream'
    res.writeHead(200, { 'content-type': type, 'content-length': statSync(filePath).size })
    createReadStream(filePath).pipe(res)
}).listen(PORT, () => {
    console.log(`Static site served from ${ROOT} at http://localhost:${PORT}`)
})
