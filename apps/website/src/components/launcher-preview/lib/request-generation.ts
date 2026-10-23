export type RequestToken = { generation: number; key: string }

export function createRequestGeneration() {
    let generation = 0

    return {
        begin(key: string): RequestToken {
            return { generation: ++generation, key }
        },
        isCurrent(token: RequestToken, key: string): boolean {
            return token.generation === generation && token.key === key
        },
        invalidate(): void {
            generation++
        },
    }
}
