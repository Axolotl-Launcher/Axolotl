import { cloudflareTest, readD1Migrations } from '@cloudflare/vitest-pool-workers'
import { defineConfig } from 'vitest/config'

const migrations = await readD1Migrations('./migrations')

export default defineConfig({
    plugins: [
        cloudflareTest({
            main: './src/index.ts',
            miniflare: {
                compatibilityDate: '2025-08-13',
                d1Databases: ['DB'],
                bindings: {
                    INSTALLATION_HMAC_SECRET:
                        'test-only-secret-that-is-longer-than-thirty-two-bytes',
                    INGEST_ENABLED: 'true',
                    MAX_BATCHES_PER_INSTALLATION_PER_DAY: '25',
                    MAX_ACCEPTED_BATCHES_PER_DAY: '100000',
                    TEST_MIGRATIONS: migrations,
                },
            },
        }),
    ],
    test: {
        setupFiles: ['./test/setup.ts'],
    },
})
