export interface UPSPal {
	id: number;
	instance_id: string;
	character_id: string;
	character_key: string;
	nickname?: string;
	level: number;
	pal_data: Record<string, any>;
	source_save_file?: string;
	source_player_uid?: string;
	source_player_name?: string;
	source_storage_type?: string;
	source_storage_slot?: number;
	collection_id?: number;
	tags: string[];
	notes?: string;
	created_at: string;
	updated_at: string;
	last_accessed_at?: string;
	transfer_count: number;
	clone_count: number;
}

export interface UPSCollection {
	id: number;
	name: string;
	description?: string;
	color?: string;
	icon?: string;
	is_favorite: boolean;
	is_archived: boolean;
	pal_count: number;
	created_at: string;
	updated_at: string;
}

export interface UPSTag {
	id: number;
	name: string;
	description?: string;
	color?: string;
	usage_count: number;
	created_at: string;
	updated_at: string;
}

export interface UPSStats {
	total_pals: number;
	total_collections: number;
	total_tags: number;
	total_transfers: number;
	total_clones: number;
	storage_size_mb: number;
	most_transferred_pal_id?: number;
	most_cloned_pal_id?: number;
	most_popular_character_id?: string;
	element_distribution: string;
	alpha_count: number;
	lucky_count: number;
	human_count: number;
	predator_count: number;
	oilrig_count: number;
	summon_count: number;
	last_updated: string;
}

export interface UPSPalsResponse {
	pals: UPSPal[];
	total_count: number;
	offset: number;
	limit: number;
}

export interface UPSGetPalsRequest {
	offset?: number;
	limit?: number;
	search_query?: string;
	character_id_filter?: string;
	collection_id?: number;
	tags?: string[];
	element_types?: string[];
	pal_types?: string[];
	sort_by?: UPSSortBy;
	sort_order?: UPSSortOrder;
}

export type UPSSortBy =
	| 'created_at'
	| 'updated_at'
	| 'character_id'
	| 'nickname'
	| 'level'
	| 'transfer_count'
	| 'clone_count';
export type UPSSortOrder = 'asc' | 'desc';
export type UPSStorageType = 'pal_box' | 'gps' | 'dps' | 'ups';

export interface UPSAddPalRequest {
	pal_dto: Record<string, any>;
	source_save_file?: string;
	source_player_uid?: string;
	source_player_name?: string;
	source_storage_type?: UPSStorageType;
	source_storage_slot?: number;
	collection_id?: number;
	tags?: string[];
	notes?: string;
}

export interface UPSUpdatePalRequest {
	pal_id: number;
	updates: Partial<Pick<UPSPal, 'nickname' | 'collection_id' | 'tags' | 'notes'>>;
}

export interface UPSDeletePalsRequest {
	pal_ids: number[];
}

export interface UPSClonePalRequest {
	pal_id: number;
}

export interface UPSExportPalRequest {
	pal_id: number;
	destination_type: UPSStorageType;
	destination_player_uid?: string;
	destination_slot?: number;
}

export interface UPSImportRequest {
	source_type: UPSStorageType;
	source_pal_id?: string;
	source_slot?: number;
	source_player_uid?: string;
	collection_id?: number;
	tags?: string[];
	notes?: string;
}

export interface UPSCreateCollectionRequest {
	name: string;
	description?: string;
	color?: string;
}

export interface UPSUpdateCollectionRequest {
	collection_id: number;
	updates: Partial<
		Pick<UPSCollection, 'name' | 'description' | 'color' | 'is_favorite' | 'is_archived'>
	>;
}

export interface UPSDeleteCollectionRequest {
	collection_id: number;
}

export interface UPSCreateTagRequest {
	name: string;
	description?: string;
	color?: string;
}

export interface UPSFilters {
	search: string;
	characterId: string;
	collectionId?: number;
	tags: string[];
	elementTypes: string[];
	palTypes: string[];
	sortBy: UPSSortBy;
	sortOrder: UPSSortOrder;
}

export interface UPSPagination {
	page: number;
	limit: number;
	totalCount: number;
	totalPages: number;
}

export type UPSContextAction =
	| 'view'
	| 'edit'
	| 'clone'
	| 'export_to_pal_box'
	| 'export_to_gps'
	| 'export_to_dps'
	| 'add_to_collection'
	| 'add_tags'
	| 'delete';
