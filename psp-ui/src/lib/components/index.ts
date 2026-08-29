// Barrel of the light component domains only. Heavy leaves are deliberately
// excluded so they stay out of the root layout chunk:
// - Map (maplibre-gl): dynamic-imported by routes/map/+page.svelte
// - PalEditModal (three.js): dynamic-imported by PalEditorOverlay.svelte
// - PalModelViewer (three.js): deep-imported where used
// - LabResearch/LabResearchControls/MissionDetails/MissionList: use the
//   '$components/guilds' / '$components/missions' barrels directly.
export * from './gamepass';
export * from './layout';
export * from './modals';
export * from './pal';
export * from './player';
export * from './presets';
export * from './shared';
