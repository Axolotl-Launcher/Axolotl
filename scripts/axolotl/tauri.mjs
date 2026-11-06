import { runWithBuildJobs } from './build-jobs.mjs'
import { app } from './desktop-build-utils.mjs'

runWithBuildJobs('pnpm', ['exec', 'tauri', ...process.argv.slice(2)], { cwd: app })
