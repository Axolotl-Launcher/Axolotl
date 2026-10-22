import { execFileSync, spawn } from 'node:child_process'
import { appendFileSync, mkdirSync, readFileSync } from 'node:fs'
import os from 'node:os'
import path from 'node:path'

const args = process.argv.slice(2)
const diagnosticsDirectory = path.join(
  process.env.RUNNER_TEMP || os.tmpdir(),
  'axolotl-release-debug',
)

function record(label, contents) {
  const message = `[release-debug] ${new Date().toISOString()} ${label}\n${contents}\n`
  process.stdout.write(message)
  try {
    mkdirSync(diagnosticsDirectory, { recursive: true })
    appendFileSync(path.join(diagnosticsDirectory, 'resources.log'), message)
  } catch (error) {
    process.stderr.write(`[release-debug] Cannot save diagnostics: ${error.message}\n`)
  }
}

function readMetric(file) {
  try {
    return readFileSync(file, 'utf8').trim()
  } catch (error) {
    return `unavailable (${error.code})`
  }
}

function commandOutput(command, commandArgs) {
  try {
    return execFileSync(command, commandArgs, {
      encoding: 'utf8',
      timeout: 5000,
      maxBuffer: 2 * 1024 * 1024,
      stdio: ['ignore', 'pipe', 'pipe'],
    }).trim()
  } catch (error) {
    return `unavailable (${error.code || error.status}): ${String(error.stderr || '').trim()}`
  }
}

function collectKernelMessages() {
  for (const [command, commandArgs] of [
    ['sudo', ['-n', 'dmesg', '--ctime']],
    ['sudo', ['-n', 'journalctl', '--kernel', '--boot', '--no-pager', '--lines=200']],
  ]) {
    const output = commandOutput(command, commandArgs)
    const events = output
      .split('\n')
      .filter((line) => /oom|out of memory|killed process|memory cgroup|segfault|hung task/i.test(line))
    record(
      `${commandArgs[1]} memory/crash events`,
      output.startsWith('unavailable')
        ? output
        : events.slice(-40).join('\n') || 'No matching kernel events available.',
    )
  }
}

function sample(reason) {
  try {
    const metrics = [
      `reason=${reason}`,
      readMetric('/proc/meminfo')
        .split('\n')
        .filter((line) => /^(MemTotal|MemFree|MemAvailable|SwapTotal|SwapFree|Dirty|Writeback):/.test(line))
        .join('\n'),
      `loadavg: ${readMetric('/proc/loadavg')}`,
      readMetric('/proc/vmstat')
        .split('\n')
        .filter((line) => /^(oom_kill|pgmajfault|pswpin|pswpout) /.test(line))
        .join('\n'),
    ]
    for (const resource of ['memory', 'cpu', 'io']) {
      metrics.push(`pressure/${resource}: ${readMetric(`/proc/pressure/${resource}`)}`)
    }
    const cgroup = readMetric('/proc/self/cgroup')
      .split('\n')
      .find((line) => line.startsWith('0::'))
      ?.slice(3)
    const cgroupDirectories = new Set(['/sys/fs/cgroup'])
    if (cgroup) cgroupDirectories.add(path.join('/sys/fs/cgroup', cgroup))
    for (const directory of cgroupDirectories) {
      for (const name of [
        'memory.current',
        'memory.peak',
        'memory.max',
        'memory.swap.current',
        'memory.swap.max',
        'memory.events',
      ]) {
        metrics.push(`${directory}/${name}: ${readMetric(path.join(directory, name))}`)
      }
    }
    metrics.push(
      'Largest processes (RSS in KiB; command names only):',
      commandOutput('ps', ['-eo', 'pid,ppid,comm,rss,pcpu,pmem', '--sort=-rss'])
        .split('\n')
        .slice(0, 16)
        .join('\n'),
    )
    record('Resource sample', metrics.join('\n'))
  } catch (error) {
    record('Sampling failed', error.message)
  }
}

if (args[0] === '--collect') {
  sample('workflow cleanup')
  collectKernelMessages()
} else {
  const [command, ...commandArgs] = args
  if (!command) throw new Error('Expected a command to diagnose, or --collect')
  const monitored = process.platform === 'linux' && commandArgs.includes('build')
  if (monitored) {
    record(
      'Build environment',
      [
        `platform=${process.platform} architecture=${os.arch()} kernel=${os.release()}`,
        `cpus=${os.availableParallelism()} totalMemoryBytes=${os.totalmem()}`,
        `ImageOS=${process.env.ImageOS || ''} ImageVersion=${process.env.ImageVersion || ''}`,
        `node=${process.version}`,
        `CARGO_PROFILE_RELEASE_LTO=${process.env.CARGO_PROFILE_RELEASE_LTO || ''}`,
        commandOutput('rustc', ['--version', '--verbose']),
        commandOutput('df', ['-h', process.cwd(), os.tmpdir()]),
      ].join('\n'),
    )
    sample('before build')
    collectKernelMessages()
  }

  const child = spawn(command, commandArgs, {
    stdio: 'inherit',
    detached: process.platform === 'linux',
  })
  let receivedSignal
  let forceKillTimer
  const sampleTimer = monitored ? setInterval(() => sample('building'), 30000) : undefined

  function forwardSignal(signal) {
    try {
      if (process.platform === 'linux') process.kill(-child.pid, signal)
      else child.kill(signal)
    } catch (error) {
      if (error.code !== 'ESRCH') record('Signal forwarding failed', error.message)
    }
  }

  for (const signal of ['SIGINT', 'SIGTERM', 'SIGHUP']) {
    process.on(signal, () => {
      receivedSignal = signal
      if (monitored) record('Wrapper received signal', signal)
      forwardSignal(signal)
      if (!forceKillTimer) {
        forceKillTimer = setTimeout(() => forwardSignal('SIGKILL'), 10000)
        forceKillTimer.unref()
      }
    })
  }

  child.on('error', (error) => {
    process.stderr.write(`[release-debug] Cannot start build command: ${error.message}\n`)
    process.exitCode = 127
  })
  child.on('close', (code, signal) => {
    clearInterval(sampleTimer)
    clearTimeout(forceKillTimer)
    if (monitored) {
      record('Build command exited', `exitCode=${code} signal=${signal || ''}`)
      sample('after build')
      collectKernelMessages()
    }
    const exitSignal = receivedSignal || signal
    process.exitCode = exitSignal
      ? 128 + os.constants.signals[exitSignal]
      : code !== null && code >= 0
        ? code
        : 127
  })
}
