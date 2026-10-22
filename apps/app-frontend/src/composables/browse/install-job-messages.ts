import { defineMessage, defineMessages, type MessageDescriptor } from '@modrinth/ui'

import type { InstallPhaseId } from '@/helpers/install'

export const installPhaseMessages = defineMessages({
    creating_backup: { id: 'app.install.phase.creating-backup', defaultMessage: 'Creating backup' },
    staging_content: { id: 'app.install.phase.staging-content', defaultMessage: 'Staging content' },
    applying_content: {
        id: 'app.install.phase.applying-content',
        defaultMessage: 'Applying content',
    },
    updating_loader: { id: 'app.install.phase.updating-loader', defaultMessage: 'Updating loader' },
    verifying: { id: 'app.install.phase.verifying', defaultMessage: 'Verifying files' },
    completed: { id: 'app.install.phase.completed', defaultMessage: 'Completed' },
    exporting: { id: 'app.downloads.phase.exporting', defaultMessage: 'Exporting selected files' },
    preparing_instance: {
        id: 'app.install.phase.preparing_instance',
        defaultMessage: 'Queued to install',
    },
    resolving_pack: {
        id: 'app.install.phase.resolving_pack',
        defaultMessage: 'Resolving content',
    },
    downloading_pack_file: {
        id: 'app.install.phase.downloading_pack_file',
        defaultMessage: 'Downloading .mrpack',
    },
    reading_pack_manifest: {
        id: 'app.install.phase.reading_pack_manifest',
        defaultMessage: 'Reading pack manifest',
    },
    downloading_content: {
        id: 'app.install.phase.downloading_content',
        defaultMessage: 'Downloading content',
    },
    extracting_overrides: {
        id: 'app.install.phase.extracting_overrides',
        defaultMessage: 'Extracting overrides',
    },
    resolving_minecraft: {
        id: 'app.install.phase.resolving_minecraft',
        defaultMessage: 'Resolving Minecraft',
    },
    resolving_loader: {
        id: 'app.install.phase.resolving_loader',
        defaultMessage: 'Resolving loader',
    },
    preparing_java: {
        id: 'app.install.phase.preparing_java',
        defaultMessage: 'Preparing Java',
    },
    downloading_minecraft: {
        id: 'app.install.phase.downloading_minecraft',
        defaultMessage: 'Downloading Minecraft',
    },
    running_loader_processors: {
        id: 'app.install.phase.running_loader_processors',
        defaultMessage: 'Running loader processors',
    },
    finalizing: {
        id: 'app.install.phase.finalizing',
        defaultMessage: 'Finalizing',
    },
    rolling_back: {
        id: 'app.install.phase.rolling_back',
        defaultMessage: 'Rolling back',
    },
}) satisfies Record<InstallPhaseId, MessageDescriptor>

const unknownPhase = defineMessage({ id: 'app.install.phase.unknown', defaultMessage: 'Working' })
export function installPhaseMessage(phase: string): MessageDescriptor {
    return (
        (installPhaseMessages as Partial<Record<string, MessageDescriptor>>)[phase] ?? unknownPhase
    )
}
