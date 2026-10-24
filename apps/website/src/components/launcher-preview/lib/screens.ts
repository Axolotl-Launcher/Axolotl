import type { PreviewScreens } from './router'

/**
 * Screen entry points by route name. Every screen is a separate chunk that is
 * fetched the first time the route is visited.
 */
export const previewScreens: PreviewScreens = {
    Home: () => import('../screens/home/index.vue'),
    DiscoverContent: () => import('../screens/discover/index.vue'),
    Favorites: () => import('../screens/discover/Favorites.vue'),
    Project: () => import('../screens/discover/Project.vue'),
    Downloads: () => import('../screens/downloads/index.vue'),
    Library: () => import('../screens/library/index.vue'),
    Instance: () => import('../screens/instance/index.vue'),
    InstanceContent: () => import('../screens/instance/ContentTab.vue'),
    InstanceWorlds: () => import('../screens/instance/WorldsTab.vue'),
    InstanceWorldEditor: () => import('../screens/instance/WorldEditorPage.vue'),
    InstanceScreenshots: () => import('../screens/instance/ScreenshotsTab.vue'),
    InstanceFiles: () => import('../screens/instance/FilesTab.vue'),
    InstanceFileStudio: () => import('../screens/instance/studio/FileStudio.vue'),
    InstanceLogs: () => import('../screens/instance/LogsTab.vue'),
    InstanceUpgrade: () => import('../screens/instance/upgrade/UpgradeShell.vue'),
    InstanceUpgradeSelect: () => import('../screens/instance/upgrade/Select.vue'),
    InstanceUpgradeCompatibility: () => import('../screens/instance/upgrade/Compatibility.vue'),
    InstanceUpgradeCustomize: () => import('../screens/instance/upgrade/Customize.vue'),
    InstanceUpgradeConfirm: () => import('../screens/instance/upgrade/Confirm.vue'),
    InstanceUpgradeProgress: () => import('../screens/instance/upgrade/Progress.vue'),
    InstanceUpgradeResult: () => import('../screens/instance/upgrade/Result.vue'),
    Screenshots: () => import('../screens/screenshots/index.vue'),
    SkinSelector: () => import('../screens/skins/index.vue'),
    Multiplayer: () => import('../screens/multiplayer/index.vue'),
    Lab: () => import('../screens/lab/index.vue'),
    GradientTextGenerator: () => import('../screens/lab/GradientText.vue'),
    ModTranslation: () => import('../screens/lab/ModTranslation.vue'),
    SkinEditor: () => import('../screens/lab/SkinEditor.vue'),
    SeedMap: () => import('../screens/lab/SeedMap.vue'),
    SchematicWorkshop: () => import('../screens/lab/SchematicWorkshop.vue'),
    RecipeGenerator: () => import('../screens/lab/RecipeGenerator.vue'),
    CommandGenerator: () => import('../screens/lab/CommandGenerator.vue'),
    Create: () => import('../screens/create/index.vue'),
    DropHelp: () => import('../screens/create/DropHelp.vue'),
    Settings: () => import('../screens/settings/index.vue'),
}
