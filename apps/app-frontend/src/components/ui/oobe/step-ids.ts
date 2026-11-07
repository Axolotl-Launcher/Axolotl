/**
 * The ids the install's OOBE record is written in, in flow order. They are a
 * persisted contract: never rename, reorder into a reused id, or recycle one
 * that has shipped. To show a changed step to existing installs again, retire
 * its id and add a new version (`appearance_v2`). See AGENTS.md.
 */
export const OOBE_STEP_IDS = [
    'language_v1',
    'account_v1',
    'migration_v1',
    'java_v1',
    'appearance_v1',
    'welcome_v1',
] as const

export type OobeStepId = (typeof OOBE_STEP_IDS)[number]
