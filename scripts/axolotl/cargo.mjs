import { runWithBuildJobs } from './build-jobs.mjs'

runWithBuildJobs('cargo', process.argv.slice(2), { cwd: process.cwd() })
