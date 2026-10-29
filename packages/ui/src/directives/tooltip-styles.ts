// The tooltip's stylesheet is a side effect, so it must not ride along with the
// directive itself: the directive is exported from the package barrel, and an ESM
// star re-export keeps side-effect imports, which would pull this CSS into every
// consumer's module graph -- including pages that never render a tooltip.
//
// Import this module once, next to wherever the directive is registered.
import '../styles/overlays.css'
