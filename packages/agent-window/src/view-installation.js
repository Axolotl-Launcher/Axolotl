import { h } from './dom.js'
import { sizeLabel, text } from './messages.js'

function fields(rows) {
    return h(
        'dl',
        { class: 'change-arguments' },
        rows
            .filter(([, value]) => value !== undefined && value !== null && value !== '')
            .flatMap(([label, value]) => [
                h('dt', { text: label }),
                h('dd', { text: String(value) }),
            ]),
    )
}

function target(review) {
    const instance = review.instance
    return fields([
        [text('argumentInstanceId'), instance?.name || review.name],
        [text('argumentGameVersion'), instance?.game_version || review.pack?.game_version],
        [text('argumentLoader'), loaderName(instance?.loader || review.pack?.loader)],
    ])
}

export function installationReview(review) {
    if (review.mode === 'modpack') {
        const pack = review.pack || {}
        return h('section', { class: 'installation-review' }, [
            target(review),
            fields([
                [text('argumentProjectId'), pack.title || review.name],
                [text('argumentVersion'), pack.version],
                [text('argumentSource'), pack.source],
                [text('argumentPath'), pack.file_name],
                [text('planGameVersions'), pack.declared_compatibility?.gameVersions?.join(' · ')],
                [text('argumentLoader'), pack.declared_compatibility?.loaders?.join(' · ')],
            ]),
            h('p', {
                class: review.replaces_pack_content ? 'installation-warning' : 'change-note',
                text: text(
                    review.replaces_pack_content
                        ? 'packReplacementEffect'
                        : 'packNewInstanceEffect',
                ),
            }),
        ])
    }
    const actionKeys = {
        install: 'changeKindInstall',
        replace: 'planReplace',
        enable: 'planEnable',
        keep: 'planKeep',
    }
    const nodeTitles = new Map(review.actions.map((action) => [action.node_id, action.title]))
    return h('section', { class: 'installation-review' }, [
        target(review),
        !review.dependencies_checked
            ? h('p', { class: 'installation-warning', text: text('planDependenciesSkipped') })
            : null,
        !review.install_dependencies
            ? h('p', { class: 'installation-warning', text: text('planDependencyInstallSkipped') })
            : null,
        h(
            'ul',
            { class: 'installation-files' },
            review.actions.map((action) =>
                h('li', {}, [
                    h('div', { class: 'installation-file-title' }, [
                        h('strong', { text: action.title }),
                        h('span', { text: action.version || '' }),
                        action.dependency
                            ? h('span', { class: 'change-kind', text: text('planDependency') })
                            : null,
                    ]),
                    h('p', {
                        class: 'change-note',
                        text: [
                            text(actionKeys[action.kind] || 'changeKindInstall'),
                            sourceName(action.source),
                            typeof action.size === 'number' ? sizeLabel(action.size) : '',
                        ]
                            .filter(Boolean)
                            .join(' · '),
                    }),
                    action.scope?.kind === 'world'
                        ? h('p', {
                              class: 'change-note',
                              text: text('planWorldScope', { path: action.scope.relativePath }),
                          })
                        : null,
                    action.current_path && action.current_path !== action.target_path
                        ? h('code', {
                              class: 'installation-path is-from',
                              text: action.current_path,
                          })
                        : null,
                    h('code', { class: 'installation-path', text: action.target_path }),
                ]),
            ),
        ),
        ...(review.issues || []).map((issue) =>
            h('div', { class: 'installation-warning' }, [
                h('strong', { text: issueTitle(issue.code) }),
                issue.conflict
                    ? fields([
                          [text('planSubject'), issue.conflict.subject],
                          [text('planRequired'), issue.conflict.required],
                          [text('planActual'), issue.conflict.actual],
                      ])
                    : null,
                h('details', {}, [
                    h('summary', { text: text('planOriginalDetails') }),
                    h('p', { text: issue.message }),
                ]),
            ]),
        ),
        ...(review.exceptions || []).map((exception) =>
            h('p', {
                class: 'installation-warning',
                text: text('planCompatibilityAccepted', {
                    name: nodeTitles.get(exception.selectionId) || text('argumentProjectId'),
                    dimensions: exception.dimensions
                        .map((dimension) =>
                            text(
                                dimension === 'gameVersion'
                                    ? 'argumentGameVersion'
                                    : 'argumentLoader',
                            ),
                        )
                        .join(' · '),
                }),
            }),
        ),
    ])
}

function issueTitle(code) {
    if (typeof code !== 'string' || !code) return text('planIssueUnknown')
    const key = `planIssue${code[0].toUpperCase()}${code.slice(1)}`
    const label = text(key)
    return label === key ? text('planIssueUnknown') : label
}

export function sourceName(source) {
    return (
        {
            modrinth: 'Modrinth',
            curseforge: 'CurseForge',
            mcarchive: 'MCArchive',
            planet_minecraft: 'Planet Minecraft',
        }[source] ||
        source ||
        ''
    )
}

function loaderName(loader) {
    return (
        {
            neoforge: 'NeoForge',
            neoForge: 'NeoForge',
            forge: 'Forge',
            fabric: 'Fabric',
            quilt: 'Quilt',
            vanilla: 'Vanilla',
        }[loader] ||
        loader ||
        ''
    )
}
