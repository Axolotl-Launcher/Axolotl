# First-Run OOBE

The OOBE is the full-window setup a new install sees before anything else. It is a fixed flow, not a feature tour.

## Scope

- Do not add, remove, reorder or reword OOBE steps unless the user explicitly asks for that OOBE change. A new page, route, setting or workflow is not a reason to extend the OOBE; it is introduced where it lives in the app.
- The steps are listed once, in order, in `steps.ts`. Their persisted ids are listed in `step-ids.ts`.
- The launcher has no guided tour, spotlight or `data-onboarding-id` targets. Do not bring them back as part of OOBE work.

## Step ids and records

- `oobe_completed_steps` in the settings holds the ids of the steps an install has moved past (Next, Skip or finishing), or in which it made a change through `markChanged` (a sign-in, a queued import, a linked folder), so a restart never offers to repeat that change. At startup, `pendingOobeSteps` shows only the steps whose ids are missing. So a release that adds a step shows that step alone on the first start after the update.
- An id is `<meaningful_name>_v<n>`, for example `appearance_v1`. Once a release has shipped an id, never rename it, reuse it for different content, or delete it from the record.
- To show a revised step to installs that already saw it, replace its entry with a new id (`appearance_v2`) and drop the old id from `OOBE_STEP_IDS`. To revise copy or styling without showing it again, keep the id.
- Unknown ids in a record are kept. They belong to a newer build the install ran before.
- `useOobe.ts` writes the record one step at a time, as the user moves on. A failed write logs a warning and never traps the user; the step returns at the next start.

## Structure

- `OobeHost.vue` owns the window: backdrop, title strip with window controls, progress rail, card or hero layout, transitions and the Back / Skip / Next footer. Do not add per-step branches to it.
- A step is a component under `steps/<name>/` plus an optional `*Art.vue` illustration. It talks to the host only through `useOobeStep()` (`setBusy`, `setReady`, `next`) and never navigates on its own.
- A step applies its choice as the user makes it, through the same helpers the settings pages use, so the OOBE and Settings cannot drift apart. Reuse existing message ids where the wording must match the rest of the app, with identical `defaultMessage`.
- `App.vue` decides when the OOBE shows: after initialization and before privacy consent, the post-update announcement and community announcements. While it is open the app shell is `invisible` and `inert`. It sits below the modal layer (z-index 95), so sign-in, anti-piracy and import modals open on top of it.
- Account steps must respect the anti-piracy policy: offline accounts stay unavailable while `useAntiPiracyStatus().status.restricted` is true, and a restricted action shows the existing notice through `useError().showAntiPiracyNotice()`.

## Visuals

- Illustrations and previews are vector (inline SVG or CSS) built from theme tokens. Draw text that carries no meaning as skeleton bars. `parts/LauncherSketch.vue` is the shared launcher mock-up; wrap it in a `*-mode` class to preview a theme.
- Every animation has a `prefers-reduced-motion: reduce` fallback.
- Check both the light and dark themes and the minimum window size (1100 × 700) after changing a step.
