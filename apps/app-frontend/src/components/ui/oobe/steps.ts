import { defineMessages, type MessageDescriptor } from '@modrinth/ui'
import { type Component, defineAsyncComponent } from 'vue'

import type { OobeStepId } from './step-ids'

export interface OobeStepDefinition {
    id: OobeStepId
    /** Short name for the progress rail. */
    label: MessageDescriptor
    title: MessageDescriptor
    description?: MessageDescriptor
    /** `card` puts the step beside its artwork; `hero` gives it the whole window. */
    layout: 'card' | 'hero'
    /** Whether the footer shows Skip until the step makes a substantive change. */
    skippable: boolean
    component: Component
    art?: Component
}

const messages = defineMessages({
    languageLabel: { id: 'app.oobe.language.label', defaultMessage: 'Language' },
    languageTitle: {
        id: 'app.oobe.language.title',
        defaultMessage: 'Choose the launcher language.',
    },
    languageDescription: {
        id: 'app.oobe.language.description',
        defaultMessage: 'The launcher switches as soon as you pick one.',
    },
    accountLabel: { id: 'app.oobe.account.label', defaultMessage: 'Account' },
    accountTitle: {
        id: 'app.oobe.account.title',
        defaultMessage: 'Who is using the launcher?',
    },
    accountDescription: {
        id: 'app.oobe.account.description',
        defaultMessage: 'Add the account you play Minecraft with.',
    },
    migrationLabel: { id: 'app.oobe.migration.label', defaultMessage: 'Import' },
    migrationTitle: {
        id: 'app.oobe.migration.title',
        defaultMessage: 'Import data from other launchers?',
    },
    migrationDescription: {
        id: 'app.oobe.migration.description',
        defaultMessage: 'Bring your existing instances and .minecraft folders along.',
    },
    javaLabel: { id: 'app.oobe.java.label', defaultMessage: 'Java' },
    javaTitle: {
        id: 'app.oobe.java.title',
        defaultMessage: 'Choose or download Java.',
    },
    javaDescription: {
        id: 'app.oobe.java.description',
        defaultMessage: 'Minecraft runs on Java. Use one you already have, or download one.',
    },
    appearanceLabel: { id: 'app.oobe.appearance.label', defaultMessage: 'Appearance' },
    appearanceTitle: {
        id: 'app.oobe.appearance.title',
        defaultMessage: 'Choose the style you like.',
    },
    welcomeLabel: { id: 'app.oobe.welcome.label', defaultMessage: 'Welcome' },
    welcomeTitle: { id: 'app.oobe.welcome.title', defaultMessage: 'Welcome!' },
})

/**
 * The fixed OOBE flow, in order. It changes only when a maintainer asks for a
 * step to be added or revised; see AGENTS.md before touching it.
 */
export const OOBE_STEPS: readonly OobeStepDefinition[] = [
    {
        id: 'language_v1',
        label: messages.languageLabel,
        title: messages.languageTitle,
        description: messages.languageDescription,
        layout: 'card',
        skippable: false,
        component: defineAsyncComponent(() => import('./steps/language/LanguageStep.vue')),
        art: defineAsyncComponent(() => import('./steps/language/LanguageArt.vue')),
    },
    {
        id: 'account_v1',
        label: messages.accountLabel,
        title: messages.accountTitle,
        description: messages.accountDescription,
        layout: 'card',
        skippable: true,
        component: defineAsyncComponent(() => import('./steps/account/AccountStep.vue')),
        art: defineAsyncComponent(() => import('./steps/account/AccountArt.vue')),
    },
    {
        id: 'migration_v1',
        label: messages.migrationLabel,
        title: messages.migrationTitle,
        description: messages.migrationDescription,
        layout: 'card',
        skippable: true,
        component: defineAsyncComponent(() => import('./steps/migration/MigrationStep.vue')),
        art: defineAsyncComponent(() => import('./steps/migration/MigrationArt.vue')),
    },
    {
        id: 'java_v1',
        label: messages.javaLabel,
        title: messages.javaTitle,
        description: messages.javaDescription,
        layout: 'card',
        skippable: true,
        component: defineAsyncComponent(() => import('./steps/java/JavaStep.vue')),
        art: defineAsyncComponent(() => import('./steps/java/JavaArt.vue')),
    },
    {
        id: 'appearance_v1',
        label: messages.appearanceLabel,
        title: messages.appearanceTitle,
        layout: 'card',
        skippable: false,
        component: defineAsyncComponent(() => import('./steps/appearance/AppearanceStep.vue')),
        art: defineAsyncComponent(() => import('./steps/appearance/AppearanceArt.vue')),
    },
    {
        id: 'welcome_v1',
        label: messages.welcomeLabel,
        title: messages.welcomeTitle,
        layout: 'hero',
        skippable: false,
        component: defineAsyncComponent(() => import('./steps/welcome/WelcomeStep.vue')),
    },
]
