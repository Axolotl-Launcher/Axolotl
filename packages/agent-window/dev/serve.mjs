// Previews the Agent Window in a normal browser without building the launcher.
// It serves src/ unchanged and injects a stand-in for the launcher host that
// supplies the formatted message catalog, sample instances and providers.
//
//   node packages/agent-window/dev/serve.mjs [port]
//
// Query parameters on the page: ?lang=zh-CN, ?theme=light, ?accent=%23f472b6,
// ?ai=off (AI disabled), ?models=0 (no enabled models), ?instances=0,
// ?fail=1 (the next scripted task fails once), ?fail=plan (the next task fails
// right after its plan, which then shows as not carried out), ?speed=0.2
// (plays faster).
// The window's history lives in this origin's localStorage, so clear it from
// the browser's developer tools to see the first-run page again.
import { readFile } from 'node:fs/promises'
import { createServer } from 'node:http'
import { extname, join, normalize } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = fileURLToPath(new URL('../src/', import.meta.url))
const locales = fileURLToPath(new URL('../../../apps/app-frontend/src/locales/', import.meta.url))
const port = Number(process.argv[2]) || 5317
const types = {
    '.html': 'text/html',
    '.css': 'text/css',
    '.js': 'text/javascript',
    '.png': 'image/png',
    '.woff2': 'font/woff2',
}

async function catalog(locale) {
    const read = async (name) =>
        JSON.parse(await readFile(join(locales, name, 'index.json'), 'utf8'))
    const english = await read('en-US')
    const localized = locale === 'en-US' ? english : await read(locale).catch(() => english)
    const prefix = 'app.agent-window.chat.'
    return Object.fromEntries(
        Object.keys(english)
            .filter((id) => id.startsWith(prefix))
            .map((id) => [
                id.slice(prefix.length).replace(/-([a-z])/g, (_, letter) => letter.toUpperCase()),
                (localized[id] || english[id]).message,
            ]),
    )
}

function hostScript(messages) {
    return `;(() => {
    const query = new URLSearchParams(location.search)
    const light = query.get('theme') === 'light'
    const accent = query.get('accent') || (light ? '#db2777' : '#f472b6')
    const models = query.get('models') === '0' ? [] : null
    let policy = { full_access: false, computer_use: false }
    const fail = query.get('fail') === 'plan' ? 'plan' : Number(query.get('fail')) || 0
    window.AgentHost = {
        appearance: {
            locale: query.get('lang') || 'en-US',
            background: light ? '#ebebeb' : '#101215',
            surface: light ? '#f8f8f8' : '#262b34',
            text: light ? '#1a202c' : '#ffffff',
            accent,
            scrollbar: light ? '#96a2b0' : '#3a4250',
            scrollbar_track: light ? '#ebebeb' : '#101215',
            scrollbar_radius: '16px',
            labels: {
                title: 'Axolotl Agent',
                minimize: 'Minimize',
                maximize: 'Maximize',
                restore: 'Restore',
                close: 'Close',
            },
            messages: ${JSON.stringify(messages)},
        },
        getContext: async () => ({
            ai_enabled: query.get('ai') !== 'off',
            providers: [
                {
                    provider_id: 'deepseek',
                    name: 'DeepSeek',
                    models: models || [
                        { id: 'deepseek-chat', name: 'DeepSeek V3.2' },
                        { id: 'deepseek-reasoner', name: 'DeepSeek R1' },
                    ],
                },
                {
                    provider_id: 'openrouter',
                    name: 'OpenRouter',
                    models: models || [
                        { id: 'deepseek/deepseek-chat', name: 'DeepSeek V3.2' },
                        { id: 'qwen/qwen3-max', name: 'Qwen3 Max' },
                    ],
                },
            ].filter((provider) => provider.models.length),
            instances:
                query.get('instances') === '0'
                    ? []
                    : [
                          { id: 'i-mist', name: 'Morning Mist', game_version: '1.21.1', loader: 'fabric' },
                          { id: 'i-vanilla', name: 'Vanilla Survival', game_version: '1.21.4', loader: 'vanilla' },
                          { id: 'i-create', name: 'Create: Above and Beyond', game_version: '1.20.1', loader: 'forge' },
                      ],
            execution_available: false,
        }),
        demo: { fail, speed: Number(query.get('speed')) || 1 },
        getPermissions: async () => policy,
        setPermissions: async (next) => {
            await new Promise((resolve) => setTimeout(resolve, 400))
            policy = { full_access: !!next.full_access, computer_use: !!next.full_access && !!next.computer_use }
            return policy
        },
    }
})()
`
}

createServer(async (request, response) => {
    try {
        const url = new URL(request.url, 'http://localhost')
        if (url.pathname === '/__dev/host.js') {
            const locale = url.searchParams.get('lang') || 'en-US'
            response.setHeader('Content-Type', 'text/javascript; charset=utf-8')
            response.end(hostScript(await catalog(locale)))
            return
        }
        const path = url.pathname === '/' ? '/index.html' : url.pathname
        if (path === '/axolotl.png') {
            response.setHeader('Content-Type', 'image/png')
            response.end(
                await readFile(new URL('../../assets/branding/axolotl.png', import.meta.url)),
            )
            return
        }
        const file = normalize(join(root, path))
        if (!file.startsWith(normalize(root))) throw Object.assign(new Error(), { code: 'ENOENT' })
        let body = await readFile(file)
        if (path === '/index.html') {
            // The host script must run before window.js, which reads window.AgentHost.
            const lang = url.searchParams.get('lang') || 'en-US'
            body = body
                .toString('utf8')
                .replace(
                    '<script defer src="/window.js">',
                    `<script src="/__dev/host.js?lang=${encodeURIComponent(lang)}"></script>\n        <script defer src="/window.js">`,
                )
        }
        response.setHeader('Content-Type', `${types[extname(file)] || 'text/plain'}; charset=utf-8`)
        response.setHeader('Cache-Control', 'no-store')
        response.end(body)
    } catch (error) {
        response.writeHead(error.code === 'ENOENT' ? 404 : 500).end()
    }
}).listen(port, '127.0.0.1', () => {
    console.log(`Agent Window preview: http://127.0.0.1:${port}/`)
})
