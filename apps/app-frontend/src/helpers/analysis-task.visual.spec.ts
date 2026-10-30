import { expect, it, vi } from 'vitest'

import { type AnalysisRunContext, createAnalysisTask } from './analysis-task'

type Context = AnalysisRunContext<number, string[]>

function controlledRun() {
    const runs: {
        input: string
        context: Context
        resolve: (value: string[]) => void
        reject: (error: unknown) => void
    }[] = []
    const run = vi.fn(
        (input: string, context: Context) =>
            new Promise<string[]>((resolve, reject) => {
                runs.push({ input, context, resolve, reject })
            }),
    )
    return { run, runs }
}
const immediate = (commit: () => void) => commit()
const tick = () => new Promise((resolve) => setTimeout(resolve, 0))

it('goes from queued through processing to ready and keeps the result', async () => {
    const { run, runs } = controlledRun()
    const task = createAnalysisTask({ label: 'test', run, schedule: immediate })
    const done = task.start('instance')
    expect(task.status.value).toBe('queued')
    await tick()
    expect(task.status.value).toBe('processing')
    expect(task.running.value).toBe(true)
    runs[0].context.progress(1)
    runs[0].context.partial(['first'])
    expect(task.progress.value).toBe(1)
    expect(task.partial.value).toEqual(['first'])
    runs[0].resolve(['first', 'second'])
    await expect(done).resolves.toEqual(['first', 'second'])
    expect(task.status.value).toBe('ready')
    expect(task.result.value).toEqual(['first', 'second'])
    expect(task.running.value).toBe(false)
})

it('keeps the results found so far when a run fails, and retries with the same input', async () => {
    const { run, runs } = controlledRun()
    const task = createAnalysisTask({ label: 'test', run, schedule: immediate })
    void task.start('instance')
    await tick()
    runs[0].context.partial(['found'])
    runs[0].reject(new Error('offline'))
    await tick()
    expect(task.status.value).toBe('error')
    expect(task.partial.value).toEqual(['found'])
    void task.retry()
    await tick()
    expect(runs[1].input).toBe('instance')
    expect(task.status.value).toBe('processing')
})

it('stops accepting results at once on cancel and reports canceled only after the stop is confirmed', async () => {
    const { run, runs } = controlledRun()
    let confirm!: () => void
    const stop = vi.fn(
        () =>
            new Promise<void>((resolve) => {
                confirm = resolve
            }),
    )
    const task = createAnalysisTask({ label: 'test', run, stop, schedule: immediate })
    void task.start('instance')
    await tick()
    runs[0].context.partial(['kept'])
    const canceled = task.cancel()
    expect(runs[0].context.signal.aborted).toBe(true)
    expect(task.status.value).toBe('canceling')
    runs[0].context.partial(['late'])
    runs[0].resolve(['late'])
    await tick()
    expect(task.partial.value).toEqual(['kept'])
    expect(task.result.value).toBeNull()
    expect(task.status.value).toBe('canceling')
    confirm()
    await canceled
    expect(stop).toHaveBeenCalledWith('instance')
    expect(task.status.value).toBe('canceled')
    expect(task.running.value).toBe(false)
})

it('does not show a run as stopped when the stop request fails', async () => {
    const { run } = controlledRun()
    const task = createAnalysisTask({
        label: 'test',
        run,
        stop: async () => {
            throw new Error('stop failed')
        },
        schedule: immediate,
    })
    void task.start('instance')
    await tick()
    await task.cancel()
    expect(task.status.value).toBe('error')
    expect(task.error.value).toBeInstanceOf(Error)
})

it('a new start abandons the previous run and never reuses its results', async () => {
    const { run, runs } = controlledRun()
    const stop = vi.fn()
    const task = createAnalysisTask({ label: 'test', run, stop, schedule: immediate })
    const first = task.start('first')
    await tick()
    void task.start('second')
    expect(runs[0].context.signal.aborted).toBe(true)
    expect(stop).toHaveBeenCalledWith('first')
    runs[0].context.progress(99)
    runs[0].resolve(['stale'])
    await expect(first).resolves.toBeUndefined()
    await tick()
    expect(task.progress.value).toBeNull()
    expect(task.result.value).toBeNull()
    expect(task.input.value).toBe('second')
})

it('coalesces frequent updates into one commit and drops them when the run ends first', async () => {
    const { run, runs } = controlledRun()
    const commits: (() => void)[] = []
    const task = createAnalysisTask({
        label: 'test',
        run,
        schedule: (commit) => commits.push(commit),
    })
    void task.start('instance')
    await tick()
    for (let value = 1; value <= 50; value++) runs[0].context.progress(value)
    expect(commits).toHaveLength(1)
    commits[0]()
    expect(task.progress.value).toBe(50)
    runs[0].context.partial(['pending'])
    runs[0].resolve(['final'])
    await tick()
    expect(task.status.value).toBe('ready')
    commits[1]()
    expect(task.partial.value).toEqual(['final'])
})

it('dispose cancels the current run and refuses later starts', async () => {
    const { run, runs } = controlledRun()
    const task = createAnalysisTask({ label: 'test', run, schedule: immediate })
    void task.start('instance')
    await tick()
    task.dispose()
    expect(runs[0].context.signal.aborted).toBe(true)
    expect(task.status.value).toBe('canceled')
    await expect(task.start('again')).resolves.toBeUndefined()
    expect(run).toHaveBeenCalledOnce()
})

it('reset abandons the current run and returns to idle without results', async () => {
    const { run, runs } = controlledRun()
    const task = createAnalysisTask({ label: 'test', run, schedule: immediate })
    void task.start('instance')
    await tick()
    runs[0].context.partial(['found'])
    task.reset()
    expect(runs[0].context.signal.aborted).toBe(true)
    expect(task.status.value).toBe('idle')
    expect(task.partial.value).toBeNull()
    expect(task.input.value).toBeNull()
    runs[0].resolve(['late'])
    await tick()
    expect(task.result.value).toBeNull()
    void task.start('again')
    await tick()
    expect(run).toHaveBeenCalledTimes(2)
})

it('waits for the stop the run registered and reports a failed stop as an error', async () => {
    let acknowledge!: () => void
    let refuse!: (error: Error) => void
    const run = vi.fn(
        (_input: string, context: Context) =>
            new Promise<string[]>(() => {
                context.signal.addEventListener('abort', () =>
                    context.stopping(
                        new Promise<void>((resolve, reject) => {
                            acknowledge = resolve
                            refuse = reject
                        }),
                    ),
                )
            }),
    )
    const task = createAnalysisTask({ label: 'test', run, schedule: immediate })
    void task.start('first')
    await tick()
    const canceled = task.cancel()
    await tick()
    expect(task.status.value).toBe('canceling')
    acknowledge()
    await canceled
    expect(task.status.value).toBe('canceled')

    void task.start('second')
    await tick()
    const failed = task.cancel()
    refuse(new Error('planner did not stop'))
    await failed
    expect(task.status.value).toBe('error')
    expect(task.running.value).toBe(false)
})

it('keeps the results queued for the next frame when the run is canceled', async () => {
    const { run, runs } = controlledRun()
    const task = createAnalysisTask({ label: 'test', run, schedule: () => {} })
    void task.start('instance')
    await tick()
    runs[0].context.partial(['published just before the cancel'])
    expect(task.partial.value).toBeNull()
    await task.cancel()
    expect(task.partial.value).toEqual(['published just before the cancel'])
    expect(task.status.value).toBe('canceled')
})

it('hands a result that arrives after its run was abandoned to discard', async () => {
    const { run, runs } = controlledRun()
    const discard = vi.fn()
    const task = createAnalysisTask({ label: 'test', run, schedule: immediate, discard })
    const first = task.start('first')
    await tick()
    const second = task.start('second')
    await tick()
    runs[0].resolve(['late'])
    await expect(first).resolves.toBeUndefined()
    expect(discard).toHaveBeenCalledExactlyOnceWith(['late'], 'first')
    runs[1].resolve(['current'])
    await expect(second).resolves.toEqual(['current'])
    expect(task.result.value).toEqual(['current'])
    expect(discard).toHaveBeenCalledOnce()
})
