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

export interface Vec3 {
	x: number;
	y: number;
	z: number;
}

export interface Quat {
	x: number;
	y: number;
	z: number;
	w: number;
}

export interface BlueprintStructureGeometry {
	map_object_id: string;
	translation: Vec3;
	rotation: Quat;
	scale: Vec3;
}

export interface BlueprintGeometry {
	structures: BlueprintStructureGeometry[];
}

export interface PlacementAnchor {
	x: number;
	y: number;
	z: number;
	yaw: number;
}

export interface BlueprintFinding {
	severity: string;
	code: string;
	message: string;
}

export interface ValidatePlacementResponse {
	findings: BlueprintFinding[];
	has_blocking: boolean;
}

export interface PlaceBlueprintResponse {
	base_id: string | null;
	structures_placed: number;
	findings: BlueprintFinding[];
}
