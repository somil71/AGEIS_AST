# Handoff Report: D3 Layouts

## Observation
1. **Flow Layout (Sankey)**: In `d:\NEEDLE\src\assets\ui.html` (`_renderFlow()`, line 5190), a Sankey diagram is drawn but `nodeWidth` is hardcoded to 15, which makes nodes thin rather than the requested "thick, distinctly colored vertical bars". Node labels use standard font instead of monospace. 
2. **Bundle Layout**: In `_renderBundle()` (line 5113), the layout arranges nodes in a circle and draws quadratic bezier paths through the center `(cx, cy)`.
3. **Tree Layout**: In `_renderTree()` (line 5381), `d3.tree()` builds a horizontal tree but there is no interactivity (e.g., node `on('click')` events to toggle `d.children` and `d._children`), meaning it is not collapsible as requested.
4. **Treemap Layout**: The UI has both a "Treemap" button (`_renderTreemap()`, line 4972) which uses `d3.treemap()`, and a "Block" button (`_renderBlock()`, line 5433). `_renderBlock()` incorrectly implements a circle packing layout using `d3.pack()` instead of the required "strict rectangular Treemap layout".
5. **UI Clutter (Hulls)**: `_drawHulls()` (line 4691) and CSS `.hull-polygon` (line 1092) draw convex hulls behind nodes, but their fill/stroke settings create a "blob" effect that can still visually conflict with edge/node clarity. 
6. **Loading UX**: Layout computations (e.g., `_renderFlow()`) run synchronously without yielding to the browser, blocking the main thread and preventing the `#graph-loading` spinner from rendering during the transition.
7. **Missing UI Elements**: The "Export Options" dropdown and "PR Impact Analyzer" modal (dark aesthetic, green button) are missing entirely from the graph controls section (around line 3357).

## Logic Chain
1. The **Sankey layout** requires thicker nodes and stylized ribbons. Modifying `nodeWidth(30)` and adjusting text styles will satisfy R1.
2. The **Tree layout** must be collapsible to meet R3. This requires adding a recursive click handler to toggle children and calling the update function dynamically.
3. The **Block layout** must match R3's strict rectangular Treemap constraints. `_renderBlock()` should either be removed in favor of `_renderTreemap()` or rewritten to use `d3.treemap()` instead of `d3.pack()`.
4. The **"Group Hulls" blob** should have its opacity drastically reduced or the feature removed entirely to satisfy R5.
5. The **loading experience** (R5) can be made smooth by wrapping the heavy layout computations inside a `setTimeout` or `requestAnimationFrame`, allowing the DOM to paint the spinner first.
6. The **missing UI components** (R4) must be injected into the `#graph-right-col` or the actions bar.

## Caveats
- I did not test the current state in a browser, so exact styling of "smooth, semi-transparent flowing ribbons" may require fine-tuning CSS.
- I assumed `ui.html` is the only place rendering these graphs. No React/Vue components exist since the app uses vanilla JS/D3.

## Conclusion
The D3 layouts need significant JS and CSS adjustments in `src/assets/ui.html`. The Sankey nodes need widening; the Tree layout needs collapse logic; the Block layout must be converted from `d3.pack()` to `d3.treemap()`; the Hull polygons need opacity reduction (or removal); and the new Export/PR-Impact UI components must be added. Yielding the event loop before heavy D3 computations will fix the loading jitter.

## Verification Method
- After applying fixes, open `d:\NEEDLE\src\assets\ui.html` (or run the app) in a browser.
- Click "Flow" and verify thick vertical bars appear.
- Click "Tree" and confirm clicking nodes collapses/expands children.
- Click "Block" and verify nested rectangular blocks (not circles) appear.
- Verify the "Export Options" dropdown and "PR Impact Analyzer" modal exist.
- Switch views and verify the loading spinner renders smoothly.
