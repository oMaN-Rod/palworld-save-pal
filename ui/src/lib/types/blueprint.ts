export interface CaptureOptions {
	production_config: boolean;
	structure_condition: boolean;
	container_contents: boolean;
	worker_pals: boolean;
	housed_pals: boolean;
	production_progress: boolean;
	access_config: boolean;
	base_identity: boolean;
}

export interface BlueprintHeader {
	schema_version: number;
	game_data_version: string;
	uesave_struct_version: string;
	manifest: CaptureOptions;
	name: string;
	source_world: string;
	source_base: string;
	created_at: number;
	structure_count: number;
	footprint_radius: number;
	anchor_height_above_terrain: number;
}

export interface BlueprintRow {
	id: string;
	name: string;
	source_world: string;
	source_base: string;
	created_at: number;
	schema_version: number;
	structure_count: number;
	manifest: string;
	footprint_radius: number;
}

export interface CaptureBlueprintResponse {
	handle: string;
	header: BlueprintHeader;
}
