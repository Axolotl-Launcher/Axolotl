# Agent Window

## What it is

The Agent Window is the launcher's AI agent panel for people who have never used an agent. It is a
separate, frameless Tauri webview (label `agent-window`) that the main window opens from its
toolbar button. The user describes a task in their own words. The agent reads with launcher tools
and proposes a plan of changes. Nothing is written until the user approves that plan, and they can
stop the agent at any time.

The crate embeds plain ES modules (there is no bundler) and serves them over the `axolotl-agent://`
protocol from the `ASSETS` table in `src/lib.rs`. A strict CSP applies, navigation is locked to the
entry document, and the window cannot call launcher commands. The real agent backend is not
connected yet. Until it is, a scripted runtime plays four sample tasks and the window labels itself
**Preview**. This README is for the people who will connect that backend and for reviewers of
PR #1039.

## Layout of the package

Rust:

- `Cargo.toml`: the `agent-window` crate (serde, tauri, tokio).
- `build.rs`: embeds the Common Controls v6 manifest so the lib tests can start on Windows MSVC.
- `src/lib.rs`: `AgentWindow` (appearance, permissions, open/close lifecycle), the `ASSETS` table,
  the CSP and navigation rules, and the Rust tests.

Window (`src/`):

- `index.html`: the entry document: static markup, the inline SVG icon sprite (`#i-<name>`) and
  module preloads.
- `window.js`: a classic script that runs first. It draws the window chrome (minimize, maximize,
  close, drag), turns the appearance into CSS variables and builds the host bridge
  `window.AgentHost`.
- `app.js`: the controller. It loads and saves the store, builds requests, applies runtime events,
  and owns the actions, keyboard shortcuts and boot.
- `agent.js`: the runtime contract, the event reducer `applyEvent`, the plan helpers (`beginApply`,
  `stopRun`, `declinePlan`, `supersedePlan`, `toggleChange`, `includedChanges`) and
  `createRuntime`.
- `state.js`: the v2 store schema, sanitising, limits, the budgeted save and the v1 migration.
- `session.js`: live state that is never saved and that the controller and views share: store,
  context, runtime, permissions and run handles.
- `demo-runtime.js`: `DemoRuntime`, the scripted Preview runtime.
- `demo-flows.js`: the four scripted tasks in English and Chinese, plus the generic answer.
- `messages.js`: `text()` lookup with `{name}` filling, the locale and cached `Intl` formatters.
- `tools.js`: `toolHelp()`, the plain-language sentence for a tool name: the tool's own, else its
  family's (`instance_`, `resource_` and so on), else "A launcher tool".
- `dom.js`: the `h()` element builder (text nodes only), icons, the Markdown and log renderers and
  the focus helpers.
- `motion.js`: `morph()` (View Transitions), `animate()`, `ScrollFollower` and `reducedMotion`.
- `mascot.js`, `mascot.css`: the axolotl's moods, gaze and cross-dissolve, and the pixel loader.
- `view-home.js`: the new task page: the hero axolotl, suggestions, the intro card, the first-run
  tour and the help dialog.
- `view-composer.js`: the composer, attachment chips, the follow-up queue, the approval bar and the
  changes pill.
- `view-transcript.js`: turns, step groups, Markdown answers, plan cards, results and notices,
  with history built lazily.
- `view-sidebar.js`: the task list: pinned tasks first, day groups, search, status dots and
  progress.
- `view-panel.js`: the Progress panel (checklist, changes, context and tool calls) and the progress
  ring.
- `view-rail.js`: the message navigator beside the transcript, with marks.
- `view-palette.js`: the command menu (Ctrl K).
- `view-slash.js`: `/` shortcuts and `@` instance mentions in the composer.
- `view-overlays.js`: pickers, menus, dialogs, settings, toasts, tooltips, the file and changes
  previews, and the screen reader announcer.
- `window.css`: every style except the mascot's.
- `fonts/`: Inter 400, 500 and 600, subset to Latin, with its licence.
- `mascot/`: sprite strips rendered by `dev/render-mascot.py`.

Development (`dev/`):

- `serve.mjs`: a browser preview server with a stand-in host.
- `render-mascot.py`: renders the mascot strips.

Launcher side, outside this package:

- `apps/app/src/api/agent_window.rs`: the `agent-window` plugin's commands and the guard that
  rejects every app-level command from this window.
- `apps/app/build.rs` and `apps/app/capabilities/agent-window.json`: command list and permissions.
- `apps/app-frontend/src/features/agent-window/AgentWindowButton.vue`: opens the window and keeps
  its appearance current.
- `apps/app-frontend/src/features/agent-window/messages.ts`: every string the window shows.

A new `.js`, `.css` or `.woff2` file must be added to `ASSETS`, or a Rust test fails. Add a
`<link rel="modulepreload">` for a new module as well. PNGs are not covered by that test.

## Host bridge (`window.AgentHost`)

`window.js` builds `window.AgentHost`, or extends one that was injected before it (the preview
server and the tests do this). Inside Tauri every call goes through
`plugin:agent-window|<command>`.

| Member                   | Source                                          | Purpose                                                              |
| ------------------------ | ----------------------------------------------- | -------------------------------------------------------------------- |
| `appearance`             | `agent_window_get_appearance`                   | Colours, labels, locale and the message catalog.                     |
| `ready`                  | `window.js`                                     | Resolves after the first appearance; `app.js` boots after it.        |
| `getContext()`           | `agent_window_get_context`                      | Providers, models, instances and flags.                              |
| `getPermissions()`       | `agent_window_get_permissions`                  | The current policy. Native only.                                     |
| `setPermissions(policy)` | `agent_window_set_permissions`                  | Saves and returns the policy. Native only.                           |
| `reveal()`               | `agent_window_action` with `{ action: 'show' }` | Shows and focuses the window. `app.js` tries three times.            |
| `runtime`                | not provided yet                                | The native agent runtime. See [Runtime contract](#runtime-contract). |
| `demo`                   | preview server only                             | `{ speed, fail }` options for `DemoRuntime`.                         |

Rust dispatches `agent-appearance-changed` from `reveal()` and `update_appearance()`. `window.js`
then fetches the appearance again and fires `agent-appearance-updated`, and the app re-renders and
refreshes the context. `agent-host-ready` fires once, after the first appearance.

### Appearance and the message catalog

The launcher sends an `Appearance` with `open_agent_window` and `update_agent_window_appearance`.
`AgentWindowButton.vue` sends it again when the class or style of `<html>` or the locale changes.

```ts
type Appearance = {
  locale: string // 'en-US'
  // Resolved CSS colours, as rgb() or hex.
  background: string
  surface: string
  text: string
  accent: string
  scrollbar: string
  scrollbar_track: string
  scrollbar_radius: string // '16px'
  labels: { title: string; minimize: string; maximize: string; restore: string; close: string }
  messages: Record<string, string> // formatAgentWindowMessages(formatMessage)
}
```

Each colour that passes `CSS.supports` becomes an `--agent-*` variable, and `--agent-on-accent` is
chosen from the accent's luminance. `messages` must be a non-empty object. Without it the window
hides its workspace and shows a fixed English error that asks the user to reopen the window from
the launcher.

### Context

`getContext()` must resolve to the shape below. `normalizeContext` in `app.js` fills in missing
lists, and only a strict `true` counts for either flag.

```ts
type Context = {
  ai_enabled: boolean
  providers: { provider_id: string; name: string; models: { id: string; name: string }[] }[]
  instances: { id: string; name: string; game_version: string; loader: string }[]
  execution_available: boolean
}
```

- The Rust command lists only enabled providers that have at least one enabled model, sorts
  instances by name, and returns `execution_available: false` for now. It fails with "Launcher is
  still initializing" until theseus is up.
- The window asks again on boot, when it gains focus or becomes visible, after an appearance
  update, when the model or instance picker opens and from the banner's Retry. A request that takes
  more than 15 s counts as failed and shows the banner.
- A new task starts with `lastSelection`, or with the first model on offer. A model or instance
  that has left the context shows as "Model unavailable" or "Instance unavailable".
- Outside Preview, sending needs a model. When `ai_enabled` is false or no model is enabled, the
  model button reads "Set up a model" and explains where to turn AI on.

### Permissions

`getPermissions()` and `setPermissions({ full_access, computer_use })` both resolve to the stored
policy.

- The policy lives in Rust (`AgentWindow::permissions()`) for the launcher process only. A restart
  resets both flags to `false`.
- `computer_use` is forced off unless `full_access` is on, in Rust and in the window.
- Turning full access on asks for confirmation first. While a save is in flight, the settings
  dialog cannot be closed.
- The window does not send the policy with requests. The backend reads `AgentWindow::permissions()`
  itself. Neither flag skips plan approval.
- If the host lacks either function, the switches are disabled.

## Runtime contract

### Methods

```js
runtime.start(request, emit) // a new turn, or the same turn again
runtime.approve(request, emit) // write the plan the user approved
runtime.check(request, emit) // read back the outcome of writes that were cut off
// Each returns a handle { stop() } synchronously.
runtime.preview // true only for the scripted runtime; anything else counts as native
```

A method that throws becomes an error event, "The agent couldn't start", which can be retried. The
handle must come back synchronously: the window does not await a promise, and it does not catch a
rejected one. Several tasks can run at once, one run per task; a message sent while its task is
running waits in that task's queue.

### Request

Every call receives these fields:

| Field          | Value                                                                                                                                    |
| -------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| `thread`       | `{ id, title }` of the task.                                                                                                             |
| `turn`         | `{ id, text, attachments }`. `text` can be empty when an attachment carries the request.                                                 |
| `scope`        | `'launcher'` for the entire launcher, `'instance'` when `instance` is set, or `'missing'` when the chosen instance has left the context. |
| `instance`     | The context entry `{ id, name, game_version, loader }`, or `null` unless `scope` is `'instance'`.                                        |
| `instances`    | The context's full instance list.                                                                                                        |
| `model`        | `{ provider, model }`, both context entries, or `null`. Only Preview can send without a model.                                           |
| `history`      | Deep copies of the task's earlier turns, runs included (status, items, plans, `resume`). Pasted text makes these large.                  |
| `resume`       | This turn's saved `run.resume`, or `null`.                                                                                               |
| `instructions` | The standing instructions from Settings, up to 2,000 characters. Can be empty.                                                           |

`approve` also receives `planId` (the id of the plan being approved) and `excluded` (the **names**
of the changes the user took out).

A `'missing'` instance shows as "Instance unavailable" in the composer. The runtime should say that
the chosen instance is gone rather than treat the request as one for the entire launcher; the
Preview says so before it borrows the first instance for a task that needs one.

Declining or replacing a plan does not call the runtime. The next request sees it in `history`, as
a run with status `declined` or `superseded`.

### Events

`emit(event)` takes these events, in whatever order a run needs:

| Event      | Fields                                        | Effect                                                                                                                                                                                                                                                         |
| ---------- | --------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `status`   | `label`                                       | Text of the status line beside the axolotl. The first event moves the run from `thinking` to `running`.                                                                                                                                                        |
| `title`    | `title`                                       | A better task title, up to 120 characters. Ignored once the user has renamed the task. Until then the title is the first 48 characters of the first request.                                                                                                   |
| `step`     | `step: { id, title, detail, tool }`, `status` | One tool call. `status` is `running`, `done` (the default) or `failed`. Steps in a row form one step group; text, a plan, a result or an error closes the group. Sending an id again updates that step, so keep ids unique within a run. `tool` shows as code. |
| `text`     | `delta`                                       | Streamed answer text, appended to the current block. Rendered as Markdown: headings, paragraphs, lists, quotes, rules, fenced code, inline code, bold, italic and links. Links are copied, never opened.                                                       |
| `progress` | `steps: [{ title, detail, status }]`          | Replaces the task checklist. `status` is `pending` (the default), `current`, `done` or `failed`. Twelve are kept. Shown in the Progress panel, the ring and the task list.                                                                                     |
| `plan`     | `plan`                                        | Changes that need approval. See [Plans and approval](#plans-and-approval). The run waits with status `awaiting`.                                                                                                                                               |
| `applied`  | `done`, `complete`                            | How many changes of the latest plan are written. `complete: true` marks the plan applied, which `check` needs.                                                                                                                                                 |
| `result`   | `title`, `text`, `next`                       | The outcome card. `next` is up to three follow-up requests, shown as chips under the latest result; a click sends one as a new request.                                                                                                                        |
| `error`    | `title`, `text`, `retryable`                  | The run failed and its running steps are marked failed. `retryable` defaults to `true` and offers Retry on the latest turn.                                                                                                                                    |
| `resume`   | `data`                                        | Opaque data kept on the run and sent back as `request.resume` to `approve` and `check` for this turn. Use a plain JSON object: anything else is dropped on reload.                                                                                             |
| `done`     | none                                          | The call is over. After a plan the run keeps waiting; while applying, the plan becomes applied and the run done; otherwise the run is done.                                                                                                                    |

- End every call with `done` or `error`. That releases the handle, saves at once, marks a task that
  finished in the background as unread with a toast, and sends the next queued follow-up. Other
  events are saved about 400 ms later.
- Follow a `plan` with `done`. An `error` after it fails the run and leaves the plan "Not carried
  out", with no Approve button (`?fail=plan` on the preview server shows this).
- Unknown event types are ignored. If the task was deleted, the window stops the handle.
- Every step, plan, result and error field is plain text. The window never parses HTML.
- A step's `tool` is shown by its real name, with a sentence from `tools.js` that says what it does
  in plain words: in its tooltip, in the step's accessible description and in the Progress panel.
  A tool without a sentence of its own is described by its family, else as "A launcher tool".

### Attachments

`request.turn.attachments` is a list of `{ id, name, size, kind, text, lines }`, where only `id`,
`name` and `size` are always present.

- A file that is picked, dropped or pasted carries only its `name` and `size` in bytes. The window
  has neither its path nor, in general, its contents.
- A text file (`.log`, `.txt`, `.json`, `.toml`, `.cfg` or `.properties`, up to 2 MiB) is read in
  the background and gains `kind`, `text` and `lines`. Sent before the read finishes, it goes
  without them.
- A paste of 16 lines or more, or of 2,500 characters or more, becomes an attachment named
  "Pasted text", "Crash report" or "Game log". A toast offers to paste it as text instead.
- `kind` is `crash` for a Minecraft crash report or a JVM fatal error, `log` for Minecraft log lines
  such as `[12:00:00] [Render thread/ERROR]`, and `paste` otherwise.
- `text` is cut to 200,000 characters. `lines` counts the lines of the whole original, and `size`
  is the original's size in bytes.
- A message holds at most 10 attachments.

### Stop, detach and check

- **Stop** (Esc, or the composer's stop button) calls `handle.stop()` and then
  `stopRun()`. The window does not filter late events, so a runtime must not emit after `stop()`,
  and `stop()` should cancel the backend work.
  - While reading (`thinking` or `running`), the run becomes `stopped`. Nothing changed, and the
    latest turn offers Retry.
  - While writing (`applying`), the run becomes `detached` and its plan `unknown`. The notice says
    some changes may already be written and offers Check status, not Retry.
- **Closing or reloading the window** resumes nothing. On load, a reading run becomes `interrupted`
  (with Retry), a writing run becomes `detached` and a running step `stopped`. A plan that was
  waiting for approval comes back and can still be approved.
- **Check status** works on the latest turn when it is detached. The run goes back to `running`
  and `runtime.check` gets the same request, `resume` included. Send `applied` with
  `complete: true` if the writes finished, then a `result` and `done`. Without `complete` the plan
  stays `unknown`. Keep a record of each write that `check` can find from `resume`.
- **Retry** and **Answer again** call `start` for the same turn with a fresh run, so `resume` is
  `null`. Answer again is offered on the latest turn when it ended done, declined or superseded and
  wrote nothing.

### Leaving Preview

The window uses the native runtime only when all of these hold:

1. `window.AgentHost.runtime` is set. The natural place is the native branch of `window.js`, next
   to `getContext`.
2. `getContext()` returns `execution_available: true`. `agent_window_get_context` returns `false`
   for now.
3. No Preview run is in flight. The window switches at its next context refresh.

Only `preview: true` marks a runtime as the Preview. `ensureRuntime()` treats any other value,
`false` or none at all, as native, so a native runtime still hands back to Preview when
`execution_available` later turns false. Setting `preview: false` says so plainly.

Once Preview is off, the badge and the composer note go, new runs are not marked `preview`,
results lose "Simulated in preview", and sending needs a model.

The window can only call commands of the `agent-window` plugin. Each new command needs an entry in
`generate_handler!` in `apps/app/src/api/agent_window.rs`, in the plugin's command list in
`apps/app/build.rs`, in `apps/app/capabilities/agent-window.json` and in the parity test
`apps/app/tests/instance_command_manifest_parity.rs`. The CSP only allows `connect-src ipc:` and
`http://ipc.localhost`, so events have to come back over Tauri IPC.

### A minimal runtime

```js
// `backend` stands for whatever the launcher exposes to this window.
function play(emit, script) {
  let stopped = false
  const send = (event) => {
    if (!stopped) emit(event)
  }
  script(send).catch((error) => {
    send({ type: 'error', title: 'Something went wrong', text: String(error), retryable: true })
  })
  return {
    stop() {
      stopped = true
      // Cancel the backend work here as well.
    },
  }
}

window.AgentHost.runtime = {
  preview: false,
  start: (request, emit) =>
    play(emit, async (send) => {
      send({ type: 'status', label: 'Reading the instance…' })
      const step = { id: `${request.turn.id}:read`, title: 'Read the instance', tool: 'instance_get' }
      send({ type: 'step', step, status: 'running' })
      const found = await backend.inspect(request)
      send({ type: 'step', step: { ...step, detail: found.summary }, status: 'done' })
      send({ type: 'text', delta: 'Sodium has a compatible update.' })
      send({ type: 'resume', data: { planRef: found.planRef } })
      send({
        type: 'plan',
        plan: {
          title: 'Performance',
          approval: 'Update 1 mod',
          changes: [{ name: 'Sodium', kind: 'update', from: '0.6.5', to: '0.6.13' }],
        },
      })
      send({ type: 'done' })
    }),
  approve: (request, emit) =>
    play(emit, async (send) => {
      send({ type: 'status', label: 'Updating Sodium…' })
      await backend.apply(request.resume.planRef, request.excluded)
      send({ type: 'applied', done: 1 })
      send({ type: 'result', title: 'Updated', text: 'Sodium is on 0.6.13.', next: ['Undo this update'] })
      send({ type: 'done' })
    }),
  check: (request, emit) =>
    play(emit, async (send) => {
      const outcome = await backend.outcome(request.resume?.planRef)
      if (outcome.complete) send({ type: 'applied', done: outcome.done, complete: true })
      send({ type: 'result', title: 'Status checked', text: outcome.summary })
      send({ type: 'done' })
    }),
}
```

## Plans and approval

```js
const plan = {
  id: 'p-1', // optional; one is made up if missing. Sent back as `planId`.
  title: 'Morning Mist · Performance', // the card's heading
  summary: '2 changes · 1 for you', // a short count beside it
  approval: 'Create a snapshot, then update 2 mods', // what approving does, in one line
  changes: [
    {
      name: 'Sodium',
      note: 'Faster rendering',
      meta: 'Modrinth · released 6 days ago',
      kind: 'update',
      glyph: 'S',
      from: '0.6.5',
      to: '0.6.13',
    },
    { name: 'Render distance', note: 'In-game video setting', icon: 'monitor', manual: true },
  ],
  notes: ['A snapshot is created first'], // reassurances under the list
  scope: [{ label: 'Update mods', count: 2 }], // counts shown in the Progress panel
}
```

- `title` and `changes` are required; the rest is optional.
- A change has a `name` and may have a `note`, a `meta` line, `from` and `to` versions (the part
  of `to` that differs is highlighted, and `to` alone shows only the target) and a `kind`: `update`,
  `rollback`, `new`, `install`, `setting`, `backup` or `remove`, each with its badge.
- Its mark is a `glyph` of up to three letters, coloured from a hash of the name, or an `icon` from
  the sprite in `index.html` (`cube` by default).
- `manual: true` marks advice the agent cannot carry out, such as an in-game setting. It shows as
  "Do it yourself", has no toggle and no "Why?", and is never part of what approving writes.
- Each other change has a tick that takes it out of the plan or puts it back (`excluded` holds
  their indexes), and a "Why?" button that sends "Why change {name}?" as a follow-up.
- `approval` is shown above the buttons, and in the approval bar above the composer while the
  card's buttons are scrolled out of view. Without it, the bar shows the title.
- Only the fields above survive a reload. Keep anything else the backend needs in `resume`.

Approving calls `beginApply()`. The plan and the run become `applying`, and `runtime.approve`
receives `planId` and the names of the excluded changes. `applied.done` drives "Applying changes ·
done/total", where the total counts only the included changes.

The window enforces these rules:

- Only the Approve button, on the card or in the approval bar, starts writing. There is no
  auto-approve, whatever the permissions say.
- `beginApply()` accepts only the last plan of the run, only while it is pending and only with at
  least one included change. Approve is disabled while every change is taken out ("Keep at least
  one change").
- A message sent while a plan waits supersedes it ("Replaced by your newer message"). The runtime
  should propose the plan again in its answer, as the Preview does from
  `history.at(-1).run.resume`. Declining does not call the runtime.
- Once decided, a plan's ticks stay as they were and cannot be changed.
- Stopping while writing never claims that nothing changed; the outcome stays unknown until checked.
- Answer again is never offered for a turn whose plan wrote anything (applied, applying or
  unknown), so the record of what was written is never replaced.
- Applied changes are listed behind the changes pill, whose "Ask to undo them" sends an ordinary
  request; undoing goes through a plan like any other change.

## Persistence

History stays on the device, in the window origin's `localStorage`, under
`axolotl.agent-window.store.v2`. Everything read back is rebuilt field by field (`parseStore`):
wrong types fall back to defaults, duplicate ids are dropped, strings are cut to their limits and
unknown states become `interrupted`, `unknown` or `done`.

Store:

| Field                      | Meaning                                                                                                   |
| -------------------------- | --------------------------------------------------------------------------------------------------------- |
| `version`                  | `2`.                                                                                                      |
| `activeId`                 | The open task; the first task if it is missing.                                                           |
| `threads`                  | The tasks.                                                                                                |
| `lastSelection`            | `{ provider_id, model_id }` used last. New tasks start with it.                                           |
| `sidebarOpen`, `panelOpen` | The docked task list (open by default) and Progress panel (closed by default) on wide windows.            |
| `introDismissed`           | Hides the "How it works" card. Set by "Got it" and by the first applied plan; Settings can show it again. |
| `tourDone`                 | The first-run tour has ended. It is only offered while the intro card has not been dismissed.             |
| `detail`                   | `concise`, `balanced` (the default) or `detailed`: how step groups fold.                                  |
| `instructions`             | Standing instructions, up to 2,000 characters, sent with every request.                                   |

The drawers that replace the panes below 760 px (task list) and 1,100 px (Progress) are not saved.

Task (`threads[]`):

- `id`, `title` (120), `titleEdited`, `createdAt`, `updatedAt`.
- `pinned` and `unread`, stored only when `true`. `unread` marks a run that settled in the
  background, and clears when the task is opened.
- `draft` (50,000), `attachments` (the draft's, 10), `instanceId`, `selection`.
- `turns`, and `queue`: follow-ups typed while the agent works (20, each
  `{ id, text, attachments, createdAt }`).

Turn: `id`, `createdAt`, `text` (50,000), `attachments` (10), `instanceId` and `selection` (copied
from the task when sent), `run`, and `marked` (stored only when `true`; flagged in the navigator).

Run:

- `status`: `thinking`, `running` or `applying` (active), `awaiting`, `done`, `failed`, `stopped`,
  `detached`, `declined`, `superseded` or `interrupted`.
- `startedAt`, `endedAt`, `label` (emptied on load), `resume`, and `preview` (made by the scripted
  runtime). `phaseAt`, which starts the timer again on approval or check, is not kept.
- `progress`: up to 12 steps, each with `title` (200), `detail` (300) and `status`.
- `items`, up to 200:
  - `activity`: up to 100 steps, each with `id`, `title` (500), `detail` (2,000), `tool` (120),
    `status` (`running`, `done`, `failed` or `stopped`) and times.
  - `text`: `text` (50,000).
  - `plan`: `plan`, `state` (`pending`, `applying`, `applied`, `declined`, `superseded`, `stopped`
    or `unknown`), `done` and `excluded`. The plan keeps `id`, `title` (200), `summary` (200),
    `approval` (400), up to 50 `changes`, 6 `notes` (200 each) and 8 `scope` entries.
  - `result`: `title` (200), `text` (2,000), `retryable` and up to 3 `next` entries (120 each).
  - `error`: `title`, `text` and `retryable`.

Attachments keep `id`, `name` (260) and `size`. `kind`, `text` (200,000) and `lines` are kept only
for `paste`, `crash` and `log`.

The limits apply when history is read back, not while a run streams, so a runtime should stay
within them.

**Saving.** User actions (send, approve, decline, rename, delete, pin, picking a model or an
instance) are all-or-nothing: they change a copy, save it and only then take effect. If the save
fails, nothing changes, the draft stays in the composer and a banner offers Retry and Export draft.
Runtime events, typing and layout changes update the live store and save within 400 ms (250 ms
while typing), and at once on `done`, `error`, when the window is hidden and on `pagehide`.

**When space runs out.** The budget is 100 tasks, 600 turns in all and 1,500,000 characters of
JSON. `saveStore` makes room in this order:

1. Whole tasks beyond 100, least recently updated first. It never drops the open task or a task
   with a draft, a queued follow-up, or a run that is active or awaiting approval.
2. The oldest settled turns across all tasks, the open one included, until the store fits.
3. If it still does not fit, it throws "Conversation storage limit reached" and leaves the stored
   copy untouched.

**Unreadable history.** If the stored JSON cannot be parsed or has another version, the window
starts from a fresh store in memory and saves nothing. A banner offers Export history (the raw
stored text), Start new history (which keeps the draft, the intro and tour flags and the task list
state) and Export draft.

**Migration from v1.** When the v2 key is empty, `axolotl.agent-window.conversations.v1` is read.
Its `threads[].messages`, requests from the first preview that never ran, become turns with an
`interrupted` run and no items. The result goes through the same rebuilding and is saved under the
v2 key on the next save. The v1 key is left in place.

## Strings

Strings come from the launcher's catalog, so the window has no translations of its own. The
exceptions are the catalog error, the `/` shortcut names (English in every language on purpose)
and the scripted Preview tasks in `demo-flows.js`, which pick English or Chinese from the locale.

To add a message:

1. Add an entry to `agentWindowMessages` in
   `apps/app-frontend/src/features/agent-window/messages.ts`. The key is camelCase and the id is
   `app.agent-window.chat.` plus the key in kebab-case (`toastDone` and
   `app.agent-window.chat.toast-done`). The preview server and the browser tests derive keys from
   the ids this way.
2. Add the id to `apps/app-frontend/src/locales/en-US/index.json` with the same English `message`,
   and a translation to `apps/app-frontend/src/locales/zh-CN/index.json`.
3. Use it in the window with `text('toastDone', { title })`. Static markup can use
   `data-text`, `data-placeholder` or `data-label` (which also sets the tooltip of a button).

Arguments must be simple `{name}` placeholders, not ICU plurals or selects.
`formatAgentWindowMessages` formats each message with every `{name}` passed as the literal text
`{name}`, so the placeholders survive translation, and `text()` fills them in. A missing value
leaves its placeholder in place, and a missing key shows the key itself.

`messages.test.ts` checks that every message is registered in en-US with the same text and has a
zh-CN entry, and that formatting keeps the placeholders.

## Running it

### Preview server

```sh
node packages/agent-window/dev/serve.mjs 5317
```

Open `http://127.0.0.1:5317/`. The server serves `src/` unchanged, without caching, so reloading
picks up edits; restart it after editing `serve.mjs` itself. It injects a stand-in host before
`window.js`, with the catalog built from the locale JSON files (falling back to en-US), sample
providers and instances, and permissions that take 400 ms to save. `execution_available` is false,
so the window is always in Preview.

| Query               | Effect                                                                                 |
| ------------------- | -------------------------------------------------------------------------------------- |
| `?lang=zh-CN`       | Locale and catalog.                                                                    |
| `?theme=light`      | Light colours.                                                                         |
| `?accent=%23f472b6` | Accent colour.                                                                         |
| `?ai=off`           | AI turned off in the context.                                                          |
| `?models=0`         | No enabled models.                                                                     |
| `?instances=0`      | No instances.                                                                          |
| `?fail=1`           | The next scripted task fails once while reading (`fail=2`, twice).                     |
| `?fail=plan`        | The next scripted task fails once, just after its plan, which shows "Not carried out". |
| `?speed=0.2`        | Multiplies every delay, so 0.2 plays five times faster.                                |

History lives in this origin's `localStorage`. Remove `axolotl.agent-window.store.v2` in the
developer tools to see the first-run page again.

### Tests

```sh
cd apps/app-frontend
node_modules/.bin/vitest run --config vitest.config.ts --project unit src/features/agent-window
```

- `agent.test.ts`: the reducer, plan helpers, the Preview runtime and Markdown blocks. It also
  checks that every tool name in `demo-flows.js` exists in
  `packages/launcher-tools/src/registry.rs` or `registry/`. Any single-quoted `snake_case` string
  in that file counts as a tool name.
- `conversation-state.test.ts`: the store: rebuilding, migration, budget and failed saves.
- `messages.test.ts`: catalog registration, placeholders and locale formatting.
- `window.test.ts`: the host bridge in a VM context.
- `workspace.test.ts`: the whole window in headless Chromium through Playwright, with a fake clock
  and reduced motion. It needs Playwright's Chromium installed.

```sh
cargo test -p agent-window --lib
```

This covers the open and close lifecycle, permissions, asset responses, that every `.js`, `.css`
and `.woff2` file is in `ASSETS`, and the navigation allow-list. The launcher's command list is
checked by `apps/app/tests/instance_command_manifest_parity.rs` in `theseus_gui`.

### Formatting and lint

`pnpm lint` does not reach this folder, which has no `package.json`. Run the checks by hand from
the repository root:

```sh
apps/app-frontend/node_modules/.bin/prettier --check "packages/agent-window/src/*.{js,css,html}" "packages/agent-window/dev/*.mjs" packages/agent-window/README.md
apps/app-frontend/node_modules/.bin/eslint --config apps/app-frontend/eslint.config.mjs packages/agent-window/src packages/agent-window/dev/serve.mjs
```

Pass Prettier those globs, not the folder: it fails to parse the `.rs` files. The code uses the
repository's Prettier config (four spaces, no semicolons, single quotes, 100 columns).

## The axolotl

The mascot is the launcher's own 3D axolotl, rendered into horizontal sprite strips that
`mascot.css` plays with `steps()`.

```sh
python packages/agent-window/dev/render-mascot.py <toolkit> [--out DIR] [--preview DIR]
```

`<toolkit>` is the 美西螈表情包工具包 folder, which holds `src/rig2.py`, `src/rest_pose.json`,
`src/camera.json` and `model/axolotl.gltf`. Only numpy and Pillow are needed. Strips go to
`src/mascot/` unless `--out` says otherwise, and `--preview` also writes contact sheets and GIFs.
All moods share one camera and framing. The script warns when a pose touches the frame edge and
lists strips it no longer renders; delete those along with their `ASSETS` entries.

Large strips have 192 px frames for the 96 px hero; small ones (`-sm`) have 56 px frames for the
28 px mark (`.axo-sm`, used below about 48 px). Everything plays at 30 fps.

| Mood     | Frames | Length | Plays       | Sizes        | Where the UI uses it                                                       |
| -------- | ------ | ------ | ----------- | ------------ | -------------------------------------------------------------------------- |
| `idle`   | 72     | 2.4 s  | loop        | large, small | Hero at rest, the tour card, the empty task list.                          |
| `think`  | 36     | 1.2 s  | loop        | small        | Status line while the agent reads.                                         |
| `work`   | 27     | 0.9 s  | loop        | small        | Status line while changes are written, after `nod`.                        |
| `happy`  | 27     | 0.9 s  | once        | large, small | Hero when clicked; status line when a run ends done or awaiting; tour end. |
| `sad`    | 24     | 0.8 s  | once, holds | small        | Status line when a run fails.                                              |
| `hello`  | 45     | 1.5 s  | once        | large        | Hero when the new task page comes into view.                               |
| `look`   | 54     | 1.8 s  | once        | large        | Hero every 9 to 16 s while idle; its frames also serve `gaze()`.           |
| `attend` | 15     | 0.5 s  | once, holds | large        | Hero while a request is being typed.                                       |
| `nod`    | 21     | 0.7 s  | once        | small        | Status line on approval, then `work`; the tour card at each step.          |

- `setMood(node, mood, { then })` starts a mood from its first frame and cross-dissolves from the
  frame it replaces, which is frozen on a `::after` ghost for 180 ms (`GHOST_MS`). Setting a
  one-shot again replays it, except `attend`. With `then`, a one-shot hands over to that mood when
  it ends, unless another mood was set meanwhile. Under reduced motion it goes straight to `then`.
- `gaze(node, x)` holds the `look` frame whose head faces `x`, from -1 (far left) to 1 (far right),
  and `gaze(node, null)` lets go. The hero follows the pointer while idle and lets go after 2.4 s
  at rest.
- A mood without a strip at a size has no CSS rule there and shows `idle`. `mascot.js` preloads
  only the strips each size plays (`PLAYED`).
- While the agent works, CSS pixel bubbles rise from the status-line mark, faster while writing.

When a frame count, a length or a size changes in `MOODS` in `render-mascot.py`, mirror it in
`mascot.css` (`--axo-frames` and the animation duration), in `PLAYED` in `mascot.js` and in
`ASSETS`. If the `look` strip changes, update `LOOK_FRAMES` and `GAZE` in `mascot.js` as well.

## Design notes worth keeping

Motion:

- Every animation explains a change of state, and none runs under reduced motion. `animate()` and
  `morph()` return early, and the CSS sets `animation: none` and `transition: none`. Durations are
  not shortened instead: `transition-property` defaults to `all`, so a short duration would
  animate `visibility` and leave elements hidden.
- `morph()` uses View Transitions for page changes such as the first send and switching tasks; a
  newer transition skips the one in progress.
- Keyframes animate `transform` and `opacity` wherever they can. The light around a waiting plan
  card is a rotating gradient behind a mask, so the compositor runs it without repainting. The
  exceptions are the shimmer on status text (`background-position`), the tick drawing
  (`stroke-dashoffset`), the sprite steps and the slide of the docked panes
  (`grid-template-columns`).

Performance:

- A task opens with its latest 40 turns. Older turns are built 40 at a time once the reader
  scrolls within two screens of the top, or down to a turn picked in the navigator, and the view
  stays where it was while they are prepended.
- Every turn but the last two uses `content-visibility: auto`, so off-screen turns skip layout and
  paint. A freshly built turn is laid out in full until a `ResizeObserver` has seen its size, so
  the scroll never meets an estimated height.
- A folded step group builds its rows only when it opens, and a long group shows its latest six
  steps behind "Show N earlier steps".
- Runtime events patch only their turn, once per frame. The task list, header, composer, panel and
  navigator redraw only when what they show changes, and Markdown rebuilds only the blocks whose
  source changed.
- A save that fits the budget costs one `JSON.stringify` and no deep copy.
- `Intl` formatters are made once per locale.

Accessibility:

- Focus guard (`guardFocus` in `dom.js`): when a redraw removes, hides, disables or makes inert
  the focused control, the focus moves to its redrawn twin (same identifying attributes, looked for
  in its own step group, turn, list or popover first), else to the first control left there, else
  to the composer. A pane that closes hands the focus to its toggle first. Removing an attachment
  or a queued message moves the focus to the next one, or to the composer once none are left.
- Announcer (`#announcer`): the open task's latest run is announced once per phase (working, plan
  waiting, applying, and how it ended), never token by token. The agent side of each turn is
  `aria-live="off"`, and other tasks report through toasts.
- The transcript is `aria-busy` while history is built or older turns are prepended, until the
  frame after, so history being shown is not read out as new.
- The whole flow works from the keyboard and with a screen reader. Escape closes an open overlay
  before it stops a run, the ticks on plan changes are checkboxes and the detail choice is a radio
  group with one tab stop.
- All text goes through text nodes; a test checks that stored, provider and agent strings never
  reach an HTML parser.
