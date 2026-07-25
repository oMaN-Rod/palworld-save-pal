declare const initWasm: () => Promise<void>;
export default initWasm;
export function init(): void;
export function set_emit_callback(cb: (frame: string) => void): void;
export function init_game_data(entries: [string, string][]): void;
export function dispatch_frame(frame: string): Promise<void>;
export function set_sql_bridge(
	exec: (sql: string, params: unknown[]) => number,
	query: (sql: string, params: unknown[]) => Record<string, unknown>[]
): void;
export function run_migrations(): Promise<void>;
