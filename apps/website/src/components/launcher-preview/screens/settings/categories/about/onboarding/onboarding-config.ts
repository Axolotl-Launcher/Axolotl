import type { MessageDescriptor } from '@modrinth/ui/src/composables/i18n.ts'

import { messages } from '../../../messages'

/**
 * The guided tours of apps/app-frontend/src/components/ui/onboarding/
 * onboardingConfig.ts: the same steps, targets, interactions, routes and
 * branches.
 */
export type OnboardingInteraction = 'manual' | 'navigate' | 'activate' | 'inspect'
export type OnboardingMode = 'main' | 'instance' | 'logagent' | 'seed-map' | 'brewing'
export type CreationPath = 'custom'
export type StepDestination = string | 'complete'

export type OnboardingStep = {
    id: string
    targetId?: string
    interaction: OnboardingInteraction
    title: MessageDescriptor
    description: MessageDescriptor
    action: MessageDescriptor
    spotlight?: 'control'
    expectedPath?: string
    next?: StepDestination
    closeSettingsAfter?: boolean
    nextByCreationPath?: Partial<Record<CreationPath, StepDestination>>
    branchByTarget?: Record<
        string,
        {
            creationPath?: CreationPath
            next: StepDestination
        }
    >
}

const step = (
    id: string,
    interaction: OnboardingInteraction,
    copy: {
        title: MessageDescriptor
        description: MessageDescriptor
        action: MessageDescriptor
    },
    options: Omit<OnboardingStep, 'id' | 'interaction' | 'title' | 'description' | 'action'> = {},
): OnboardingStep => ({ id, interaction, ...copy, ...options })

const copy = (
    title: MessageDescriptor,
    description: MessageDescriptor,
    action: MessageDescriptor,
) => ({ title, description, action })

const control = (targetId: string, expectedPath?: string) => ({
    targetId,
    spotlight: 'control' as const,
    ...(expectedPath ? { expectedPath } : {}),
})

const inspect = (
    id: string,
    targetId: string,
    title: MessageDescriptor,
    description: MessageDescriptor,
) =>
    step(id, 'inspect', copy(title, description, messages.appOnboardingActionContinueArea), {
        targetId,
    })

const settingsTourSteps: Array<{
    id: string
    targetId: string
    title: MessageDescriptor
    description: MessageDescriptor
}> = [
    {
        id: 'settings-interface',
        targetId: 'settings-tab-interface',
        title: messages.appOnboardingAppearanceTitle,
        description: messages.appOnboardingAppearanceDescription,
    },
    {
        id: 'settings-language-translation',
        targetId: 'settings-tab-language-translation',
        title: messages.appOnboardingLanguageTitle,
        description: messages.appOnboardingLanguageDescription,
    },
    {
        id: 'settings-launch-defaults',
        targetId: 'settings-tab-launch-defaults',
        title: messages.appOnboardingDefaultsTitle,
        description: messages.appOnboardingDefaultsDescription,
    },
    {
        id: 'settings-content-downloads',
        targetId: 'settings-tab-content-downloads',
        title: messages.appOnboardingResourcesTitle,
        description: messages.appOnboardingResourcesDescription,
    },
]

export const onboardingTours: Record<OnboardingMode, OnboardingStep[]> = {
    'seed-map': [
        inspect(
            'seed-map-area',
            'seed-map-canvas',
            messages.appOnboardingSeedMapAreaTitle,
            messages.appOnboardingSeedMapAreaDescription,
        ),
        inspect(
            'seed-map-advanced',
            'seed-map-advanced-guide',
            messages.appOnboardingSeedMapTitle,
            messages.appOnboardingSeedMapDescription,
        ),
        inspect(
            'seed-map-underground',
            'seed-map-underground-mode',
            messages.appOnboardingSeedMapUndergroundTitle,
            messages.appOnboardingSeedMapUndergroundDescription,
        ),
    ],
    brewing: [
        inspect(
            'recipe-brewing',
            'recipe-brewing-editor',
            messages.appOnboardingBrewingTitle,
            messages.appOnboardingBrewingDescription,
        ),
    ],
    logagent: [
        inspect(
            'logagent-source',
            'logagent-settings-source',
            messages.appOnboardingLogagentSourceTitle,
            messages.appOnboardingLogagentSourceDescription,
        ),
        inspect(
            'logagent-sharing',
            'logagent-settings-sharing',
            messages.appOnboardingLogagentSharingTitle,
            messages.appOnboardingLogagentSharingDescription,
        ),
        inspect(
            'logagent-cloud',
            'logagent-settings-cloud',
            messages.appOnboardingLogagentCloudTitle,
            messages.appOnboardingLogagentCloudDescription,
        ),
    ],
    main: [
        step(
            'welcome',
            'manual',
            copy(
                messages.appOnboardingWelcomeTitle,
                messages.appOnboardingWelcomeDescription,
                messages.appOnboardingActionStart,
            ),
        ),
        inspect(
            'home-widget-grid',
            'home-widget-grid',
            messages.appOnboardingHomeWidgetsTitle,
            messages.appOnboardingHomeWidgetsDescription,
        ),
        inspect(
            'home-widget-customize',
            'home-widget-customize',
            messages.appOnboardingHomeCustomizeTitle,
            messages.appOnboardingHomeCustomizeDescription,
        ),
        inspect(
            'home-layout-switch',
            'home-layout-switch',
            messages.appOnboardingHomeLayoutTitle,
            messages.appOnboardingHomeLayoutDescription,
        ),
        step(
            'discover-navigation',
            'navigate',
            copy(
                messages.appOnboardingDiscoverTitle,
                messages.appOnboardingDiscoverDescription,
                messages.appOnboardingActionClickDiscover,
            ),
            // Discover keeps the last project type, so the path is not fixed.
            control('nav-discover'),
        ),
        inspect(
            'discover-content',
            'browse-content',
            messages.appOnboardingBrowseTitle,
            messages.appOnboardingBrowseDescription,
        ),
        step(
            'discover-favorites-navigation',
            'navigate',
            copy(
                messages.appOnboardingFavoritesTitle,
                messages.appOnboardingFavoritesDescription,
                messages.appOnboardingActionClickFavorites,
            ),
            control('browse-favorites-tab', '/browse/favorites'),
        ),
        inspect(
            'discover-favorites-content',
            'browse-favorites-content',
            messages.appOnboardingFavoritesTitle,
            messages.appOnboardingFavoritesDescription,
        ),
        step(
            'skins-navigation',
            'navigate',
            copy(
                messages.appOnboardingSkinsTitle,
                messages.appOnboardingSkinsDescription,
                messages.appOnboardingActionClickSkins,
            ),
            control('nav-skins', '/skins'),
        ),
        inspect(
            'skins-page',
            'skins-page',
            messages.appOnboardingSkinsPageTitle,
            messages.appOnboardingSkinsPageDescription,
        ),
        step(
            'account',
            'inspect',
            copy(
                messages.appOnboardingAccountTitle,
                messages.appOnboardingAccountDescription,
                messages.appOnboardingActionContinueArea,
            ),
            control('account-entry'),
        ),
        inspect(
            'offline-account',
            'offline-account-entry',
            messages.appOnboardingOfflineAccountTitle,
            messages.appOnboardingOfflineAccountDescription,
        ),
        step(
            'lab-navigation',
            'navigate',
            copy(
                messages.appOnboardingLabTitle,
                messages.appOnboardingLabDescription,
                messages.appOnboardingActionClickLab,
            ),
            control('nav-lab', '/lab'),
        ),
        inspect(
            'lab-tools',
            'lab-tools',
            messages.appOnboardingLabToolsTitle,
            messages.appOnboardingLabToolsDescription,
        ),
        step(
            'lab-gradient-text-navigation',
            'navigate',
            copy(
                messages.appOnboardingLabEditorTitle,
                messages.appOnboardingLabEditorDescription,
                messages.appOnboardingActionOpenGradientText,
            ),
            control('lab-gradient-text-card', '/lab/gradient-text'),
        ),
        inspect(
            'lab-gradient-text-editor',
            'lab-gradient-text-editor',
            messages.appOnboardingLabEditorTitle,
            messages.appOnboardingLabEditorDescription,
        ),
        step(
            'lab-return-navigation',
            'navigate',
            copy(
                messages.appOnboardingLabSeedMapTitle,
                messages.appOnboardingLabSeedMapDescription,
                messages.appOnboardingActionReturnLab,
            ),
            control('nav-lab', '/lab'),
        ),
        step(
            'lab-seed-map-navigation',
            'navigate',
            copy(
                messages.appOnboardingLabSeedMapTitle,
                messages.appOnboardingLabSeedMapDescription,
                messages.appOnboardingActionOpenSeedMap,
            ),
            control('lab-seed-map-card', '/lab/seed-map'),
        ),
        inspect(
            'lab-seed-map-workspace',
            'seed-map-workspace',
            messages.appOnboardingLabSeedMapTitle,
            messages.appOnboardingLabSeedMapDescription,
        ),
        step(
            'lab-return-schematic-navigation',
            'navigate',
            copy(
                messages.appOnboardingLabSchematicTitle,
                messages.appOnboardingLabSchematicDescription,
                messages.appOnboardingActionReturnLab,
            ),
            control('nav-lab', '/lab'),
        ),
        step(
            'lab-schematic-navigation',
            'navigate',
            copy(
                messages.appOnboardingLabSchematicTitle,
                messages.appOnboardingLabSchematicDescription,
                messages.appOnboardingActionOpenSchematicWorkshop,
            ),
            control('lab-schematic-preview-card', '/lab/schematic-preview'),
        ),
        inspect(
            'lab-schematic-workspace',
            'schematic-preview-workspace',
            messages.appOnboardingLabSchematicTitle,
            messages.appOnboardingLabSchematicDescription,
        ),
        step(
            'lab-recipe-generator-navigation',
            'navigate',
            copy(
                messages.appOnboardingLabRecipeGeneratorTitle,
                messages.appOnboardingLabRecipeGeneratorDescription,
                messages.appOnboardingActionOpenRecipeGenerator,
            ),
            control('lab-recipe-generator-card', '/lab/recipe-generator'),
        ),
        inspect(
            'lab-recipe-generator-workspace',
            'recipe-generator-workspace',
            messages.appOnboardingLabRecipeGeneratorTitle,
            messages.appOnboardingLabRecipeGeneratorDescription,
        ),
        inspect(
            'downloads-tabs',
            'downloads-tabs',
            messages.appOnboardingDownloadsPageTitle,
            messages.appOnboardingDownloadsPageDescription,
        ),
        step(
            'settings-navigation',
            'activate',
            copy(
                messages.appOnboardingSettingsTitle,
                messages.appOnboardingSettingsDescription,
                messages.appOnboardingActionClickSettings,
            ),
            control('nav-settings', '/settings'),
        ),
        ...settingsTourSteps.map((entry, index) =>
            step(
                entry.id,
                'activate',
                copy(entry.title, entry.description, messages.appOnboardingActionClickTab),
                {
                    ...control(entry.targetId),
                    closeSettingsAfter: index === settingsTourSteps.length - 1,
                },
            ),
        ),
        step(
            'library-navigation',
            'navigate',
            copy(
                messages.appOnboardingLibraryTitle,
                messages.appOnboardingLibraryDescription,
                messages.appOnboardingActionClickLibrary,
            ),
            control('nav-library', '/library'),
        ),
        inspect(
            'library-content',
            'library-content',
            messages.appOnboardingLibraryPageTitle,
            messages.appOnboardingLibraryPageDescription,
        ),
        step(
            'create-instance',
            'navigate',
            copy(
                messages.appOnboardingCreateTitle,
                messages.appOnboardingCreateDescription,
                messages.appOnboardingActionClickCreate,
            ),
            control('create-instance', '/create'),
        ),
        step(
            'creation-flow',
            'activate',
            copy(
                messages.appOnboardingCreationTitle,
                messages.appOnboardingCreationDescription,
                messages.appOnboardingActionClickCreationMethod,
            ),
            {
                targetId: 'creation-methods',
                branchByTarget: {
                    'creation-method-custom': { creationPath: 'custom', next: 'creation-name' },
                    'creation-method-import': { next: 'complete' },
                },
            },
        ),
        inspect(
            'creation-name',
            'creation-name',
            messages.appOnboardingCreationNameTitle,
            messages.appOnboardingCreationNameDescription,
        ),
        inspect(
            'creation-loader',
            'creation-loader',
            messages.appOnboardingCreationLoaderTitle,
            messages.appOnboardingCreationLoaderDescription,
        ),
        step(
            'creation-version',
            'inspect',
            copy(
                messages.appOnboardingCreationVersionTitle,
                messages.appOnboardingCreationVersionDescription,
                messages.appOnboardingActionContinueArea,
            ),
            {
                targetId: 'creation-game-version',
                nextByCreationPath: { custom: 'creation-confirm' },
            },
        ),
        step(
            'creation-confirm',
            'inspect',
            copy(
                messages.appOnboardingCreationConfirmTitle,
                messages.appOnboardingCreationConfirmDescription,
                messages.appOnboardingActionFinishArea,
            ),
            { targetId: 'creation-confirm' },
        ),
    ],
    instance: [
        inspect(
            'instance-actions',
            'instance-actions',
            messages.appOnboardingInstanceActionsTitle,
            messages.appOnboardingInstanceActionsDescription,
        ),
        step(
            'instance-tabs',
            'inspect',
            copy(
                messages.appOnboardingInstanceTabsTitle,
                messages.appOnboardingInstanceTabsDescription,
                messages.appOnboardingActionFinishArea,
            ),
            { targetId: 'instance-tabs' },
        ),
    ],
}

export function onboardingTargetSelector(targetId: string) {
    return `[data-onboarding-id="${targetId}"]`
}
