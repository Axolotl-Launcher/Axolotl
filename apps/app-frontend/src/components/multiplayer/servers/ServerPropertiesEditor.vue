<script setup lang="ts">
import {
    DropdownIcon,
    FileTextIcon,
    GameIcon,
    GlobeIcon,
    MapIcon,
    MoreHorizontalIcon,
    PackageIcon,
    SettingsIcon,
} from '@modrinth/assets'
import {
    configFieldLabel,
    getConfigFile,
    parseProperties,
    type PropertiesEntry,
    resolveConfigField,
    type ResolvedConfigField,
    serializeProperties,
    setProperty,
} from '@modrinth/server'
import {
    Accordion,
    Button,
    Combobox,
    defineMessages,
    injectNotificationManager,
    type MessageDescriptor,
    StyledInput,
    Toggle,
    useVIntl,
} from '@modrinth/ui'
import { type Component, computed, onMounted, ref } from 'vue'

import StudioEditor from '@/components/instance/studio/StudioEditor.vue'
import { servers } from '@/helpers/servers'

const props = defineProps<{
    serverId: string
}>()

const { formatMessage } = useVIntl()
const messages = defineMessages({
    title: { id: 'app.servers.properties.title', defaultMessage: 'Server properties' },
    formMode: { id: 'app.servers.properties.mode.form', defaultMessage: 'Form' },
    textMode: { id: 'app.servers.properties.mode.text', defaultMessage: 'Text' },
    missing: {
        id: 'app.servers.properties.missing',
        defaultMessage: 'Start the server once to generate this file.',
    },
    loadFailed: {
        id: 'app.servers.properties.load-failed',
        defaultMessage: 'Failed to load the server configuration.',
    },
})

const fieldMessages = defineMessages({
    'server-port': {
        id: 'app.servers.properties.field.server-port',
        defaultMessage: 'Server port',
    },
    difficulty: { id: 'app.servers.properties.field.difficulty', defaultMessage: 'Difficulty' },
    gamemode: { id: 'app.servers.properties.field.gamemode', defaultMessage: 'Game mode' },
    'level-type': { id: 'app.servers.properties.field.level-type', defaultMessage: 'Level type' },
    'max-players': {
        id: 'app.servers.properties.field.max-players',
        defaultMessage: 'Max players',
    },
    'view-distance': {
        id: 'app.servers.properties.field.view-distance',
        defaultMessage: 'View distance',
    },
    'simulation-distance': {
        id: 'app.servers.properties.field.simulation-distance',
        defaultMessage: 'Simulation distance',
    },
    'max-tick-time': {
        id: 'app.servers.properties.field.max-tick-time',
        defaultMessage: 'Max tick time',
    },
    'max-world-size': {
        id: 'app.servers.properties.field.max-world-size',
        defaultMessage: 'Max world size',
    },
    'op-permission-level': {
        id: 'app.servers.properties.field.op-permission-level',
        defaultMessage: 'OP permission level',
    },
    'function-permission-level': {
        id: 'app.servers.properties.field.function-permission-level',
        defaultMessage: 'Function permission level',
    },
    'spawn-protection': {
        id: 'app.servers.properties.field.spawn-protection',
        defaultMessage: 'Spawn protection',
    },
    'player-idle-timeout': {
        id: 'app.servers.properties.field.player-idle-timeout',
        defaultMessage: 'Player idle timeout',
    },
    'network-compression-threshold': {
        id: 'app.servers.properties.field.network-compression-threshold',
        defaultMessage: 'Network compression threshold',
    },
    'rate-limit': { id: 'app.servers.properties.field.rate-limit', defaultMessage: 'Rate limit' },
    'query.port': { id: 'app.servers.properties.field.query.port', defaultMessage: 'Query port' },
    'rcon.port': { id: 'app.servers.properties.field.rcon.port', defaultMessage: 'RCON port' },
    'level-name': { id: 'app.servers.properties.field.level-name', defaultMessage: 'Level name' },
    'level-seed': { id: 'app.servers.properties.field.level-seed', defaultMessage: 'Level seed' },
    motd: {
        id: 'app.servers.properties.field.motd',
        defaultMessage: 'Message of the day (MOTD)',
    },
    'resource-pack': {
        id: 'app.servers.properties.field.resource-pack',
        defaultMessage: 'Resource pack',
    },
    'resource-pack-sha1': {
        id: 'app.servers.properties.field.resource-pack-sha1',
        defaultMessage: 'Resource pack SHA-1',
    },
    'resource-pack-prompt': {
        id: 'app.servers.properties.field.resource-pack-prompt',
        defaultMessage: 'Resource pack prompt',
    },
    'rcon.password': {
        id: 'app.servers.properties.field.rcon.password',
        defaultMessage: 'RCON password',
    },
    'server-ip': { id: 'app.servers.properties.field.server-ip', defaultMessage: 'Server IP' },
    'text-filtering-config': {
        id: 'app.servers.properties.field.text-filtering-config',
        defaultMessage: 'Text filtering config',
    },
    'initial-enabled-packs': {
        id: 'app.servers.properties.field.initial-enabled-packs',
        defaultMessage: 'Initial enabled packs',
    },
    'online-mode': {
        id: 'app.servers.properties.field.online-mode',
        defaultMessage: 'Online mode',
    },
    'white-list': { id: 'app.servers.properties.field.white-list', defaultMessage: 'Whitelist' },
    'enforce-whitelist': {
        id: 'app.servers.properties.field.enforce-whitelist',
        defaultMessage: 'Enforce whitelist',
    },
    'enforce-secure-profile': {
        id: 'app.servers.properties.field.enforce-secure-profile',
        defaultMessage: 'Enforce secure profile',
    },
    'prevent-proxy-connections': {
        id: 'app.servers.properties.field.prevent-proxy-connections',
        defaultMessage: 'Prevent proxy connections',
    },
    'allow-flight': {
        id: 'app.servers.properties.field.allow-flight',
        defaultMessage: 'Allow flight',
    },
    'allow-nether': {
        id: 'app.servers.properties.field.allow-nether',
        defaultMessage: 'Allow the Nether',
    },
    'spawn-animals': {
        id: 'app.servers.properties.field.spawn-animals',
        defaultMessage: 'Spawn animals',
    },
    'spawn-monsters': {
        id: 'app.servers.properties.field.spawn-monsters',
        defaultMessage: 'Spawn monsters',
    },
    'spawn-npcs': { id: 'app.servers.properties.field.spawn-npcs', defaultMessage: 'Spawn NPCs' },
    pvp: {
        id: 'app.servers.properties.field.pvp',
        defaultMessage: 'Player versus player (PvP)',
    },
    'enable-command-block': {
        id: 'app.servers.properties.field.enable-command-block',
        defaultMessage: 'Enable command blocks',
    },
    'enable-status': {
        id: 'app.servers.properties.field.enable-status',
        defaultMessage: 'Enable status',
    },
    'enable-query': {
        id: 'app.servers.properties.field.enable-query',
        defaultMessage: 'Enable query',
    },
    'enable-rcon': {
        id: 'app.servers.properties.field.enable-rcon',
        defaultMessage: 'Enable RCON',
    },
    'enable-jmx-monitoring': {
        id: 'app.servers.properties.field.enable-jmx-monitoring',
        defaultMessage: 'Enable JMX monitoring',
    },
    'force-gamemode': {
        id: 'app.servers.properties.field.force-gamemode',
        defaultMessage: 'Force game mode',
    },
    hardcore: { id: 'app.servers.properties.field.hardcore', defaultMessage: 'Hardcore' },
    'announce-player-achievements': {
        id: 'app.servers.properties.field.announce-player-achievements',
        defaultMessage: 'Announce player achievements',
    },
    'log-ips': { id: 'app.servers.properties.field.log-ips', defaultMessage: 'Log IP addresses' },
    'hide-online-players': {
        id: 'app.servers.properties.field.hide-online-players',
        defaultMessage: 'Hide online players',
    },
    'require-resource-pack': {
        id: 'app.servers.properties.field.require-resource-pack',
        defaultMessage: 'Require resource pack',
    },
    'sync-chunk-writes': {
        id: 'app.servers.properties.field.sync-chunk-writes',
        defaultMessage: 'Sync chunk writes',
    },
    'use-native-transport': {
        id: 'app.servers.properties.field.use-native-transport',
        defaultMessage: 'Use native transport',
    },
    'allow-end': {
        id: 'app.servers.properties.field.allow-end',
        defaultMessage: 'Allow the End',
    },
    'generate-structures': {
        id: 'app.servers.properties.field.generate-structures',
        defaultMessage: 'Generate structures',
    },
    'enable-lan': {
        id: 'app.servers.properties.field.enable-lan',
        defaultMessage: 'Enable LAN',
    },
    'accepts-transfers': {
        id: 'app.servers.properties.field.accepts-transfers',
        defaultMessage: 'Accept player transfers',
    },
    'broadcast-console-to-ops': {
        id: 'app.servers.properties.field.broadcast-console-to-ops',
        defaultMessage: 'Broadcast console to operators',
    },
    'broadcast-rcon-to-ops': {
        id: 'app.servers.properties.field.broadcast-rcon-to-ops',
        defaultMessage: 'Broadcast RCON to operators',
    },
    'bug-report-link': {
        id: 'app.servers.properties.field.bug-report-link',
        defaultMessage: 'Bug report link',
    },
    'chat-spam-threshold-seconds': {
        id: 'app.servers.properties.field.chat-spam-threshold-seconds',
        defaultMessage: 'Chat spam threshold (seconds)',
    },
    'command-spam-threshold-seconds': {
        id: 'app.servers.properties.field.command-spam-threshold-seconds',
        defaultMessage: 'Command spam threshold (seconds)',
    },
    'enable-code-of-conduct': {
        id: 'app.servers.properties.field.enable-code-of-conduct',
        defaultMessage: 'Enable code of conduct',
    },
    'entity-broadcast-range-percentage': {
        id: 'app.servers.properties.field.entity-broadcast-range-percentage',
        defaultMessage: 'Entity broadcast range percentage',
    },
    'generator-settings': {
        id: 'app.servers.properties.field.generator-settings',
        defaultMessage: 'Generator settings',
    },
    'initial-disabled-packs': {
        id: 'app.servers.properties.field.initial-disabled-packs',
        defaultMessage: 'Initial disabled packs',
    },
    'management-server-allowed-origins': {
        id: 'app.servers.properties.field.management-server-allowed-origins',
        defaultMessage: 'Management server allowed origins',
    },
    'management-server-enabled': {
        id: 'app.servers.properties.field.management-server-enabled',
        defaultMessage: 'Enable management server',
    },
    'management-server-host': {
        id: 'app.servers.properties.field.management-server-host',
        defaultMessage: 'Management server host',
    },
    'management-server-port': {
        id: 'app.servers.properties.field.management-server-port',
        defaultMessage: 'Management server port',
    },
    'management-server-secret': {
        id: 'app.servers.properties.field.management-server-secret',
        defaultMessage: 'Management server secret',
    },
    'management-server-tls-enabled': {
        id: 'app.servers.properties.field.management-server-tls-enabled',
        defaultMessage: 'Enable management server TLS',
    },
    'management-server-tls-keystore': {
        id: 'app.servers.properties.field.management-server-tls-keystore',
        defaultMessage: 'Management server TLS keystore',
    },
    'management-server-tls-keystore-password': {
        id: 'app.servers.properties.field.management-server-tls-keystore-password',
        defaultMessage: 'Management server TLS keystore password',
    },
    'max-chained-neighbor-updates': {
        id: 'app.servers.properties.field.max-chained-neighbor-updates',
        defaultMessage: 'Max chained neighbor updates',
    },
    'pause-when-empty-seconds': {
        id: 'app.servers.properties.field.pause-when-empty-seconds',
        defaultMessage: 'Pause when empty (seconds)',
    },
    'region-file-compression': {
        id: 'app.servers.properties.field.region-file-compression',
        defaultMessage: 'Region file compression',
    },
    'resource-pack-id': {
        id: 'app.servers.properties.field.resource-pack-id',
        defaultMessage: 'Resource pack ID',
    },
    'status-heartbeat-interval': {
        id: 'app.servers.properties.field.status-heartbeat-interval',
        defaultMessage: 'Status heartbeat interval',
    },
    'text-filtering-version': {
        id: 'app.servers.properties.field.text-filtering-version',
        defaultMessage: 'Text filtering version',
    },
})

const sectionMessages = defineMessages({
    network: {
        id: 'app.servers.properties.section.network',
        defaultMessage: 'Network & Security',
    },
    world: { id: 'app.servers.properties.section.world', defaultMessage: 'World' },
    gameplay: { id: 'app.servers.properties.section.gameplay', defaultMessage: 'Gameplay' },
    content: { id: 'app.servers.properties.section.content', defaultMessage: 'Content' },
    advanced: { id: 'app.servers.properties.section.advanced', defaultMessage: 'Advanced' },
    others: { id: 'app.servers.properties.section.others', defaultMessage: 'Other' },
})

const SECTION_ICONS = {
    network: GlobeIcon,
    world: MapIcon,
    gameplay: GameIcon,
    content: PackageIcon,
    advanced: SettingsIcon,
    others: MoreHorizontalIcon,
} as const

const FIELD_SECTIONS = [
    {
        title: sectionMessages.network,
        icon: SECTION_ICONS.network,
        openByDefault: true,
        fields: [
            'server-port',
            'server-ip',
            'motd',
            'max-players',
            'online-mode',
            'white-list',
            'enforce-whitelist',
            'enforce-secure-profile',
            'prevent-proxy-connections',
            'hide-online-players',
            'enable-status',
            'enable-query',
            'query.port',
            'enable-rcon',
            'rcon.port',
            'rcon.password',
            'enable-lan',
            'accepts-transfers',
        ],
    },
    {
        title: sectionMessages.world,
        icon: SECTION_ICONS.world,
        openByDefault: true,
        fields: [
            'level-name',
            'level-seed',
            'level-type',
            'generator-settings',
            'generate-structures',
            'spawn-protection',
            'allow-nether',
            'allow-end',
            'allow-flight',
            'view-distance',
            'simulation-distance',
            'entity-broadcast-range-percentage',
            'max-world-size',
            'max-chained-neighbor-updates',
            'region-file-compression',
            'sync-chunk-writes',
        ],
    },
    {
        title: sectionMessages.gameplay,
        icon: SECTION_ICONS.gameplay,
        openByDefault: true,
        fields: [
            'gamemode',
            'force-gamemode',
            'difficulty',
            'hardcore',
            'pvp',
            'spawn-animals',
            'spawn-monsters',
            'spawn-npcs',
            'enable-command-block',
            'announce-player-achievements',
            'player-idle-timeout',
            'pause-when-empty-seconds',
            'max-tick-time',
            'op-permission-level',
            'function-permission-level',
            'network-compression-threshold',
            'rate-limit',
            'chat-spam-threshold-seconds',
            'command-spam-threshold-seconds',
            'bug-report-link',
            'use-native-transport',
        ],
    },
    {
        title: sectionMessages.content,
        icon: SECTION_ICONS.content,
        openByDefault: false,
        fields: [
            'resource-pack',
            'resource-pack-id',
            'resource-pack-sha1',
            'resource-pack-prompt',
            'require-resource-pack',
            'initial-enabled-packs',
            'initial-disabled-packs',
            'enable-code-of-conduct',
            'text-filtering-config',
            'text-filtering-version',
            'log-ips',
            'broadcast-console-to-ops',
            'broadcast-rcon-to-ops',
        ],
    },
    {
        title: sectionMessages.advanced,
        icon: SECTION_ICONS.advanced,
        openByDefault: false,
        fields: [
            'management-server-enabled',
            'management-server-host',
            'management-server-port',
            'management-server-secret',
            'management-server-allowed-origins',
            'management-server-tls-enabled',
            'management-server-tls-keystore',
            'management-server-tls-keystore-password',
            'status-heartbeat-interval',
            'enable-jmx-monitoring',
        ],
    },
]

const FILE_NAME = 'server.properties'

const isLoading = ref(true)
const isMissing = ref(false)
const isSaving = ref(false)
const mode = ref<'form' | 'text'>('form')
const entries = ref<PropertiesEntry[]>([])
const rawText = ref('')
const baselineText = ref('')
const normalizedBaseline = ref('')
const { handleError } = injectNotificationManager()

async function load() {
    isLoading.value = true
    isMissing.value = false
    try {
        const text = await servers.readFile(props.serverId, FILE_NAME)
        entries.value = parseProperties(text)
        rawText.value = text
        baselineText.value = text
        normalizedBaseline.value = serializeProperties(entries.value)
    } catch {
        isMissing.value = true
    } finally {
        isLoading.value = false
    }
}

onMounted(load)

const definition = computed(() => getConfigFile(FILE_NAME))

const isDirty = computed(() =>
    mode.value === 'text'
        ? rawText.value !== baselineText.value
        : serializeProperties(entries.value) !== normalizedBaseline.value,
)

function fieldLabel(key: string): string {
    const descriptor = fieldMessages[key as keyof typeof fieldMessages]
    return descriptor ? formatMessage(descriptor) : configFieldLabel(key)
}

interface FormField {
    key: string
    value: string
    field: ResolvedConfigField
}

const allFormFields = computed<FormField[]>(() =>
    entries.value
        .map((entry) => (entry.type === 'pair' ? entry : null))
        .filter((entry): entry is Extract<PropertiesEntry, { type: 'pair' }> => entry !== null)
        .map((pair) => ({
            key: pair.key,
            value: pair.value,
            field: definition.value
                ? resolveConfigField(definition.value, pair.key, pair.value)
                : { key: pair.key, kind: 'string' as const, inferred: true },
        })),
)

const formSections = computed(() => {
    const knownKeys = new Set(FIELD_SECTIONS.flatMap((section) => section.fields))
    const byKey = new Map(allFormFields.value.map((field) => [field.key, field]))
    const sections: {
        title: MessageDescriptor
        icon: Component
        openByDefault: boolean
        fields: FormField[]
    }[] = FIELD_SECTIONS.flatMap((section) => {
        const fields = section.fields
            .map((key) => byKey.get(key))
            .filter((field): field is FormField => field !== undefined)
        return fields.length === 0
            ? []
            : [
                  {
                      title: section.title,
                      icon: section.icon,
                      openByDefault: section.openByDefault,
                      fields,
                  },
              ]
    })
    const others = allFormFields.value.filter((field) => !knownKeys.has(field.key))
    if (others.length > 0) {
        sections.push({
            title: sectionMessages.others,
            icon: SECTION_ICONS.others,
            openByDefault: false,
            fields: others,
        })
    }
    return sections
})

function setFieldValue(key: string, value: string | number | undefined) {
    entries.value = setProperty(entries.value, key, value?.toString() ?? '')
}

function switchMode(next: 'form' | 'text') {
    if (next === 'text' && mode.value === 'form') {
        rawText.value = serializeProperties(entries.value)
    } else if (next === 'form' && mode.value === 'text') {
        entries.value = parseProperties(rawText.value)
    }
    mode.value = next
}

async function save(): Promise<boolean> {
    if (isMissing.value) return true
    isSaving.value = true
    try {
        const text = mode.value === 'text' ? rawText.value : serializeProperties(entries.value)
        await servers.writeFile(props.serverId, FILE_NAME, text)
        entries.value = parseProperties(text)
        rawText.value = text
        baselineText.value = text
        normalizedBaseline.value = serializeProperties(entries.value)
        return true
    } catch (error) {
        handleError?.(error)
        return false
    } finally {
        isSaving.value = false
    }
}

function cancel() {
    entries.value = parseProperties(baselineText.value)
    rawText.value = baselineText.value
}

defineExpose({ save, cancel, isDirty })
</script>

<template>
    <section data-onboarding-id="server-properties" class="flex flex-col gap-4">
        <div class="flex items-center justify-between gap-3">
            <div class="flex min-w-0 items-center gap-2.5">
                <div
                    class="flex size-9 shrink-0 items-center justify-center rounded-lg bg-surface-3 text-[var(--color-text-primary)]"
                >
                    <FileTextIcon class="size-4" />
                </div>
                <div class="min-w-0">
                    <h3
                        class="m-0 truncate text-base font-semibold text-[var(--color-text-primary)]"
                    >
                        {{ formatMessage(messages.title) }}
                    </h3>
                </div>
            </div>
            <div class="flex items-center gap-2">
                <Button
                    :type="mode === 'form' ? 'highlight' : 'quiet'"
                    size="2xs"
                    native-type="button"
                    @click="switchMode('form')"
                >
                    {{ formatMessage(messages.formMode) }}
                </Button>
                <Button
                    :type="mode === 'text' ? 'highlight' : 'quiet'"
                    size="2xs"
                    native-type="button"
                    @click="switchMode('text')"
                >
                    {{ formatMessage(messages.textMode) }}
                </Button>
            </div>
        </div>

        <p v-if="isMissing" class="m-0 text-[var(--color-text-tertiary)]">
            {{ formatMessage(messages.missing) }}
        </p>

        <template v-else-if="mode === 'form'">
            <div class="flex flex-col">
                <Accordion
                    v-for="section in formSections"
                    :key="section.title.id"
                    :open-by-default="section.openByDefault"
                    overflow-visible
                    :button-class="'group flex min-h-11 w-full cursor-pointer items-center gap-3 bg-transparent px-1 text-left'"
                    class="border-0 border-b border-solid border-surface-4 py-1 last:border-b-0"
                >
                    <template #button="{ open }">
                        <span class="flex min-w-0 flex-1 items-center gap-3">
                            <span
                                class="flex size-7 shrink-0 items-center justify-center rounded-md bg-surface-3 text-[var(--color-text-tertiary)] transition-colors group-hover:text-[var(--color-text-default)]"
                            >
                                <component :is="section.icon" class="size-4" />
                            </span>
                            <span
                                class="min-w-0 flex-1 truncate text-sm font-semibold text-[var(--color-text-default)] group-hover:text-[var(--color-text-primary)]"
                            >
                                {{ formatMessage(section.title) }}
                            </span>
                        </span>
                        <DropdownIcon
                            class="ml-auto size-4 shrink-0 text-[var(--color-text-tertiary)] transition-transform duration-300 group-hover:text-[var(--color-text-default)]"
                            :class="open && 'rotate-180'"
                        />
                    </template>
                    <div
                        class="grid grid-cols-1 gap-x-5 gap-y-3 px-1 pb-4 pt-1 sm:grid-cols-2 xl:grid-cols-3"
                    >
                        <template v-for="item in section.fields" :key="item.key">
                            <div
                                v-if="item.field.kind === 'boolean'"
                                class="flex min-h-9 min-w-0 items-center justify-between gap-3"
                            >
                                <label
                                    class="truncate text-sm font-medium text-[var(--color-text-default)]"
                                    :for="`server-prop-${item.key}`"
                                >
                                    <span v-tooltip="item.key">{{ fieldLabel(item.key) }}</span>
                                </label>
                                <Toggle
                                    :id="`server-prop-${item.key}`"
                                    :model-value="item.value === 'true'"
                                    small
                                    @update:model-value="
                                        setFieldValue(item.key, $event ? 'true' : 'false')
                                    "
                                />
                            </div>

                            <div v-else class="flex min-w-0 flex-col gap-1.5">
                                <label
                                    class="truncate text-sm font-medium text-[var(--color-text-default)]"
                                    :for="`server-prop-${item.key}`"
                                >
                                    <span v-tooltip="item.key">{{ fieldLabel(item.key) }}</span>
                                </label>
                                <StyledInput
                                    v-if="
                                        item.field.kind === 'integer' ||
                                        item.field.kind === 'number'
                                    "
                                    :id="`server-prop-${item.key}`"
                                    :model-value="item.value"
                                    inputmode="numeric"
                                    size="small"
                                    wrapper-class="w-full"
                                    @update:model-value="setFieldValue(item.key, $event)"
                                />

                                <Combobox
                                    v-else-if="item.field.kind === 'enum'"
                                    :model-value="item.value"
                                    :options="
                                        (item.field.options ?? []).map((value) => ({
                                            value,
                                            label: value,
                                        }))
                                    "
                                    width="full"
                                    @update:model-value="setFieldValue(item.key, $event)"
                                />

                                <StyledInput
                                    v-else
                                    :id="`server-prop-${item.key}`"
                                    :model-value="item.value"
                                    size="small"
                                    wrapper-class="w-full"
                                    @update:model-value="setFieldValue(item.key, $event)"
                                />
                            </div>
                        </template>
                    </div>
                </Accordion>
            </div>
        </template>

        <div
            v-else
            class="h-[clamp(18rem,60vh,32rem)] overflow-hidden rounded-lg border border-solid border-surface-4"
        >
            <StudioEditor
                file-path="server.properties"
                language="properties"
                :content="rawText"
                :read-only="isSaving"
                @update:content="rawText = $event"
            />
        </div>
    </section>
</template>
