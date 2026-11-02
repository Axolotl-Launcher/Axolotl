import { expect, it } from 'vitest'

import { runAgent } from '../../../../../packages/agent-runtime/src/run.ts'
import { FixtureHost, job } from '../../../../../packages/agent-runtime/tests/fixtures.ts'

it('browser internal checklists update progress without calling a business write tool', async () => {
    const host = new FixtureHost()
    host.respond = (_, index) =>
        index === 1
            ? {
                  role: 'assistant',
                  content: '',
                  tool_calls: [
                      {
                          id: 'todo',
                          name: 'write_todos',
                          args: { todos: [{ content: 'Inspect launcher', status: 'completed' }] },
                      },
                  ],
              }
            : { role: 'assistant', content: 'Checklist recorded.' }
    expect(
        (await runAgent(job({ attempt_id: 'browser-todos' }), host, new AbortController().signal))
            .status,
    ).toBe('completed')
    expect(host.calls).toEqual([])
    expect(host.events).toContainEqual({
        type: 'progress',
        steps: [{ title: 'Inspect launcher', detail: '', status: 'done' }],
    })
})

it('browser approval checkpoints keep concurrent attempts isolated and resume only selected calls', async () => {
    const hosts = [new FixtureHost(), new FixtureHost()]
    for (const [index, host] of hosts.entries())
        host.respond = () => ({
            role: 'assistant',
            content: '',
            tool_calls: [
                {
                    id: `write-${index}`,
                    name: 'instances_rename',
                    args: { name: `Target ${index}` },
                },
            ],
        })
    const jobs = hosts.map((_, index) => job({ attempt_id: `browser-attempt-${index}` }))
    const paused = await Promise.all(
        hosts.map((host, index) => runAgent(jobs[index], host, new AbortController().signal)),
    )
    expect(paused.map((result) => result.status)).toEqual(['awaiting', 'awaiting'])
    expect(hosts.flatMap((host) => host.calls)).toEqual([])
    for (const host of hosts)
        host.respond = () => ({ role: 'assistant', content: 'Decision respected.' })
    const resumed = await Promise.all(
        hosts.map((host, index) => {
            if (index === 0) host.allowed.add('write-0')
            return runAgent(
                {
                    ...jobs[index],
                    resume: {
                        call_ids: [`write-${index}`],
                        decisions: [
                            index === 0
                                ? { type: 'approve' }
                                : { type: 'reject', message: 'User rejected this action.' },
                        ],
                    },
                },
                host,
                new AbortController().signal,
            )
        }),
    )
    expect(resumed.map((result) => result.status)).toEqual(['completed', 'completed'])
    expect(hosts[0].calls.map((call) => call.call_id)).toEqual(['write-0'])
    expect(hosts[1].calls).toEqual([])
})
