import { pathToFileURL } from 'node:url'

export function validateCiResults(jobs) {
    const failures = []
    const requireResult = (name, expected) => {
        if (jobs[name]?.result !== expected)
            failures.push(
                `${name}: expected ${expected}, received ${jobs[name]?.result ?? 'missing'}`,
            )
    }
    requireResult('changes', 'success')
    requireResult('guardrails', 'success')
    const outputs = jobs.changes?.outputs || {}
    for (const name of ['desktop', 'frontend', 'website']) {
        if (!['true', 'false'].includes(outputs[name]))
            failures.push(`Invalid ${name} scope output: ${outputs[name]}`)
    }
    for (const [name, scope] of [
        ['frontend', 'frontend'],
        ['resources', 'desktop'],
        ['desktop', 'desktop'],
        ['website', 'website'],
    ]) {
        requireResult(name, outputs[scope] === 'true' ? 'success' : 'skipped')
    }
    if (failures.length) throw new Error(failures.join('\n'))
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
    try {
        validateCiResults(JSON.parse(process.env.RESULTS))
    } catch (error) {
        console.error(error.message)
        process.exitCode = 1
    }
}
