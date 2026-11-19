import { invoke } from '@/helpers/invoke'

export function installTelemetryHandlers(): void {
    window.addEventListener('online', () => {
        void invoke('plugin:telemetry|notify_online').catch(() => undefined)
    })
}
