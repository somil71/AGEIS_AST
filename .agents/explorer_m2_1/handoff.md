# Milestone 2: UI Additions & Polish - Analysis & Implementation Plan

## 1. Observation
- The frontend source code for the Needle graph visualizer is located in `src/assets/ui.html`.
- **Export Menu**: The UI currently has a single button for exporting SVG: `<button class="btn btn-ghost btn-sm" onclick="exportGraphSVG()">Export SVG</button>` located in `#graph-controls` under the "Actions" section (`line 3360`).
- **PR Impact Analyzer Modal**: A modal system already exists in the file (e.g., `#repo-modal` on `line 3646`). There is no existing UI for the "PR Impact Analyzer".
- **Group Hulls**: Implemented in the `_drawHulls` function (`lines 4691-4717`). The polygons are rendered using `.attr('fill', col).attr('stroke', col)` without any opacity modifiers, making them opaque "blobs" that obscure the labels underneath.
- **Loading Smoothness**: The `#graph-loading` element is abruptly shown/hidden by directly toggling `style.display='flex'` and `style.display='none'` (`lines 895, 3375, 4569`, etc.), which creates a jarring loading experience.

## 2. Logic Chain
- **Export Menu**: We need to replace the single "Export SVG" button with a custom HTML/CSS dropdown. It should match the dark aesthetic and include the requested options: Block Diagram, Copy Mermaid, Diagram SVG, Analysis Report, JSON Report.
- **PR Impact Analyzer Modal**: We can add a new absolute positioned modal (`#pr-impact-modal`) based on the existing modal styles. It needs a sleek dark theme (`background: #18181b`) and a vibrant green action button (`background: #22c55e`). We also need to add a button in the UI to trigger this modal.
- **Group Hulls Fix**: Modifying the D3 code in `_drawHulls` to include `.attr('fill-opacity', 0.05)` and `.attr('stroke-opacity', 0.15)` will make the convex hulls extremely transparent, preventing them from obscuring the node labels. Alternatively, we can remove the function call entirely from the layout render functions (`_renderForce`), but lowering opacity satisfies the design requirement perfectly while keeping the visual grouping.
- **Loading Smoothness**: Updating `#graph-loading` in CSS to include a background (`background: var(--bg)`), `z-index`, and a CSS `transition: opacity 0.3s ease` will allow smooth cross-fading. Replacing `style.display = 'none'` with `classList.add('hidden')` in the JS will trigger this transition, smoothing the switch between layouts.

## 3. Caveats
- The logic behind the new Export options (Block Diagram, Copy Mermaid, Analysis Report, JSON Report) and the PR Impact Analyzer is currently un-implemented stubs. This milestone explicitly states "Add the UI for a 'PR Impact Analyzer' modal", so only the UI implementation is required.
- Replacing `.style.display='none'` across all layout functions requires multiple replacements (at least 15 occurrences), but it's purely a mechanical update in the DOM manipulation logic.

## 4. Conclusion
The requested UI Additions and Polish can all be implemented within `src/assets/ui.html`. 
1. **Export Menu**: Replace the Export button at line 3360 with a `<div class="export-dropdown-container">` containing the required links.
2. **PR Impact Analyzer UI**: Append the modal HTML to the document body (near `repo-modal`) and add a trigger button to the sidebar.
3. **Group Hulls Fix**: Update `_drawHulls()` at line 4714 to apply `.attr('fill-opacity', 0.05)` and `.attr('stroke-opacity', 0.15)`.
4. **Loading Smoothness**: Update CSS for `#graph-loading` (add transition and opacity classes) and update JS display toggles to use `classList.toggle('hidden')`.

## 5. Verification Method
- **Export Menu / PR Impact Analyzer**: Open `src/assets/ui.html` in a browser. Ensure the Export dropdown appears under "Actions" in the left sidebar and that the PR Impact Analyzer modal can be opened and visually matches the requested aesthetic.
- **Group Hulls**: Select the "Force" layout with "Group Hulls" enabled. The hulls should now be mostly transparent.
- **Loading Smoothness**: Toggle between "Force" and "Radial" views. The loading spinner should fade smoothly instead of abruptly flashing on screen.
