// The conversation: user requests on the right, the agent's work under each one.
// Turns are keyed, and a running turn is patched in place so streaming text,
// step spinners and expanded groups keep their state between events.
import { includedChanges } from './agent.js'
import { append, button, element, h, hue, icon, renderMarkdown } from './dom.js'
import { axolotl, pixelLoader, setMood } from './mascot.js'
import { dateLabel, durationLabel, text, whenLabel } from './messages.js'
import { animate, reducedMotion } from './motion.js'
import { currentThread, session } from './session.js'
import { ACTIVE_STATUSES } from './state.js'
import { toolHelp } from './tools.js'
import { fileChip } from './view-composer.js'
import { stateLabel } from './view-panel.js'

const root = () => element('messages')
const scroller = () => element('message-scroll')
// A thread opens with its latest turns; older ones are built a batch at a time
// as the reader scrolls up to them, so opening costs the same at any length.
const BATCH = 40
// The next batch is built two screens before the top, so it is ready before
// it would come into view.
const reach = () => scroller().clientHeight * 2

// A turn built above the view is laid out in full until the browser has
// remembered its size; then it may skip layout like any older turn without
// changing height under the reader.
const placing = new ResizeObserver((entries) => {
    const nodes = entries.map((entry) => entry.target)
    for (const node of nodes) placing.unobserve(node)
    requestAnimationFrame(() => {
        for (const node of nodes) node.classList.remove('is-placing')
    })
})

function place(node) {
    node.classList.add('is-placing')
    placing.observe(node)
}

// History that is only being shown, not new, is not read out: the live
// region stays busy while it changes. The browser hands the change to screen
// readers after the next frame's layout, so busy lasts until the frame after.
let quiet = 0
function quietly(change) {
    root().setAttribute('aria-busy', 'true')
    change()
    cancelAnimationFrame(quiet)
    quiet = requestAnimationFrame(() => {
        quiet = requestAnimationFrame(() => root().removeAttribute('aria-busy'))
    })
}

export function renderTranscript(thread) {
    placing.disconnect()
    const nodes = thread.turns.slice(-BATCH).map((turn) => createTurn(thread, turn, false))
    // Laid out once in full, so scrolling up through them never meets a turn
    // that still has the estimated height.
    nodes.forEach(place)
    quietly(() => root().replaceChildren(...nodes))
}

// Patches the thread on screen: gone turns leave, new ones arrive at the
// bottom, and older turns that are not built yet stay that way.
export function patchTranscript(thread, fresh) {
    const ids = new Set(thread.turns.map((turn) => turn.id))
    for (const node of root().querySelectorAll(':scope > [data-turn]'))
        if (!ids.has(node.dataset.turn)) node.remove()
    const from = oldestShown(thread)
    for (const turn of thread.turns.slice(from < 0 ? -BATCH : from))
        syncTurn(thread, turn, turnNode(turn.id), fresh)
}

// Where the built turns start in the thread, or -1 when none are built.
function oldestShown(thread) {
    for (const node of root().children) {
        const index = thread.turns.findIndex((turn) => turn.id === node.dataset.turn)
        if (index >= 0) return index
    }
    return -1
}

// Builds older turns above the built ones: the next batch, or for a jump the
// batches down to `index`. The view keeps showing the same thing: the scroll
// moves down by the height that was added.
function showOlder(thread, index) {
    const from = oldestShown(thread)
    if (from <= 0) return false
    const batches = index === undefined ? 1 : Math.ceil((from - index) / BATCH)
    const nodes = thread.turns
        .slice(Math.max(0, from - batches * BATCH), from)
        .map((turn) => createTurn(thread, turn, false))
    // The reader scrolls into this batch next, so it is laid out now; a jump
    // lays out only what lies above its turn.
    if (index === undefined) nodes.forEach(place)
    quietly(() => keepView(() => root().prepend(...nodes)))
    return true
}

// Changes turns above the view without moving what it shows: the scroll
// follows the first turn in view by however far it moved.
function keepView(change) {
    const view = scroller()
    const anchor = [...root().children].find(
        (node) => node.offsetTop + node.offsetHeight > view.scrollTop,
    )
    const top = anchor?.getBoundingClientRect().top
    change()
    if (anchor) view.scrollTop += anchor.getBoundingClientRect().top - top
}

// Builds older turns down to this one, for the navigator, and returns it.
export function revealTurn(thread, turnId) {
    const index = thread.turns.findIndex((turn) => turn.id === turnId)
    if (index < 0) return null
    if (oldestShown(thread) > index) showOlder(thread, index)
    const node = turnNode(turnId)
    if (!node) return null
    // A jump that ends near the top would build the next batch on its way
    // there and cut its scroll short, so that batch is built first.
    if (node.offsetTop < reach()) showOlder(thread)
    // A turn that skipped layout takes its real height once the scroll nears
    // it (Chromium looks 1.5 windows ahead). Above this turn, and on the way
    // down to it when the jump glides, that would move it while the jump is
    // under way, so those turns take it now.
    const passed = reducedMotion.matches ? node.offsetTop : scroller().scrollTop
    const stop = Math.min(passed, node.offsetTop) - window.innerHeight * 2
    const above = []
    for (
        let turn = node.previousElementSibling;
        turn && turn.offsetTop + turn.offsetHeight > stop;
        turn = turn.previousElementSibling
    )
        above.push(turn)
    keepView(() => above.forEach(place))
    return node
}

// Builds the next batch of older turns while the reader nears the top.
export function bindTranscript() {
    const view = scroller()
    view.addEventListener(
        'scroll',
        () => {
            if (view.scrollTop < reach()) showOlder(currentThread())
        },
        { passive: true },
    )
}

export function turnNode(turnId) {
    return root().querySelector(`[data-turn="${CSS.escape(turnId)}"]`)
}

function createTurn(thread, turn, fresh) {
    const node = h('article', { class: 'turn', data: { turn: turn.id } }, [
        userRow(turn),
        // The live transcript reads out each new request. The agent's side is
        // quiet: streamed text, steps and the ticking timer would be read token
        // by token, so the announcer says once what changed instead.
        h('div', { class: 'agent', 'aria-live': 'off' }, [
            h('div', { class: 'agent-items' }),
            h('div', { class: 'run-status', hidden: true }),
            h('div', { class: 'run-notice', hidden: true }),
            h('div', { class: 'turn-actions' }),
        ]),
    ])
    if (fresh) node.classList.add('is-arriving')
    syncTurn(thread, turn, node, false)
    return node
}

function userRow(turn) {
    const bubble = h('div', { class: 'user-bubble' }, [
        turn.text ? h('p', { text: turn.text }) : null,
    ])
    if (turn.attachments.length)
        bubble.append(
            h(
                'div',
                { class: 'bubble-files' },
                turn.attachments.map((file) => fileChip(file)),
            ),
        )
    return h('div', { class: 'user-row' }, [
        bubble,
        h('div', { class: 'user-meta' }, [
            h('time', { text: dateLabel(turn.createdAt) }),
            button(
                {
                    class: 'icon-button tiny edit-turn',
                    label: text('editMessage'),
                    data: { editTurn: turn.id },
                },
                icon('edit'),
            ),
            button(
                {
                    class: 'icon-button tiny',
                    label: text('copy'),
                    data: { copyTurn: turn.id, copyKind: 'user' },
                },
                icon('copy'),
            ),
        ]),
    ])
}

export function syncTurn(thread, turn, node = turnNode(turn.id), fresh = true) {
    if (!node) {
        // Only a new turn is built here; an older one waits above until it is
        // scrolled to.
        const from = oldestShown(thread)
        if (from >= 0 && thread.turns.indexOf(turn) < from) return null
        node = createTurn(thread, turn, fresh)
        root().append(node)
        return node
    }
    const run = turn.run
    const items = node.querySelector('.agent-items')
    const active = ACTIVE_STATUSES.includes(run.status)
    const lastIndex = run.items.length - 1
    run.items.forEach((item, index) => {
        let child = items.querySelector(`:scope > [data-item="${CSS.escape(item.id)}"]`)
        if (!child) {
            child = createItem(item)
            if (fresh) child.classList.add('is-arriving')
            items.append(child)
        }
        updateItem(child, item, { thread, turn, active, last: index === lastIndex })
    })
    syncStatus(node.querySelector('.run-status'), turn, active)
    syncNotice(node.querySelector('.run-notice'), thread, turn)
    syncActions(
        node.querySelector('.turn-actions'),
        turn,
        active,
        thread.turns.at(-1)?.id === turn.id,
    )
    node.dataset.status = run.status
    return node
}

function createItem(item) {
    const props = { data: { item: item.id } }
    switch (item.type) {
        case 'activity':
            return h('div', { ...props, class: 'activity' }, [
                button(
                    {
                        class: 'activity-summary',
                        'aria-controls': `steps-${item.id}`,
                        data: { action: 'toggle-activity' },
                    },
                    [
                        h('span', { class: 'activity-icon' }),
                        h('span', { class: 'activity-label' }),
                        h('span', { class: 'activity-meta' }),
                        icon('right', 'activity-chevron'),
                    ],
                ),
                h('div', { id: `steps-${item.id}`, class: 'activity-body' }, [
                    h('div', { class: 'activity-clip' }, [h('ol', { class: 'steps' })]),
                ]),
            ])
        case 'text':
            return h('div', { ...props, class: 'agent-text' })
        case 'plan':
            return h('section', { ...props, class: 'plan-card' })
        case 'result':
        case 'error':
            return h('div', {
                ...props,
                class: `result-card ${item.type === 'error' ? 'is-error' : 'is-success'}`,
            })
        default:
            return h('div', props)
    }
}

function updateItem(node, item, view) {
    if (item.type === 'activity') updateActivity(node, item, view)
    else if (item.type === 'text') updateText(node, item, view)
    else if (item.type === 'plan') updatePlan(node, item, view)
    else {
        if (!node.childElementCount) fillResult(node, item, view)
        if (item.type === 'error') syncRetry(node, item, view)
        else syncNext(node, item, view)
    }
}

function stepIcon(status) {
    if (status === 'running') return pixelLoader()
    return icon(status === 'done' ? 'check' : status === 'failed' ? 'close' : 'stop', 'draw')
}

function updateActivity(node, item, { active }) {
    const running =
        item.steps.some((step) => step.status === 'running') || (!item.endedAt && active)
    const failed = item.steps.some((step) => step.status === 'failed')
    const stopped = item.steps.some((step) => step.status === 'stopped')
    const state = running ? 'running' : failed ? 'failed' : stopped ? 'stopped' : 'done'
    if (node.dataset.state !== state) {
        node.dataset.state = state
        const holder = node.querySelector('.activity-icon')
        holder.replaceChildren(
            running
                ? pixelLoader()
                : icon(state === 'done' ? 'check-circle' : state === 'failed' ? 'alert' : 'stop'),
        )
        // Finished, the icon tells how; running, the label already says so.
        if (running) {
            holder.removeAttribute('role')
            holder.removeAttribute('aria-label')
        } else {
            holder.setAttribute('role', 'img')
            holder.setAttribute('aria-label', stateLabel(state))
            holder.firstChild.classList.add('pop')
        }
    }
    // The group stays open while it works and folds away when finished, unless
    // the user opened or closed it themselves. Concise keeps every group folded
    // to the step at hand; detailed keeps them all open.
    const detail = session.store.detail
    const open = node.dataset.user
        ? node.dataset.user === 'open'
        : detail === 'detailed' || (running && detail !== 'concise')
    setOpen(node, open)
    const current = item.steps.findLast((step) => step.status === 'running')
    // While the list is open it already names the current step, so the header
    // counts instead; folded away, the header names the step itself.
    node.querySelector('.activity-label').textContent = running
        ? open
            ? text('stepsRunning', { count: item.steps.length })
            : current?.title || text('working')
        : item.steps.length === 1
          ? item.steps[0].title
          : item.steps.length
            ? text('stepsDoneNamed', { title: item.steps[0].title, count: item.steps.length })
            : text('stepsDone', { count: 0 })
    // Under a second says nothing useful, so short groups show no time.
    const end = item.endedAt || item.steps.at(-1)?.endedAt
    const took = !running && item.startedAt && end ? end - item.startedAt : 0
    node.querySelector('.activity-meta').textContent = took >= 1000 ? durationLabel(took) : ''

    // A folded group builds its rows only once it is opened, which keeps a
    // long history light: finished groups are folded by default.
    node.activityItem = item
    if (open || node.dataset.built) buildSteps(node, item)
}

function buildSteps(node, item) {
    node.dataset.built = 'true'
    const list = node.querySelector('.steps')
    for (const step of item.steps) {
        let row = list.querySelector(`:scope > [data-step="${CSS.escape(step.id)}"]`)
        if (!row) {
            row = h('li', { class: 'step is-arriving', data: { step: step.id } }, [
                h('span', { class: 'step-icon', role: 'img' }),
                h('span', { class: 'step-copy' }, [
                    h('span', { class: 'step-title' }),
                    h('span', { class: 'step-detail' }),
                ]),
                h('code', { class: 'step-tool' }),
                h('span', { class: 'step-help', id: `step-help-${++helpIds}`, hidden: true }),
            ])
            list.append(row)
        }
        if (row.dataset.state !== step.status) {
            const mark = row.querySelector('.step-icon')
            mark.replaceChildren(stepIcon(step.status))
            mark.setAttribute('aria-label', stateLabel(step.status))
            if (row.dataset.state) mark.firstChild.classList.add('pop')
            row.dataset.state = step.status
        }
        row.querySelector('.step-title').textContent = step.title
        const detail = row.querySelector('.step-detail')
        detail.textContent = step.detail
        detail.hidden = !step.detail
        const tool = row.querySelector('.step-tool')
        tool.textContent = step.tool || ''
        tool.hidden = !step.tool
        // The chip shows the tool's real name and its tooltip says what it does.
        // The chip takes no tab stop, which a long run would fill with dozens,
        // so the row's description carries the same words to screen readers.
        const help = row.querySelector('.step-help')
        help.textContent = step.tool ? toolHelp(step.tool) : ''
        if (step.tool) {
            tool.dataset.tip = help.textContent
            row.setAttribute('aria-describedby', help.id)
        } else {
            delete tool.dataset.tip
            row.removeAttribute('aria-describedby')
        }
    }
    // A long run shows its latest steps; the earlier ones fold behind one row.
    const rows = list.querySelectorAll(':scope > .step')
    const folded =
        rows.length > LATEST_STEPS + 2 && !node.dataset.allSteps ? rows.length - LATEST_STEPS : 0
    rows.forEach((row, index) => {
        row.hidden = index < folded
    })
    let more = list.querySelector(':scope > .step-more')
    if (folded && !more) {
        more = h('li', { class: 'step-more' }, [
            button({ class: 'text-button', data: { action: 'show-steps' } }, [
                icon('up'),
                h('span'),
            ]),
        ])
        list.prepend(more)
    } else if (!folded) more?.remove()
    if (more) more.querySelector('span').textContent = text('earlierSteps', { count: folded })
}

const LATEST_STEPS = 6
// Turns are rebuilt when a task opens again, so a counter keeps every row's
// description id unique across them.
let helpIds = 0

export function showAllSteps(target) {
    const node = target.closest('.activity')
    node.dataset.allSteps = 'true'
    for (const row of node.querySelectorAll('.step[hidden]')) row.hidden = false
    target.closest('.step-more').remove()
}

function updateText(node, item, { active, last }) {
    if (node.dataset.length !== String(item.text.length)) {
        renderMarkdown(node, item.text)
        node.dataset.length = String(item.text.length)
    }
    node.classList.toggle('is-streaming', active && last)
}

export const KINDS = {
    update: ['up', 'changeKindUpdate'],
    rollback: ['refresh', 'changeKindRollback'],
    new: ['plus', 'changeKindNew'],
    install: ['download', 'changeKindInstall'],
    setting: ['settings', 'changeKindSetting'],
    backup: ['archive', 'changeKindBackup'],
    remove: ['trash', 'changeKindRemove'],
}

// Lights the part of a version that changes, so 0.6.5 → 0.6.13 reads at a glance.
function versionTo(from, to) {
    const after = to.split(/([.\-+])/)
    const before = (from || '').split(/([.\-+])/)
    let same = 0
    while (from && same < after.length && after[same] === before[same]) same++
    return h('span', { class: 'version is-to' }, [
        same ? after.slice(0, same).join('') : null,
        h('mark', { text: after.slice(same).join('') }),
    ])
}

function changeRow(change, index, turnId) {
    // The glyph only repeats the name's initials.
    const mark = change.glyph
        ? h('span', {
              class: 'change-mark glyph',
              'aria-hidden': 'true',
              text: change.glyph,
              style: `--mark-hue: ${hue(change.name)}`,
          })
        : h('span', { class: 'change-mark' }, [icon(change.icon || 'cube')])
    const kind = !change.manual && KINDS[change.kind]
    const badge = change.manual
        ? h('span', { class: 'change-kind is-manual', text: text('manualChange') })
        : kind
          ? h('span', { class: 'change-kind', data: { kind: change.kind } }, [
                icon(kind[0]),
                h('span', { text: text(kind[1]) }),
            ])
          : null
    // Its arrow is drawn, so the versions are named as one, with the arrow.
    const diff =
        change.manual || !change.to
            ? null
            : h(
                  'span',
                  {
                      class: 'change-diff',
                      role: 'img',
                      'aria-label': change.from ? `${change.from} → ${change.to}` : change.to,
                  },
                  [
                      change.from
                          ? h('span', { class: 'version is-from', text: change.from })
                          : null,
                      change.from ? icon('arrow-right', 'diff-arrow') : null,
                      versionTo(change.from, change.to),
                  ],
              )
    return h('li', { class: `change${change.manual ? ' is-manual' : ''}`, data: { index } }, [
        mark,
        h('span', { class: 'change-copy' }, [
            h('span', { class: 'change-title' }, [
                h('strong', { text: change.name }),
                badge,
                // A newcomer can ask about one change before deciding. The
                // question replaces this plan; the agent answers and proposes
                // it again.
                change.manual
                    ? null
                    : button(
                          {
                              class: 'change-why',
                              data: { followup: text('askWhy', { name: change.name }) },
                          },
                          text('why'),
                      ),
            ]),
            change.note || change.meta
                ? h('span', { class: 'change-note' }, [
                      change.note ? h('span', { text: change.note }) : null,
                      change.meta ? h('span', { class: 'change-meta', text: change.meta }) : null,
                  ])
                : null,
        ]),
        diff || h('span'),
        change.manual
            ? h('span')
            : button(
                  {
                      class: 'change-toggle',
                      role: 'checkbox',
                      'aria-checked': 'true',
                      label: text('includeChange', { name: change.name }),
                      data: { toggleChange: `${turnId}:${index}` },
                  },
                  icon('check'),
              ),
    ])
}

function planFooter(item, turn) {
    const included = includedChanges(item)
    const total = included.length
    const skipped = (item.excluded || []).map((index) => item.plan.changes[index].name)
    const turnId = turn.id
    switch (item.state) {
        case 'pending':
            return h('div', { class: 'plan-actions' }, [
                // Says what approving does, in the plan's own words, above the promise.
                h('div', { class: 'plan-promise' }, [
                    icon('shield'),
                    h('span', {}, [
                        item.plan.approval ? h('strong', { text: item.plan.approval }) : null,
                        skipped.length
                            ? h('span', {
                                  class: 'plan-skipping',
                                  text: text('excludedCount', {
                                      count: skipped.length,
                                      names: skipped.join(' · '),
                                  }),
                              })
                            : h('span', { text: text('approvalPromise') }),
                    ]),
                ]),
                h('div', { class: 'plan-buttons' }, [
                    button({
                        class: 'secondary-button',
                        text: text('decline'),
                        data: { action: 'decline', turn: turnId },
                    }),
                    button(
                        {
                            class: 'primary-button',
                            data: { action: 'approve', turn: turnId },
                            'data-testid': 'approve-plan',
                            disabled: !total,
                            title: total ? null : text('nothingSelected'),
                        },
                        [icon('check'), h('span', { text: text('approve') })],
                    ),
                ]),
            ])
        case 'applying': {
            // One block per change, placed as each change is written.
            const done = Math.min(item.done, total)
            return h('div', { class: 'plan-actions is-progress' }, [
                h('p', { class: 'plan-state' }, [
                    pixelLoader(),
                    text('applyingProgress', { done, total }),
                ]),
                h(
                    'span',
                    { class: 'plan-blocks', 'aria-hidden': 'true' },
                    Array.from({ length: Math.max(total, 1) }, (_, index) =>
                        h('i', { class: index < done ? 'is-placed' : '' }),
                    ),
                ),
            ])
        }
        case 'applied':
            return h('p', { class: 'plan-state is-applied' }, [
                icon('check-circle'),
                skipped.length
                    ? text('planAppliedPartial', { done: total, skipped: skipped.length })
                    : text('planApplied'),
            ])
        case 'declined':
            return h('p', { class: 'plan-state' }, [icon('close'), text('planDeclined')])
        case 'superseded':
            return h('p', { class: 'plan-state' }, [icon('refresh'), text('planSuperseded')])
        case 'stopped':
            return h('p', { class: 'plan-state' }, [icon('stop'), text('planStopped')])
        default:
            return h('p', { class: 'plan-state is-unknown' }, [
                icon('warning'),
                text('planUnknown'),
            ])
    }
}

function updatePlan(node, item, { turn }) {
    const excluded = item.excluded || []
    const signature = `${item.state}:${item.done}:${excluded.join()}`
    if (node.dataset.signature === signature) return
    const first = !node.dataset.signature
    node.dataset.signature = signature
    node.dataset.state = item.state
    if (first) {
        const plan = item.plan
        node.setAttribute('aria-labelledby', `plan-${item.id}`)
        // Reviewing from the approval bar moves the focus here.
        node.tabIndex = -1
        append(node, [
            h('span', { class: 'plan-orbit', 'aria-hidden': 'true' }),
            h('header', { class: 'plan-header' }, [
                h('div', {}, [
                    h('span', { class: 'plan-kicker', text: text('planKicker') }),
                    h('h3', { id: `plan-${item.id}`, text: plan.title }),
                ]),
                plan.summary ? h('span', { class: 'plan-count', text: plan.summary }) : null,
            ]),
            h(
                'ul',
                { class: 'change-list' },
                plan.changes.map((change, index) => changeRow(change, index, turn.id)),
            ),
            plan.notes.length
                ? h(
                      'ul',
                      { class: 'plan-notes' },
                      plan.notes.map((note) =>
                          h('li', {}, [icon('check'), h('span', { text: note })]),
                      ),
                  )
                : null,
            h('div', { class: 'plan-footer' }),
        ])
    }
    // Once the plan is decided its ticks only record the choice: they stay as
    // they look but leave the tab order and read as unavailable.
    const open = item.state === 'pending'
    for (const row of node.querySelectorAll('.change')) {
        const out = excluded.includes(Number(row.dataset.index))
        row.classList.toggle('is-excluded', out)
        const toggle = row.querySelector('.change-toggle')
        if (!toggle) continue
        toggle.setAttribute('aria-checked', String(!out))
        toggle.setAttribute('aria-disabled', String(!open))
        toggle.tabIndex = open ? 0 : -1
    }
    const footer = node.querySelector('.plan-footer')
    footer.replaceChildren(planFooter(item, turn))
    if (!first)
        animate(footer.firstChild, [{ opacity: 0, transform: 'translateY(4px)' }, { opacity: 1 }])
}

// A short burst of accent pixels when a result arrives live, never on reload.
function burst() {
    return h(
        'span',
        { class: 'burst', 'aria-hidden': 'true' },
        Array.from({ length: 10 }, (_, index) => h('i', { style: `--i: ${index}` })),
    )
}

function fillResult(node, item, view) {
    const live = node.classList.contains('is-arriving') && !reducedMotion.matches
    node.append(
        h('span', { class: 'result-mark' }, [
            icon(item.type === 'error' ? 'alert' : 'check-circle', 'result-icon'),
            live && item.type === 'result' ? burst() : null,
        ]),
        h('div', { class: 'result-copy' }, [
            h('strong', { text: item.title }),
            item.text ? h('p', { text: item.text }) : null,
            item.type === 'result' && view.turn.run.preview
                ? h('span', { class: 'preview-note', text: text('previewResult') })
                : null,
        ]),
    )
}

// What to ask next, under the latest result only; a click sends it.
function syncNext(node, item, { thread, turn, active }) {
    const show = !!item.next?.length && !active && thread.turns.at(-1)?.id === turn.id
    let row = node.querySelector('.result-next')
    if (show && !row) {
        row = h(
            'div',
            { class: 'result-next' },
            item.next.map((value) =>
                button({ class: 'next-chip', data: { followup: value } }, [
                    h('span', { text: value }),
                    icon('arrow-right'),
                ]),
            ),
        )
        node.querySelector('.result-copy').append(row)
    } else if (!show) row?.remove()
}

function syncRetry(node, item, { thread, turn, active }) {
    const show = item.retryable && !active && thread.turns.at(-1)?.id === turn.id
    let retry = node.querySelector('[data-action="retry"]')
    if (show && !retry) {
        retry = button({ class: 'secondary-button', data: { action: 'retry', turn: turn.id } }, [
            icon('refresh'),
            h('span', { text: text('retry') }),
        ])
        node.append(retry)
    } else if (!show) retry?.remove()
}

// While the agent works its mascot sits beside what it is doing. When the run
// ends the mascot reacts for a moment before the line goes away.
const EXIT_MOODS = { done: 'happy', awaiting: 'happy', failed: 'sad' }
const EXIT_LABELS = { done: 'stateDone', awaiting: 'stateAwaiting', failed: 'stateFailed' }
const leaving = new WeakMap()

function syncStatus(node, turn, active) {
    if (active) {
        clearTimeout(leaving.get(node))
        delete node.dataset.leaving
        node.hidden = false
        if (!node.childElementCount)
            node.append(
                h('span', { class: 'status-mark' }, [
                    axolotl({ mood: 'think', className: 'axo-sm' }),
                ]),
                h('span', { class: 'shimmer' }),
                h('span', { class: 'elapsed' }),
                h('span', { class: 'status-hint' }, [h('kbd', { text: 'Esc' }), text('escToStop')]),
            )
        // It thinks while reading; once a plan is approved it nods, then paddles
        // away while the changes are written.
        const phase = turn.run.status === 'applying' ? 'applying' : 'thinking'
        if (node.dataset.phase !== phase) {
            node.dataset.phase = phase
            const mark = node.querySelector('.axo')
            if (phase === 'applying') setMood(mark, 'nod', { then: 'work' })
            else setMood(mark, 'think')
        }
        node.querySelector('.shimmer').textContent =
            turn.run.label || text(turn.run.status === 'applying' ? 'applying' : 'thinking')
        updateElapsed(node, turn)
        return
    }
    if (node.hidden || node.dataset.leaving) return
    const mood = EXIT_MOODS[turn.run.status]
    if (!mood || reducedMotion.matches || !node.isConnected) {
        node.hidden = true
        return
    }
    node.dataset.leaving = mood
    delete node.dataset.phase
    setMood(node.querySelector('.axo'), mood)
    node.querySelector('.shimmer').textContent = text(EXIT_LABELS[turn.run.status])
    leaving.set(
        node,
        setTimeout(() => {
            node.hidden = true
            delete node.dataset.leaving
        }, 1400),
    )
}

export function updateElapsed(node, turn) {
    const started = turn.run.phaseAt || turn.run.startedAt
    node.querySelector('.elapsed').textContent = started ? durationLabel(Date.now() - started) : ''
}

const NOTICES = {
    stopped: ['stop', 'noticeStopped', 'retry'],
    interrupted: ['clock', 'noticeInterrupted', 'retry'],
    detached: ['warning', 'noticeDetached', 'check-status'],
}

function syncNotice(node, thread, turn) {
    const notice = NOTICES[turn.run.status]
    const latest = thread.turns.at(-1)?.id === turn.id
    const signature = notice ? `${turn.run.status}:${latest}` : ''
    if (node.dataset.signature === signature) return
    node.dataset.signature = signature
    node.hidden = !notice
    if (!notice) {
        node.replaceChildren()
        return
    }
    const [name, key, action] = notice
    node.dataset.tone = turn.run.status
    // Only the latest turn offers the action. replaceChildren would show a
    // null as the text "null".
    node.replaceChildren(
        ...[
            icon(name),
            h('span', { text: text(key) }),
            action && latest
                ? button({
                      class: 'text-button',
                      text: text(action === 'retry' ? 'retry' : 'checkStatus'),
                      data: { action, turn: turn.id },
                  })
                : null,
        ].filter(Boolean),
    )
}

// Under a finished answer: copy it, mark it in the navigator, and how long it
// took. A marked answer keeps its mark visible.
// The latest answer can be asked for again, unless it wrote changes: running
// it again would drop the record of what was written.
const AGAIN = ['done', 'declined', 'superseded']
const WROTE = ['applied', 'applying', 'unknown']

function syncActions(node, turn, active, latest) {
    const show = !active && turn.run.items.length > 0
    const hasText = turn.run.items.some((item) => item.type === 'text')
    const again =
        latest &&
        AGAIN.includes(turn.run.status) &&
        !turn.run.items.some((item) => item.type === 'plan' && WROTE.includes(item.state))
    const signature = show ? `${hasText}:${!!turn.marked}:${again}` : ''
    if (node.dataset.signature === signature) return
    node.dataset.signature = signature
    if (!show) {
        node.replaceChildren()
        return
    }
    const { startedAt, endedAt } = turn.run
    const took = startedAt && endedAt ? endedAt - startedAt : 0
    node.replaceChildren(
        ...[
            hasText
                ? button(
                      {
                          class: 'icon-button tiny',
                          label: text('copyAnswer'),
                          data: { copyTurn: turn.id, copyKind: 'agent' },
                      },
                      icon('copy'),
                  )
                : null,
            again
                ? button(
                      {
                          class: 'icon-button tiny',
                          label: text('answerAgain'),
                          data: { action: 'retry', turn: turn.id },
                      },
                      icon('refresh'),
                  )
                : null,
            // A toggle keeps one name and says pressed or not; only the
            // tooltip says what a click would do.
            button(
                {
                    class: 'icon-button tiny mark-turn',
                    'aria-label': text('markTurn'),
                    'aria-pressed': String(!!turn.marked),
                    data: {
                        markTurn: turn.id,
                        tip: text(turn.marked ? 'unmarkTurn' : 'markTurn'),
                    },
                },
                icon('bookmark'),
            ),
            took >= 1000
                ? h('span', {
                      class: 'turn-took',
                      title: whenLabel(endedAt),
                      text: text('tookTime', { duration: durationLabel(took) }),
                  })
                : null,
        ].filter(Boolean),
    )
}

export function toggleActivity(summary) {
    const node = summary.closest('.activity')
    const open = !node.hasAttribute('data-open')
    node.dataset.user = open ? 'open' : 'closed'
    setOpen(node, open)
    if (open && !node.dataset.built && node.activityItem) buildSteps(node, node.activityItem)
}

// A folded group only looks closed: its rows stay in the page so it can glide
// open. Inert takes them out of reach of the keyboard and screen readers too.
function setOpen(node, open) {
    node.toggleAttribute('data-open', open)
    node.querySelector('.activity-summary').setAttribute('aria-expanded', String(open))
    const body = node.querySelector('.activity-body')
    if (body.inert === open) body.inert = !open
}
