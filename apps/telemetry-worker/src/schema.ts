import { z } from 'zod'

const uuid = z.string().regex(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i)
const timestamp = z.string().datetime({ offset: true })
export const HEARTBEAT_RETENTION_DAYS = 35

export function heartbeatDayWindow(now = new Date()): { oldest: string; latest: string } {
    const oldest = new Date(now)
    oldest.setUTCDate(oldest.getUTCDate() - HEARTBEAT_RETENTION_DAYS)
    return { oldest: oldest.toISOString().slice(0, 10), latest: now.toISOString().slice(0, 10) }
}

function isCalendarDay(day: string): boolean {
    const time = Date.parse(`${day}T00:00:00.000Z`)
    return Number.isFinite(time) && new Date(time).toISOString().slice(0, 10) === day
}

export const heartbeatEventSchema = z
    .object({
        type: z.literal('heartbeat'),
        event_id: uuid,
        occurred_at: timestamp,
        day: z
            .string()
            .regex(/^\d{4}-\d{2}-\d{2}$/)
            .refine(isCalendarDay),
    })
    .strict()
    .superRefine((event, context) => {
        const time = Date.parse(event.occurred_at)
        const { oldest, latest } = heartbeatDayWindow()
        if (
            event.day < oldest ||
            event.day > latest ||
            (Number.isFinite(time) && new Date(time).toISOString().slice(0, 10) !== event.day)
        ) {
            context.addIssue({ code: 'custom', path: ['day'], message: 'Invalid heartbeat day' })
        }
    })

export const telemetryEventSchema = heartbeatEventSchema

export const batchSchema = z
    .object({
        schema_version: z.literal(1),
        batch_id: uuid,
        installation_id: uuid,
        app: z
            .object({
                version: z.string().min(1).max(64),
                environment: z.enum(['production', 'development']),
                platform: z.string().min(1).max(32),
                arch: z.string().min(1).max(32),
            })
            .strict(),
        events: z.array(telemetryEventSchema).min(1).max(10),
    })
    .strict()

export type TelemetryBatch = z.infer<typeof batchSchema>
