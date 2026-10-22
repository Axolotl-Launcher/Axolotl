export type SkinAccountSession = number

export function createSkinAccountSession() {
    let currentSession = 0

    return {
        begin(): SkinAccountSession {
            return ++currentSession
        },
        isCurrent(session: SkinAccountSession): boolean {
            return session === currentSession
        },
    }
}
