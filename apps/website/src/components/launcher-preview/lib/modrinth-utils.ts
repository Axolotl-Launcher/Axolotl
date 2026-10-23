/**
 * The `@modrinth/utils` helpers the shared content layout uses. The site does
 * not list the package; it is a source package of the workspace, so the preview
 * reads the two modules by path, as `server-core.ts` does for `@modrinth/server`.
 */
export {
    formatVersionsForDisplay,
    type GameVersionTag,
} from '../../../../../../packages/utils/projects.ts'
export { capitalizeString } from '../../../../../../packages/utils/utils.ts'
