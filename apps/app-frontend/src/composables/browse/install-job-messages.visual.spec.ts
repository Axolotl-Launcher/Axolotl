import { expect, it } from 'vitest'

import { installPhaseMessage, installPhaseMessages } from './install-job-messages'

it('provides a message descriptor for every task stage including export verification', () => {
    for (const phase of [
        'exporting',
        'preparing_instance',
        'creating_backup',
        'staging_content',
        'resolving_pack',
        'downloading_pack_file',
        'reading_pack_manifest',
        'downloading_content',
        'extracting_overrides',
        'resolving_minecraft',
        'resolving_loader',
        'preparing_java',
        'downloading_minecraft',
        'running_loader_processors',
        'applying_content',
        'updating_loader',
        'verifying',
        'finalizing',
        'completed',
        'rolling_back',
    ]) {
        const message = installPhaseMessage(phase)
        expect(message.id).toBeTruthy()
        expect(message.defaultMessage).toBeTruthy()
        expect(message).toBe(installPhaseMessages[phase as keyof typeof installPhaseMessages])
    }
    expect(installPhaseMessage('verifying').defaultMessage).toBe('Verifying files')
})

it('renders an unknown persisted stage with a valid fallback descriptor', () => {
    expect(installPhaseMessage('future-stage')).toEqual({
        id: 'app.install.phase.unknown',
        defaultMessage: 'Working',
    })
})
