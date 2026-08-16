## Iteration 2 Synthesis (Fixing Auditor/Challenger Failures)
1. **PR Impact Analyzer UI**: Replace the `alert()` dummy with a real async function `runPRImpactAnalysis()` that simulates network delay and updates a `#pr-impact-result` `div`. Add input field for PR URL.
2. **Export Menu**: Add real JavaScript handlers: `exportBlockDiagram()`, `exportCopyMermaid()`, `exportAnalysisReport()`, and `exportJSONReport()`.
3. **Group Hulls**: Handle 2-node hulls. If `pts.length === 2`, construct a 4-point bounding box with padding around the 2 points instead of calling `d3.polygonHull`.
4. **Export SVG Colors**: The `exportGraphSVG()` function must query all `<style>` elements and embed their content into a `<style>` block within the exported SVG clone, so CSS variables (like `--bg`) work.
5. **Loading Smoothness (`display: none`)**: Replace all JavaScript assignments of `style.display = 'none'` or `'flex'` on `#graph-loading` with `style.opacity = '0'; style.visibility = 'hidden'` and `style.opacity = '1'; style.visibility = 'visible'`, respectively. Make sure CSS has transitions for opacity and visibility.

Implement these precisely in `src/assets/ui.html`.
