// noinspection JSUnusedGlobalSymbols

export const getUserLink = (user: { username: string }) => {
    return `/user/${user.username}`
}

export const isStaff = (user: { role: string } | null | undefined) => {
    return user && STAFF_ROLES.includes(user.role)
}

export const isAdmin = (user: { role: string } | null | undefined) => {
    return user && user.role === 'admin'
}

export const STAFF_ROLES = ['moderator', 'admin']
