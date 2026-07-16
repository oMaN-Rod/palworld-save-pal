import { m } from '$i18n/messages';
import { getToastState, getUpsState } from '$states';
import type { UPSCollection, UPSPal, UPSPalsResponse, UPSStats, UPSTag } from '$types';
import { MessageType } from '$types';
import { c } from '$utils/commonTranslations';
import type { WSMessageHandler } from '$ws/types';

export const getUpsPalsHandler: WSMessageHandler = {
	type: MessageType.GET_UPS_PALS,
	async handle(data: UPSPalsResponse) {
		const upsState = getUpsState();
		upsState.setPalsData(data.pals, data.total_count, data.offset, data.limit);
	}
};

export const getUpsAllFilteredIdsHandler: WSMessageHandler = {
	type: MessageType.GET_UPS_ALL_FILTERED_IDS,
	async handle(data: { pal_ids: number[]; total_count: number }) {
		const upsState = getUpsState();
		// Select all the returned pal IDs
		data.pal_ids.forEach((id) => upsState.selectedPals.add(id));
		upsState.selectedPals = new Set(upsState.selectedPals);
	}
};

export const addUpsPalHandler: WSMessageHandler = {
	type: MessageType.ADD_UPS_PAL,
	async handle(data: { pal: UPSPal }) {
		const upsState = getUpsState();

		if (data.pal) {
			upsState.pals = [data.pal, ...upsState.pals];
			upsState.pagination.totalCount++;

			// Refresh collections to update pal_count
			await upsState.loadCollections();
		}
	}
};

export const updateUpsPalHandler: WSMessageHandler = {
	type: MessageType.UPDATE_UPS_PAL,
	async handle(data: { pal: Partial<UPSPal> }) {
		const upsState = getUpsState();

		if (data.pal && data.pal.id) {
			const index = upsState.pals.findIndex((p) => p.id === data.pal.id);
			if (index >= 0) {
				// Check if collection_id was changed
				const collectionChanged =
					'collection_id' in data.pal &&
					data.pal.collection_id !== upsState.pals[index].collection_id;

				// Update the pal in place
				Object.assign(upsState.pals[index], data.pal);

				// Refresh collections if collection assignment changed
				if (collectionChanged) {
					await upsState.loadCollections();
				}
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
			// Refresh the pals list and collections
			await upsState.loadPals(true);
			await upsState.loadCollections();

			if (data.deleted_count === data.requested_count) {
				toastState.add(
					m.deleted_entity({
						count: data.deleted_count,
						entity: m.pal({ count: data.deleted_count })
					}),
					m.success(),
					'success'
				);
			} else {
				toastState.add(
					m.deleted_partial({
						count: data.deleted_count,
						total: data.requested_count,
						pals: m.pal({ count: data.deleted_count })
					}),
					m.warning(),
					'warning'
				);
			}
		} else {
			toastState.add(m.no_pals_deleted({ pals: c.pals }), m.error(), 'error');
		}
	}
};

export const cloneUpsPalHandler: WSMessageHandler = {
	type: MessageType.CLONE_UPS_PAL,
	async handle(data: { original_pal_id: number; cloned_pal: UPSPal }) {
		const upsState = getUpsState();

		if (data.cloned_pal) {
			upsState.pals = [data.cloned_pal, ...upsState.pals];
			upsState.pagination.totalCount++;

			// Refresh collections to update pal_count
			await upsState.loadCollections();
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
		if (data.success) {
			const target = data.destination_type.toUpperCase();
			const message =
				data.destination_slot !== undefined
					? m.exported_pal_to_target_slot({ pal: c.pal, target, slot: data.destination_slot })
					: m.exported_pal_to_target({ pal: c.pal, target });
			getToastState().add(message, m.success(), 'success');
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
		const toast = getToastState();

		if (data.success && data.cloned_count > 0) {
			// Refresh the pals list to show new cloned pals and collections
			await upsState.loadPals(true);
			await upsState.loadCollections();
			await upsState.loadStats(); // Update stats after cloning

			toast.add(
				m.successfully_cloned_pals_to_entity({
					count: data.cloned_count,
					pals: m.pal({ count: data.cloned_count }),
					entity: c.universalPalStorage
				}),
				m.success(),
				'success'
			);
		} else {
			toast.add(m.clone_to_entity_failed({ entity: c.universalPalStorage }), m.error(), 'error');
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

			// Refresh collections to update pal_count
			await upsState.loadCollections();
		} else {
			toastState.add(m.import_to_entity_failed({ entity: c.universalPalStorage }), m.error(), 'error');
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
			toastState.add(
				m.created_entity_named({ entity: c.collection, name: data.collection.name }),
				m.success(),
				'success'
			);
		}
	}
};

export const updateUpsCollectionHandler: WSMessageHandler = {
	type: MessageType.UPDATE_UPS_COLLECTION,
	async handle(data: { collection: UPSCollection }) {
		const upsState = getUpsState();
		const toastState = getToastState();

		if (data.collection) {
			const index = upsState.collections.findIndex((col) => col.id === data.collection.id);
			if (index >= 0) {
				upsState.collections[index] = data.collection;
				toastState.add(
					m.updated_entity_named({ entity: c.collection, name: data.collection.name }),
					m.success(),
					'success'
				);
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
			// Remove collection from list
			const collection = upsState.collections.find((col) => col.id === data.collection_id);
			upsState.collections = upsState.collections.filter((col) => col.id !== data.collection_id);

			// Clear filter if it was set to this collection
			if (upsState.filters.collectionId === data.collection_id) {
				upsState.filters.collectionId = undefined;
			}

			toastState.add(
				collection
					? m.deleted_entity_named({ entity: c.collection, name: collection.name })
					: m.deleted_entity_only({ entity: c.collection }),
				m.success(),
				'success'
			);
		} else {
			toastState.add(m.delete_entity_failed({ entity: c.collection }), m.error(), 'error');
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
			// Add or update tag in the list
			const index = upsState.tags.findIndex((t) => t.id === data.tag.id);
			if (index >= 0) {
				upsState.tags[index] = data.tag;
			} else {
				upsState.tags = [...upsState.tags, data.tag];
			}
			toastState.add(
				m.created_entity_named({ entity: c.tag, name: data.tag.name }),
				m.success(),
				'success'
			);
		}
	}
};

export const updateUpsTagHandler: WSMessageHandler = {
	type: MessageType.UPDATE_UPS_TAG,
	async handle(data: { tag: UPSTag }) {
		const upsState = getUpsState();
		const toastState = getToastState();

		if (data.tag) {
			// Update tag in the list
			const index = upsState.tags.findIndex((t) => t.id === data.tag.id);
			if (index >= 0) {
				upsState.tags[index] = data.tag;
			}
			toastState.add(
				m.updated_entity_named({ entity: c.tag, name: data.tag.name }),
				m.success(),
				'success'
			);
		}
	}
};

export const deleteUpsTagHandler: WSMessageHandler = {
	type: MessageType.DELETE_UPS_TAG,
	async handle(data: { success: boolean; tag_id: number }) {
		const upsState = getUpsState();
		const toastState = getToastState();

		if (data.success) {
			// Remove tag from the list
			const deletedTag = upsState.tags.find((t) => t.id === data.tag_id);
			upsState.tags = upsState.tags.filter((t) => t.id !== data.tag_id);

			if (deletedTag) {
				toastState.add(
					m.deleted_entity_named({ entity: c.tag, name: deletedTag.name }),
					m.success(),
					'success'
				);
			}
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
	getUpsAllFilteredIdsHandler,
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
	updateUpsTagHandler,
	deleteUpsTagHandler,
	getUpsStatsHandler
];
