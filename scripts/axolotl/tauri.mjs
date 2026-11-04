import { app, run } from './desktop-build-utils.mjs'

process.env.CARGO_BUILD_JOBS ||=
    process.platform === 'darwin' && process.arch === 'arm64' ? '2' : '4'

run('pnpm', ['exec', 'tauri', ...process.argv.slice(2)], { cwd: app })
