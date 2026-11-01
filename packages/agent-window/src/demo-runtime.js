// Plays the scripted tasks in demo-flows.js through the runtime contract in
// agent.js. It never calls the launcher: every step and result is simulated, and
// the window marks runs from this runtime as a preview.
import {
    ATTACHMENT_STEP,
    CHECK_STEP,
    FAILURE,
    FLOWS,
    GENERIC,
    PICK_INSTANCE_STEP,
    SAMPLE_INSTANCE,
    SKIPPED,
} from './demo-flows.js'
import { locale } from './messages.js'

const STOPPED = Symbol('stopped')
const LOADERS = {
    fabric: 'Fabric',
    forge: 'Forge',
    neoforge: 'NeoForge',
    quilt: 'Quilt',
    vanilla: 'Vanilla',
}

function language() {
    return locale().toLowerCase().startsWith('zh') ? 'zh' : 'en'
}

function fill(value, vars) {
    if (typeof value === 'string')
        return value.replace(/\{(\w+)\}/g, (match, name) =>
            Object.hasOwn(vars, name) ? String(vars[name]) : match,
        )
    if (Array.isArray(value)) return value.map((entry) => fill(entry, vars))
    if (value && typeof value === 'object')
        return Object.fromEntries(
            Object.entries(value).map(([key, entry]) => [key, fill(entry, vars)]),
        )
    return value
}

// Splits answer text into small pieces so it streams like model output. CJK text
// has no spaces, so it is cut every few characters instead.
function tokens(value) {
    return value.match(/[　-鿿＀-￯]{1,3}|[^\s　-鿿＀-￯]+\s*|\s+/g) || []
}

class Playback {
    constructor(emit, speed) {
        this.emit = emit
        this.speed = speed
        this.stopped = false
        this.timer = null
        this.cancel = null
    }

    wait(milliseconds) {
        if (this.stopped) return Promise.reject(STOPPED)
        return new Promise((resolve, reject) => {
            this.cancel = reject
            this.timer = setTimeout(() => {
                this.timer = null
                this.cancel = null
                resolve()
            }, milliseconds * this.speed)
        })
    }

    send(event) {
        if (!this.stopped) this.emit(event)
    }

    stop() {
        this.stopped = true
        clearTimeout(this.timer)
        this.cancel?.(STOPPED)
    }
}

export class DemoRuntime {
    preview = true

    constructor(options = {}) {
        this.speed = Number(options?.speed) > 0 ? Number(options.speed) : 1
        // Lets the preview server and tests show the failure state on demand.
        this.failures = Number.isInteger(options?.fail) ? options.fail : 0
    }

    #play(emit, script) {
        const playback = new Playback(emit, this.speed)
        script(playback).catch((error) => {
            if (error !== STOPPED) {
                console.error(error)
                playback.send({ type: 'error', title: String(error), text: '', retryable: true })
            }
        })
        return { stop: () => playback.stop() }
    }

    #instance(request, required, lang) {
        const chosen = request.instance
        const fallback = request.instances?.[0]
        const instance = chosen || (required ? fallback || SAMPLE_INSTANCE[lang] : null)
        return {
            picked: !chosen && required,
            vars: instance
                ? {
                      instance: instance.name,
                      version: instance.game_version || '—',
                      loader: LOADERS[instance.loader] || instance.loader || '—',
                      count: request.instances?.length ?? 0,
                  }
                : { count: request.instances?.length ?? 0 },
        }
    }

    async #read(playback, request, index, [tool, title, detail], status = 'done') {
        const id = `${request.turn.id}:read:${index}`
        playback.send({ type: 'step', step: { id, title, detail: '', tool }, status: 'running' })
        await playback.wait(480 + ((index * 173) % 380))
        playback.send({ type: 'step', step: { id, title, detail, tool }, status })
    }

    async #stream(playback, value) {
        const parts = tokens(value)
        for (let index = 0; index < parts.length; index += 2) {
            playback.send({ type: 'text', delta: parts.slice(index, index + 2).join('') })
            await playback.wait(32)
        }
    }

    #progress(playback, steps, states) {
        playback.send({
            type: 'progress',
            steps: steps.map(([title, detail], index) => ({
                title,
                detail,
                status: states[index],
            })),
        })
    }

    start(request, emit) {
        return this.#play(emit, async (playback) => {
            const lang = language()
            const previous = request.history?.at(-1)?.run
            // A pasted crash report says what the task is even without words.
            const crashReport = (request.turn.attachments || []).some(
                (file) => file.kind === 'crash',
            )
            let flow =
                FLOWS.find((entry) => entry.match.test(request.turn.text)) ||
                (crashReport ? FLOWS.find((entry) => entry.id === 'crash') : undefined)
            const followup =
                !flow &&
                ['awaiting', 'superseded', 'declined'].includes(previous?.status) &&
                FLOWS.find((entry) => entry.id === previous?.resume?.flow)
            flow ||= followup || null
            const script = flow ? flow[lang] : GENERIC[lang]
            const { picked, vars } = this.#instance(request, flow?.requiresInstance, lang)
            const copy = fill(script, vars)

            playback.send({ type: 'status', label: copy.working })
            if (copy.title && !followup) playback.send({ type: 'title', title: copy.title })
            if (copy.progress)
                this.#progress(playback, copy.progress, ['current', 'pending', 'pending'])
            await playback.wait(650)

            let index = 0
            for (const file of request.turn.attachments || [])
                await this.#read(playback, request, index++, [
                    null,
                    ...fill(ATTACHMENT_STEP[lang], file),
                ])
            if (picked)
                await this.#read(playback, request, index++, fill(PICK_INSTANCE_STEP[lang], vars))
            const reads = followup
                ? copy.reads.slice(-1)
                : flow
                  ? copy.reads
                  : request.instance
                    ? copy.readsWithInstance
                    : copy.reads
            for (const [position, read] of reads.entries()) {
                if (flow && this.failures > 0 && position === reads.length - 1) {
                    this.failures--
                    const failure = FAILURE[lang]
                    await this.#read(
                        playback,
                        request,
                        index++,
                        [read[0], read[1], failure.step],
                        'failed',
                    )
                    playback.send({
                        type: 'error',
                        title: failure.title,
                        text: failure.text,
                        retryable: true,
                    })
                    return
                }
                await this.#read(playback, request, index++, read)
                if (copy.progress && position === Math.floor(reads.length / 2))
                    this.#progress(playback, copy.progress, ['done', 'current', 'pending'])
            }
            await playback.wait(300)
            await this.#stream(playback, followup ? copy.followup : copy.answer)
            if (!flow) {
                playback.send({ type: 'done' })
                return
            }
            await playback.wait(350)
            this.#progress(playback, copy.progress, ['done', 'done', 'pending'])
            playback.send({ type: 'resume', data: { flow: flow.id, vars } })
            playback.send({ type: 'plan', plan: copy.plan })
            playback.send({ type: 'done' })
        })
    }

    approve(request, emit) {
        return this.#play(emit, async (playback) => {
            const lang = language()
            const flow = FLOWS.find((entry) => entry.id === request.resume?.flow)
            if (!flow) throw new Error('Unknown preview task')
            const copy = fill(flow[lang], request.resume.vars || {})
            this.#progress(playback, copy.progress, ['done', 'done', 'current'])
            playback.send({ type: 'status', label: copy.progress[2][0] })
            // Changes the user took out are skipped; the count never passes what is left.
            const skipped = request.excluded || []
            const included =
                copy.plan.changes.filter((change) => !change.manual).length - skipped.length
            let done = 0
            for (const [index, [tool, title, detail, rows]] of copy.writes.entries()) {
                const id = `${request.turn.id}:write:${index}`
                playback.send({
                    type: 'step',
                    step: { id, title, detail: '', tool },
                    status: 'running',
                })
                await playback.wait(820 + ((index * 211) % 420))
                playback.send({ type: 'step', step: { id, title, detail, tool }, status: 'done' })
                if (rows) {
                    done = Math.min(done + rows, included)
                    playback.send({ type: 'applied', done })
                }
            }
            await playback.wait(250)
            this.#progress(playback, copy.progress, ['done', 'done', 'done'])
            const note = skipped.length
                ? ` ${fill(SKIPPED[lang], { names: skipped.join(lang === 'zh' ? '、' : ', ') })}`
                : ''
            playback.send({ type: 'result', ...copy.result, text: copy.result.text + note })
            playback.send({ type: 'done' })
        })
    }

    check(request, emit) {
        return this.#play(emit, async (playback) => {
            const lang = language()
            const flow = FLOWS.find((entry) => entry.id === request.resume?.flow)
            const copy = flow ? fill(flow[lang], request.resume.vars || {}) : null
            const [tool, title, detail, heading] = CHECK_STEP[lang]
            playback.send({ type: 'status', label: title })
            await this.#read(playback, request, 99, [tool, title, detail])
            if (copy) {
                const total = copy.writes.reduce((sum, write) => sum + (write[3] || 0), 0)
                playback.send({ type: 'applied', done: total, complete: true })
                this.#progress(playback, copy.progress, ['done', 'done', 'done'])
            }
            playback.send({ type: 'result', title: heading, text: copy?.check || detail })
            playback.send({ type: 'done' })
        })
    }
}
