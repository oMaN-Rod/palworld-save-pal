// True only in the browser-only build (worker transport + in-memory stub DB).
// The desktop app and the Docker/server deployment both use the WebSocket
// transport against a real backend, so DB-backed and native-only features work
// there and must stay enabled.
export const isWebBuild = import.meta.env.VITE_TRANSPORT === 'worker';
