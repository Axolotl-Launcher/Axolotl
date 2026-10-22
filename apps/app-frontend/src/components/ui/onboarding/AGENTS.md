# Desktop Onboarding

When adding or materially changing a desktop app page, route, navigation entry, large user-facing component, core workflow, settings section, or content-management feature under `apps/app-frontend`, assess the Axolotl onboarding experience.

- Update the onboarding when the feature is relevant to a new user's first-use journey or changes an existing guided workflow.
- Define tours and localized message descriptors in `apps/app-frontend/src/components/ui/onboarding/onboardingConfig.ts`. Keep `OnboardingOverlay` presentational and put reusable runtime behavior in `useOnboardingTour`; do not add step-ID-specific branches to either file.
- Add one stable, semantic `data-onboarding-id` to the component that owns each guided target, then reference that ID through `targetId` in the step configuration. Remove targets when their steps are removed.
- Choose the interaction deliberately: `navigate` must use the real control and wait for its route, `activate` must execute the control's original behavior, and `inspect` must explain a region while any non-onboarding click advances without activating the underlying UI.
- Express workflow branches with `branchByTarget` and `nextByCreationPath` in the step configuration instead of hard-coded conditionals. Missing optional targets must time out and skip rather than trap the tour or show placeholder copy.
- Do not add a navigation step for the page that first-run or replay mode already opens. Establish prerequisite route or modal state before starting the tour, then begin with useful page content.
- Keep the step definition, target, interaction, expected route, branch behavior, and English and Simplified Chinese FormatJS copy synchronized with the feature.
- For a major feature that should not appear in the first-run tour, add an appropriate contextual/replayable tour or document why onboarding is not needed.
- Verify first-run and replay modes, target-missing behavior, narrow windows, and guided modals after changing any target. Guided modals must reserve space for the bottom dialogue instead of rendering beneath it.
- Add future mascot assets only through `OnboardingMascotStage`; do not scatter mascot asset references across onboarding steps.
