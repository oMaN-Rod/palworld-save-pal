import { getToastState, getUpsState } from '$states';
import type { UPSCollection, UPSPal, UPSPalsResponse, UPSStats, UPSTag } from '$types';
import { MessageType } from '$types';
import type { WSMessageHandler } from '$ws/types';

export const getUpsPalsHandler: WSMessageHandler = {
	type: MessageType.GET_UPS_PALS,
	async handle(data: UPSPalsResponse) {
		const upsState = getUpsState();
		upsState.setPalsData(data.pals, data.total_count, data.offset, data.limit);
	}
};

export const addUpsPalHandler: WSMessageHandler = {
	type: MessageType.ADD_UPS_PAL,
	async handle(data: { pal: UPSPal }) {
		const upsState = getUpsState();
		const toastState = getToastState();

		if (data.pal) {
			upsState.pals = [data.pal, ...upsState.pals];
			upsState.pagination.totalCount++;

			toastState.add(
				`Added ${data.pal.nickname || data.pal.character_id} to UPS`,
				'Success',
				'success'
			);
		}
	}
};

export const updateUpsPalHandler: WSMessageHandler = {
	type: MessageType.UPDATE_UPS_PAL,
	async handle(data: { pal: Partial<UPSPal> }) {
		const upsState = getUpsState();
		const toastState = getToastState();

		if (data.pal && data.pal.id) {
			const index = upsState.pals.findIndex((p) => p.id === data.pal.id);
			if (index >= 0) {
				Object.assign(upsState.pals[index], data.pal);
				toastState.add(
					`Updated ${data.pal.nickname || data.pal.character_id}`,
					'Success',
					'success'
				);
			}
		}
	}
};

export const deleteUpsPalsHandler: WSMessageHandler = {
	type: MessageType.DELETE_UPS_PALS,
	async handle(data: { deleted_count: number; requested_count: number }) {
		const upsState = getUpsState();
		const toastState = getToastState();

		if (data.deleted_count > 0) {
			await upsState.loadPals(true);

			if (data.deleted_count === data.requested_count) {
				toastState.add(
					`Deleted ${data.deleted_count} pal${data.deleted_count > 1 ? 's' : ''}`,
					'Success',
					'success'
				);
			} else {
				toastState.add(
					`Deleted ${data.deleted_count} of ${data.requested_count} requested pals`,
					'Partial',
					'warning'
				);
			}
		} else {
			toastState.add('No pals were deleted', 'Error', 'error');
		}
	}
};

export const cloneUpsPalHandler: WSMessageHandler = {
	type: MessageType.CLONE_UPS_PAL,
	async handle(data: { original_pal_id: number; cloned_pal: UPSPal }) {
		const upsState = getUpsState();
		const toastState = getToastState();

		if (data.cloned_pal) {
			upsState.pals = [data.cloned_pal, ...upsState.pals];
			upsState.pagination.totalCount++;

			toastState.add(
				`Cloned ${data.cloned_pal.nickname || data.cloned_pal.character_id}`,
				'Success',
				'success'
			);
		}
	}
};

export const exportUpsPalHandler: WSMessageHandler = {
	type: MessageType.EXPORT_UPS_PAL,
	async handle(data: {
		success: boolean;
		destination_type: string;
		destination_player_uid?: string;
		destination_slot?: number;
		error?: string;
	}) {
		const toastState = getToastState();

		if (data.success) {
			let message = `Exported pal to ${data.destination_type.toUpperCase()}`;
			if (data.destination_slot !== undefined) {
				message += ` slot ${data.destination_slot}`;
			}
			toastState.add(message, 'Success', 'success');
		} else {
			toastState.add(`Export failed: ${data.error || 'Unknown error'}`, 'Error', 'error');
		}
	}
};

export const cloneToUpsHandler: WSMessageHandler = {
	type: MessageType.CLONE_TO_UPS,
	async handle(data: {
		success: boolean;
		cloned_count: number;
		total_requested: number;
		errors?: string[];
	}) {
		const upsState = getUpsState();
		const toastState = getToastState();

		if (data.success && data.cloned_count > 0) {
			await upsState.loadPals(true);
			await upsState.loadStats();

			if (data.cloned_count === data.total_requested) {
				toastState.add(
					`Successfully cloned ${data.cloned_count} pal${data.cloned_count > 1 ? 's' : ''} to UPS`,
					'Success',
					'success'
				);
			} else {
				toastState.add(
					`Cloned ${data.cloned_count} of ${data.total_requested} pals to UPS${data.errors ? `. Errors: ${data.errors.length}` : ''}`,
					'Partial',
					'warning'
				);
			}
		} else {
			const errorMsg =
				data.errors && data.errors.length > 0
					? `Clone failed: ${data.errors[0]}${data.errors.length > 1 ? ` (and ${data.errors.length - 1} more)` : ''}`
					: 'Clone to UPS failed';
			toastState.add(errorMsg, 'Error', 'error');
		}
	}
};

export const importToUpsHandler: WSMessageHandler = {
	type: MessageType.IMPORT_TO_UPS,
	async handle(data: { success: boolean; pal?: UPSPal; error?: string }) {
		const upsState = getUpsState();
		const toastState = getToastState();

		if (data.success && data.pal) {
			upsState.pals = [data.pal, ...upsState.pals];
			upsState.pagination.totalCount++;

			toastState.add(
				`Imported ${data.pal.nickname || data.pal.character_id} to UPS`,
				'Success',
				'success'
			);
		} else {
			toastState.add(`Import failed: ${data.error || 'Unknown error'}`, 'Error', 'error');
		}
	}
};

export const getUpsCollectionsHandler: WSMessageHandler = {
	type: MessageType.GET_UPS_COLLECTIONS,
	async handle(data: { collections: UPSCollection[] }) {
		const upsState = getUpsState();
		upsState.setCollections(data.collections);
	}
};

export const createUpsCollectionHandler: WSMessageHandler = {
	type: MessageType.CREATE_UPS_COLLECTION,
	async handle(data: { collection: UPSCollection }) {
		const upsState = getUpsState();
		const toastState = getToastState();

		if (data.collection) {
			upsState.collections = [...upsState.collections, data.collection];
			toastState.add(`Created collection "${data.collection.name}"`, 'Success', 'success');
		}
	}
};

export const updateUpsCollectionHandler: WSMessageHandler = {
	type: MessageType.UPDATE_UPS_COLLECTION,
	async handle(data: { collection: UPSCollection }) {
		const upsState = getUpsState();
		const toastState = getToastState();

		if (data.collection) {
			const index = upsState.collections.findIndex((c) => c.id === data.collection.id);
			if (index >= 0) {
				upsState.collections[index] = data.collection;
				toastState.add(`Updated collection "${data.collection.name}"`, 'Success', 'success');
			}
		}
	}
};

export const deleteUpsCollectionHandler: WSMessageHandler = {
	type: MessageType.DELETE_UPS_COLLECTION,
	async handle(data: { success: boolean; collection_id: number }) {
		const upsState = getUpsState();
		const toastState = getToastState();

		if (data.success) {
			const collection = upsState.collections.find((c) => c.id === data.collection_id);
			upsState.collections = upsState.collections.filter((c) => c.id !== data.collection_id);

			if (upsState.filters.collectionId === data.collection_id) {
				upsState.filters.collectionId = undefined;
			}

			toastState.add(
				`Deleted collection ${collection ? `"${collection.name}"` : ''}`,
				'Success',
				'success'
			);
		} else {
			toastState.add('Failed to delete collection', 'Error', 'error');
		}
	}
};

export const getUpsTagsHandler: WSMessageHandler = {
	type: MessageType.GET_UPS_TAGS,
	async handle(data: { tags: UPSTag[] }) {
		const upsState = getUpsState();
		upsState.setTags(data.tags);
	}
};

export const createUpsTagHandler: WSMessageHandler = {
	type: MessageType.CREATE_UPS_TAG,
	async handle(data: { tag: UPSTag }) {
		const upsState = getUpsState();
		const toastState = getToastState();

		if (data.tag) {
			const index = upsState.tags.findIndex((t) => t.id === data.tag.id);
			if (index >= 0) {
				upsState.tags[index] = data.tag;
			} else {
				upsState.tags = [...upsState.tags, data.tag];
			}
			toastState.add(`Created tag "${data.tag.name}"`, 'Success', 'success');
		}
	}
};

export const getUpsStatsHandler: WSMessageHandler = {
	type: MessageType.GET_UPS_STATS,
	async handle(data: { stats: UPSStats }) {
		const upsState = getUpsState();
		upsState.setStats(data.stats);
	}
};

export const upsHandlers = [
	getUpsPalsHandler,
	addUpsPalHandler,
	updateUpsPalHandler,
	deleteUpsPalsHandler,
	cloneUpsPalHandler,
	cloneToUpsHandler,
	exportUpsPalHandler,
	importToUpsHandler,
	getUpsCollectionsHandler,
	createUpsCollectionHandler,
	updateUpsCollectionHandler,
	deleteUpsCollectionHandler,
	getUpsTagsHandler,
	createUpsTagHandler,
	getUpsStatsHandler
];
