import contributorsData from './data/contributors.json'
import teamData from './data/team.json'

/**
 * The About page's people (apps/app-frontend/src/data/about): the development
 * team with its bundled avatars, and the repository contributors.
 */
export interface TeamMember {
    name: string
    avatar: string
    avatarRemote?: string
    url?: string
    experience?: string
}

export interface Contributor {
    name: string
    avatarUrl: string
    url: string
    contributions: number
}

const teamAvatarModules = import.meta.glob('./data/avatars/*', {
    eager: true,
    import: 'default',
    query: '?url',
}) as Record<string, string>

export const teamMembers: (TeamMember & { avatarUrl: string })[] = (teamData as TeamMember[]).map(
    (member) => ({
        ...member,
        avatarUrl: teamAvatarModules[`./data/avatars/${member.avatar}`] ?? '',
    }),
)

export const contributors = contributorsData as Contributor[]
