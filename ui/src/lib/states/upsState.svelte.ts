import {
	type UPSCollection,
	type UPSFilters,
	type UPSPagination,
	type UPSPal,
	type UPSSortBy,
	type UPSSortOrder,
	type UPSStats,
	type UPSTag,
	MessageType
} from '$lib/types';
import { isReady, send } from '$lib/utils/websocketUtils';

export interface UPSState {
	pals: UPSPal[];
	collections: UPSCollection[];
	tags: UPSTag[];
	stats: UPSStats | null;
	filters: UPSFilters;
	pagination: UPSPagination;
	loading: boolean;
	selectedPals: Set<number>;
	viewMode: 'grid' | 'list';
	showCollectionsPanel: boolean;
	showTagsPanel: boolean;
	showStatsPanel: boolean;
}

const DEFAULT_FILTERS: UPSFilters = {
	search: '',
	characterId: 'All',
	collectionId: undefined,
	tags: [],
	sortBy: 'created_at',
	sortOrder: 'desc'
};

const DEFAULT_PAGINATION: UPSPagination = {
	page: 1,
	limit: 30,
	totalCount: 0,
	totalPages: 0
};

class UPSStateClass {
	pals = $state<UPSPal[]>([]);
	collections = $state<UPSCollection[]>([]);
	tags = $state<UPSTag[]>([]);
	stats = $state<UPSStats | null>(null);
	filters = $state<UPSFilters>({ ...DEFAULT_FILTERS });
	pagination = $state<UPSPagination>({ ...DEFAULT_PAGINATION });
	loading = $state(false);
	selectedPals = $state<Set<number>>(new Set());
	viewMode = $state<'grid' | 'list'>('grid');
	showCollectionsPanel = $state(true);
	showTagsPanel = $state(false);
	showStatsPanel = $state(false);

	get hasSelectedPals(): boolean {
		return this.selectedPals.size > 0;
	}

	get filteredCollections(): UPSCollection[] {
		return this.collections.filter((c) => !c.is_archived);
	}

	get favoriteCollections(): UPSCollection[] {
		return this.collections.filter((c) => c.is_favorite && !c.is_archived);
	}

	get availableCharacterIds(): string[] {
		const characterIds = new Set(this.pals.map((p) => p.character_id));
		return Array.from(characterIds).sort();
	}

	get availableTags(): UPSTag[] {
		return this.tags.sort((a, b) => b.usage_count - a.usage_count);
	}

	async loadPals(refresh: boolean = false): Promise<void> {
		if (!refresh && this.loading) return;

		this.loading = true;
		try {
			const offset = (this.pagination.page - 1) * this.pagination.limit;

			await send(MessageType.GET_UPS_PALS, {
				offset,
				limit: this.pagination.limit,
				search_query: this.filters.search || undefined,
				character_id_filter:
					this.filters.characterId !== 'All' ? this.filters.characterId : undefined,
				collection_id: this.filters.collectionId,
				tags: this.filters.tags.length > 0 ? this.filters.tags : undefined,
				sort_by: this.filters.sortBy,
				sort_order: this.filters.sortOrder
			});
		} catch (error) {
			console.error('Error loading UPS pals:', error);
		} finally {
			this.loading = false;
		}
	}

	async loadCollections(): Promise<void> {
		try {
			await send(MessageType.GET_UPS_COLLECTIONS);
		} catch (error) {
			console.error('Error loading UPS collections:', error);
		}
	}

	async loadTags(): Promise<void> {
		try {
			await send(MessageType.GET_UPS_TAGS);
		} catch (error) {
			console.error('Error loading UPS tags:', error);
		}
	}

	async loadStats(): Promise<void> {
		try {
			await send(MessageType.GET_UPS_STATS);
		} catch (error) {
			console.error('Error loading UPS stats:', error);
		}
	}

	async loadAll(): Promise<void> {
		if (!isReady()) {
			console.warn('WebSocket not ready, cannot load UPS data.');
			return;
		}
		await Promise.all([this.loadPals(), this.loadCollections(), this.loadTags(), this.loadStats()]);
	}

	async createCollection(name: string, description?: string, color?: string): Promise<void> {
		try {
			await send(MessageType.CREATE_UPS_COLLECTION, {
				name,
				description,
				color
			});
			await this.loadCollections();
		} catch (error) {
			console.error('Error creating collection:', error);
		}
	}

	async updateCollection(
		collectionId: number,
		updates: Partial<
			Pick<UPSCollection, 'name' | 'description' | 'color' | 'is_favorite' | 'is_archived'>
		>
	): Promise<void> {
		try {
			await send(MessageType.UPDATE_UPS_COLLECTION, {
				collection_id: collectionId,
				updates
			});

			const index = this.collections.findIndex((c) => c.id === collectionId);
			if (index >= 0) {
				Object.assign(this.collections[index], updates);
			}
		} catch (error) {
			console.error('Error updating collection:', error);
		}
	}

	async deleteCollection(collectionId: number): Promise<void> {
		try {
			await send(MessageType.DELETE_UPS_COLLECTION, {
				collection_id: collectionId
			});

			this.collections = this.collections.filter((c) => c.id !== collectionId);

			if (this.filters.collectionId === collectionId) {
				this.filters.collectionId = undefined;
				await this.loadPals(true);
			}
		} catch (error) {
			console.error('Error deleting collection:', error);
		}
	}

	async createTag(name: string, description?: string, color?: string): Promise<void> {
		try {
			await send(MessageType.CREATE_UPS_TAG, {
				name,
				description,
				color
			});

			await this.loadTags();
		} catch (error) {
			console.error('Error creating tag:', error);
		}
	}

	async updatePal(
		palId: number,
		updates: Partial<Pick<UPSPal, 'nickname' | 'collection_id' | 'tags' | 'notes'>>
	): Promise<void> {
		try {
			await send(MessageType.UPDATE_UPS_PAL, {
				pal_id: palId,
				updates
			});

			const index = this.pals.findIndex((p) => p.id === palId);
			if (index >= 0) {
				Object.assign(this.pals[index], updates);
			}

			// Refresh collections if collection assignment changed
			if ('collection_id' in updates) {
				await this.loadCollections();
			}
		} catch (error) {
			console.error('Error updating pal:', error);
		}
	}

	async clonePal(palId: number): Promise<void> {
		try {
			await send(MessageType.CLONE_UPS_PAL, {
				pal_id: palId
			});
			// Refresh pals list and collections after cloning
			await this.loadPals(true);
			await this.loadCollections();
		} catch (error) {
			console.error('Error cloning pal:', error);
		}
	}

	async deleteSelectedPals(): Promise<void> {
		if (this.selectedPals.size === 0) return;

		try {
			await send(MessageType.DELETE_UPS_PALS, {
				pal_ids: Array.from(this.selectedPals)
			});

			this.selectedPals.clear();
			await this.loadPals(true);
			await this.loadCollections(); // Update collections to refresh pal_count
			await this.loadStats(); // Update stats after deletion
		} catch (error) {
			console.error('Error deleting pals:', error);
		}
	}

	async deletePals(palIds: number[]): Promise<void> {
		try {
			await send(MessageType.DELETE_UPS_PALS, {
				pal_ids: palIds
			});

			palIds.forEach((id) => this.selectedPals.delete(id));

			await this.loadPals(true);
			await this.loadCollections(); // Update collections to refresh pal_count
			await this.loadStats(); // Update stats after deletion
		} catch (error) {
			console.error('Error deleting pals:', error);
		}
	}

	async exportPal(
		palId: number,
		destinationType: 'pal_box' | 'gps' | 'dps',
		destinationPlayerUid?: string,
		destinationSlot?: number
	): Promise<void> {
		try {
			await send(MessageType.EXPORT_UPS_PAL, {
				pal_id: palId,
				destination_type: destinationType,
				destination_player_uid: destinationPlayerUid,
				destination_slot: destinationSlot
			});

			await this.loadStats();
		} catch (error) {
			console.error('Error exporting pal:', error);
		}
	}

	async importFromSave(
		sourceType: 'pal_box' | 'gps' | 'dps',
		sourcePalId?: string,
		sourceSlot?: number,
		sourcePlayerUid?: string,
		collectionId?: number,
		tags?: string[],
		notes?: string
	): Promise<void> {
		try {
			await send(MessageType.IMPORT_TO_UPS, {
				source_type: sourceType,
				source_pal_id: sourcePalId,
				source_slot: sourceSlot,
				source_player_uid: sourcePlayerUid,
				collection_id: collectionId,
				tags: tags,
				notes: notes
			});

			await this.loadPals(true);
			await this.loadCollections(); // Update collections to refresh pal_count
			await this.loadStats();
		} catch (error) {
			console.error('Error importing pal:', error);
		}
	}

	async cloneToUps(
		palIds: string[],
		sourceType: 'pal_box' | 'gps' | 'dps',
		sourcePlayerUid?: string,
		collectionId?: number,
		tags?: string[],
		notes?: string
	): Promise<void> {
		try {
			await send(MessageType.CLONE_TO_UPS, {
				pal_ids: palIds,
				source_type: sourceType,
				source_player_uid: sourcePlayerUid,
				collection_id: collectionId,
				tags: tags,
				notes: notes
			});

			await this.loadPals(true);
			await this.loadCollections(); // Update collections to refresh pal_count
			await this.loadStats();
		} catch (error) {
			console.error('Error cloning pals to UPS:', error);
		}
	}

	togglePalSelection(palId: number): void {
		if (this.selectedPals.has(palId)) {
			this.selectedPals.delete(palId);
		} else {
			this.selectedPals.add(palId);
		}
		this.selectedPals = new Set(this.selectedPals);
	}

	selectAllPals(): void {
		this.pals.forEach((pal) => this.selectedPals.add(pal.id));
		this.selectedPals = new Set(this.selectedPals);
	}

	clearSelection(): void {
		this.selectedPals.clear();
	}

	updateSearch(search: string): void {
		this.filters.search = search;
		this.pagination.page = 1;
	}

	updateCharacterFilter(characterId: string): void {
		this.filters.characterId = characterId;
		this.pagination.page = 1;
	}

	updateCollectionFilter(collectionId?: number): void {
		this.filters.collectionId = collectionId;
		this.pagination.page = 1;
	}

	updateTagFilter(tags: string[]): void {
		this.filters.tags = tags;
		this.pagination.page = 1;
	}

	updateSort(sortBy: UPSSortBy, sortOrder: UPSSortOrder): void {
		this.filters.sortBy = sortBy;
		this.filters.sortOrder = sortOrder;
		this.pagination.page = 1;
	}

	clearFilters(): void {
		this.filters = { ...DEFAULT_FILTERS };
		this.pagination.page = 1;
	}

	setPage(page: number): void {
		if (page >= 1 && page <= this.pagination.totalPages) {
			this.pagination.page = page;
		}
	}

	nextPage(): void {
		this.setPage(this.pagination.page + 1);
	}

	prevPage(): void {
		this.setPage(this.pagination.page - 1);
	}

	setPageSize(limit: number): void {
		this.pagination.limit = limit;
		this.pagination.page = 1;
	}

	setPalsData(pals: UPSPal[], totalCount: number, offset: number, limit: number): void {
		this.pals = pals;
		this.pagination.totalCount = totalCount;
		this.pagination.totalPages = Math.ceil(totalCount / limit);

		if (this.pagination.page > this.pagination.totalPages && this.pagination.totalPages > 0) {
			this.pagination.page = this.pagination.totalPages;
		}
	}

	setCollections(collections: UPSCollection[]): void {
		this.collections = collections;
	}

	setTags(tags: UPSTag[]): void {
		this.tags = tags;
	}

	setStats(stats: UPSStats): void {
		this.stats = stats;
	}

	setViewMode(mode: 'grid' | 'list'): void {
		this.viewMode = mode;
	}

	toggleCollectionsPanel(): void {
		this.showCollectionsPanel = !this.showCollectionsPanel;
		this.showTagsPanel = false;
		this.showStatsPanel = false;
	}

	toggleTagsPanel(): void {
		this.showTagsPanel = !this.showTagsPanel;
		this.showCollectionsPanel = false;
		this.showStatsPanel = false;
	}

	toggleStatsPanel(): void {
		this.showStatsPanel = !this.showStatsPanel;
		this.showCollectionsPanel = false;
		this.showTagsPanel = false;
	}

	reset(): void {
		this.pals = [];
		this.collections = [];
		this.tags = [];
		this.stats = null;
		this.filters = { ...DEFAULT_FILTERS };
		this.pagination = { ...DEFAULT_PAGINATION };
		this.selectedPals.clear();
		this.loading = false;
	}
}

export const upsState = new UPSStateClass();

export function getUpsState() {
	return upsState;
}
