---
name: figma-mcp
description: Use the Figma MCP server to translate a Figma design into a Vue page or component layout. Use when the user provides a Figma URL, asks to implement a design, or wants to draft a page layout from Figma.
---

Read the [Figma standard](../../../standards/frontend/FIGMA_MCP_USAGE.md), [shared UI instructions](../../../packages/ui/AGENTS.md) and [surface standard](../../../standards/frontend/SURFACE_SYSTEM.md). Verify the Figma tools are connected before fetching design data. If unavailable, report that dependency and continue only with references the user has actually supplied. The input is the Figma URL in the user request.

## Steps

1. **Parse the Figma URL** from the user-supplied URL — extract the `fileKey` and `nodeId`. Convert `-` to `:` in the node ID.
2. **Read the standards above** for tool availability and adaptation rules before calling Figma tools.
3. **Call `get_design_context`** with the extracted `nodeId` and `fileKey`, using `clientLanguages: "typescript,html,css"` and `clientFrameworks: "vue"`. This is the first Figma design tool call.
4. **Adapt the output to the Axolotl codebase:**
   - Map Figma color variables to `surface-*` / `text-*` tokens — never use Figma's aliased names directly.
   - Check `packages/ui/src/components/` for existing components that match elements in the design (buttons, cards, modals, inputs, etc.).
   - Check `packages/assets/styles/variables.scss` for tokens not exposed in Figma.
   - Match spacing values exactly from the design.
5. **Use `get_screenshot`** if you need a closer visual reference of specific nodes.
6. **Use `get_variable_defs`** to verify which design tokens are applied to ambiguous elements.
7. **Build the component** as a Vue SFC using Tailwind classes and the project's existing component library.
