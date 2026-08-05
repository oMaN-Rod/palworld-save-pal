QOL pass + dendrogram PNG export for the breeding calculator.

## Part A — Dendrogram PNG export (feature)

New util `ui/src/lib/breeding/dendrogram/exportPng.ts` (no new deps; canvas + ClipboardItem):
- `exportTreeToPng(svgEl, {scale=2, margin=32})` → PNG Blob: measure the live `g.dendro-zoom-layer` natural bbox (ignores current zoom/pan → full tree auto-fitted), deep-clone the SVG, inline pal `<image href>` assets as data URLs (same-origin, no taint) and await decode, drop the d3-zoom transform, wrap content in a translate group mapping bbox+margin into the canvas, inject a `DENDRO_COLORS.bgCard` background rect, serialize → SVG blob URL → `Image` → canvas at bbox×scale → `toBlob('image/png')`.
- `downloadPng(blob, filename)` — repo's existing `a[download]`+revoke pattern.
- `copyPngToClipboard(blob)` — `ClipboardItem` with feature-detect, returns success bool.
- Pure helpers (`computeCanvasTransform(bbox, margin, scale)`, filename `slugify`) exported for unit tests.

Wire-up in `ChainDendrogram.svelte` toolbar (next to zoom buttons): Download PNG (`download` icon) + Copy PNG (`copy`→`check` on success). Disabled while exporting; toasts via `getToastState()`; filename `{target-slug}-dendrogram.png`. Works for chain AND direct trees in Graph mode (shared component).

i18n: new flat `breeding_*` keys in `data/json/ui/en.json` (`breeding_export_png`, `breeding_copy_png`, `breeding_png_copied`, `breeding_png_copy_failed`, `breeding_export_failed`); paraglide regenerates on dev/build.

## Part B — QOL / UI polish

1. i18n ~30 hardcoded strings: side panel (Controls/Cfg/Direct/Parent A/B/Target/Compute/Any/M/F/Remove/Expand/Collapse), ChainCard (Owned/Selected/Wild, "already available" note), ChainTooltip (Step/Bred/Target), DirectResult (Special, gender-prob title), GraphView (Prev/Next/All/Per-Gen/Gen/No tree), PalPicker (placeholder/Search/No matches), ChainDendrogram (Zoom in/out/Fit view), page (error strings, Toggle gender, owned-pal count lines).
2. Consistent label style (`text-[10px] font-semibold text-surface-400 uppercase tracking-wider mb-1`), side-panel pills bumped 9px→10px matching page pills, passive chips unified to 9px.
3. States: direct mode clears stale results + spinner while running; list mode shows spinner (not "no chains" empty state) while computing; errors as rose alert chips in all modes; warnings keyed `#each`; OwnerSelect "No players found" empty row.
4. Layout/interaction: unify list⇄graph padding (no page jump), drop redundant toggle titles, pan via Pointer Events for touch support.

## Verification
- New vitest cases for the pure export helpers; existing suites stay green.
- `bun run check` (only pre-existing wasm `set_oodle_bridge` error) + `bun run build`.
- Manual: dev server, graph mode, verify download + clipboard paste, light-touch QOL spot checks.

No backend changes, no new dependencies. ChainCard list view not exported (not a dendrogram).