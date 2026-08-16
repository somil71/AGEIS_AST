# Original User Request

## Initial Request — 2026-07-10T14:43:57Z

Redesign and fix the D3.js architecture visualization layouts in the Needle UI to perfectly match the provided reference designs (Sankey Flow, Radial Bundle, Horizontal Tree, Square Treemap, and an Export menu), while removing visual clutter like the opaque convex hulls ("blob") and improving loading smoothness.

Working directory: D:\NEEDLE
Integrity mode: development

## Requirements

### R1. Flow Layout (Sankey)
Implement a beautiful `d3-sankey` diagram representing dependencies between folders/modules. Nodes should be thick, distinctly colored vertical bars with clear monospaced labels (e.g., `folder (count)`), and links should be smooth, semi-transparent flowing ribbons.

### R2. Bundle Layout (Hierarchical Edge Bundling)
Implement a radial bundle layout where files/modules are arranged in a large circle. Dependencies between them should be drawn as curved spline paths cutting through the center of the circle.

### R3. Tree & Treemap Layouts
- **Tree**: Implement a clean, horizontal collapsible tree structure starting from a root node and expanding outwards to files.
- **Treemap (Block)**: Implement a strict rectangular Treemap layout (`d3.treemap`) where folders and files are represented as sized, colored nested rectangles.

### R4. Export Menu & PR Impact Analyzer
- Add an "Export Options" dropdown menu matching the reference design (Block Diagram, Copy Mermaid, Diagram SVG, Analysis Report, JSON Report).
- Add the UI for a "PR Impact Analyzer" modal with a sleek, dark aesthetic and a vibrant green action button.

### R5. Visual Polish & Clutter Reduction
- Fix the "Group Hulls" feature: either remove it or ensure the polygons are drawn completely behind the nodes/text with very low opacity so they do not obscure labels or create an ugly "blob".
- Improve the loading experience (spinner/transitions) so switching between views feels smooth rather than jarring or cluttered.

## Acceptance Criteria

### Visualization Verification
- [ ] Clicking the "Flow" view renders a proper `d3-sankey` diagram without JavaScript errors.
- [ ] Clicking the "Bundle" view renders a circular node arrangement with inner curved links.
- [ ] Clicking the "Tree" view renders a horizontal tree structure.
- [ ] Clicking the "Treemap" or "Block" view renders nested rectangular blocks.
- [ ] All graph layouts dynamically scale to fit the canvas and do not get permanently hidden behind loading spinners.

### UI Polish
- [ ] The "Group Hulls" option no longer obscures node text.
- [ ] The "Export" menu and "PR Impact Analyzer" UI components are present in the DOM and styled to match the dark/premium aesthetic.
