import { run } from './desktop-build-utils.mjs'

export function runWithBuildJobs(
    command,
    args,
    options = {},
    { platform = process.platform, arch = process.arch, env = process.env, runCommand = run } = {},
) {
    return runCommand(command, args, {
        ...options,
        env: {
            ...env,
            CARGO_BUILD_JOBS:
                env.CARGO_BUILD_JOBS || (platform === 'darwin' && arch === 'arm64' ? '2' : '4'),
        },
    })
}
