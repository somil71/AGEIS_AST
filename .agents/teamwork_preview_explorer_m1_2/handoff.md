# Handoff Report: D3 Layouts Investigation

## 1. Observation

1. **Group Hulls (Clutter)**: In `src/assets/ui.html` (line 4712), `_drawHulls()` appends a polygon for the bounding hull:
   ```javascript
   hullG.append('polygon')
     .attr('class','hull-polygon')
     .attr('points', padded.map(p=>p.join(',')).join(' '))
     .attr('fill', col).attr('stroke', col);
   ```
   There is no `fill-opacity` or `stroke-opacity` defined, causing it to render fully opaque and create an ugly "blob" that obscures the graph labels and nodes.
2. **Treemap / Block Layout**: In `_renderBlock()` (line 5433), the layout engine is `d3.pack()`, which renders nested circles (`nodeG.append('circle').attr('r', d => d.r)`). This violates the requirement for "nested rectangular blocks".
3. **Tree Layout**: In `_renderTree()` (line 5381), the layout uses `d3.tree()` but renders the entire structure statically. There is no logic for a "collapsible" structure (i.e. no click handlers to toggle between `d.children` and `d._children`).
4. **Flow Layout (Sankey)**: `_renderFlow()` (line 5190) creates nodes with a `nodeWidth(15)`, failing the requirement for "thick" vertical bars. The cycle breaker (`if (l.source > l.target) return;`) prevents circular reference errors in `d3.sankey()`, but is highly simplistic.
5. **Missing UI Components**: Searches for "Export Options" and "Impact Analyzer" in `ui.html` return zero results. The dropdown and modal do not currently exist in the codebase.

## 2. Logic Chain

- The visual clutter from the "Group Hulls" is directly caused by the missing opacity styles. Adding `.attr('fill-opacity', 0.05).attr('stroke-opacity', 0.15)` will push the hull behind the nodes and text visually.
- To satisfy the "Treemap / Block" acceptance criteria, `_renderBlock()` must be completely rewritten to use `d3.treemap()` instead of `d3.pack()`, replacing `<circle>` appends with `<rect>` appends (similar to what is partially implemented in `_renderTreemap`).
- To make the "Tree" layout collapsible, it must be updated to store collapsed children in a `_children` property, coupled with a `click` event listener on nodes that swaps `d.children` and `d._children` and re-runs the D3 tree update cycle.
- For the "Flow" (Sankey) diagram, adjusting the configuration to `sankey().nodeWidth(30+)` and ensuring the color logic matches the nodes correctly will satisfy the "thick vertical bars" requirement.
- The missing "Export Options" and "PR Impact Analyzer" components must be explicitly implemented by adding the corresponding HTML to the toolbar area and a `<dialog>`/modal for the Analyzer, along with styling for the "dark aesthetic and vibrant green action button."

## 3. Caveats

- I am unable to visually preview the UI, so layout parameter tuning (e.g. `fill-opacity` value, Sankey `nodeWidth`) might need further adjustment by the implementer.
- The naive cycle breaker in Sankey (`l.source > l.target`) breaks cycles but might drop too many valid edges depending on node indexing order. A proper DAG resolution (e.g. DFS cycle removal) is recommended if the visual flow looks incomplete.

## 4. Conclusion

The layouts are functional but fail specific architectural and design requirements. The fix strategy is purely frontend (modifying `src/assets/ui.html`):
1. Add opacity attributes to the hull polygons.
2. Replace `d3.pack()` with `d3.treemap()` in `_renderBlock()`.
3. Add interaction state (`_children`) to `_renderTree()` to make it collapsible.
4. Increase node width in the Sankey layout.
5. Inject the missing Export Menu and PR Impact Analyzer modal HTML/CSS.

## 5. Verification Method

1. Run the application backend (e.g., `cargo run`).
2. Open the Web UI in a browser.
3. **Group Hulls**: Enable the "Group Hulls" checkbox and verify polygons are semi-transparent and do not obscure text.
4. **Block View**: Click the "Block" layout button and verify nested rectangles are drawn instead of circles.
5. **Tree View**: Click the "Tree" layout, and click on nodes to verify they collapse and expand dynamically.
6. **UI Polish**: Verify the "Export Options" dropdown is in the DOM and the "PR Impact Analyzer" opens a dark-themed modal with a green button.
