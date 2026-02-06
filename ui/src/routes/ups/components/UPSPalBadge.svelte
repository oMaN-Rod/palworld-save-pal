<script lang="ts">
	import { PalBadge } from '$components';
	import { EditTagsModal, AddToCollectionModal, ExportPalModal } from '$components/modals';
	import ContextMenu from '$components/ui/context-menu/ContextMenu.svelte';
	import { Copy, Trash, Upload, FolderPlus, Tag } from 'lucide-svelte';
	import {
		getUpsState,
		getModalState,
		getAppState,
		getNavigationState,
		getToastState
	} from '$states';
	import type { UPSPal, Pal, AddToCollectionResult } from '$types';
	import * as m from '$i18n/messages';
	import { c } from '$utils/commonTranslations';

	let { upsPal, onSelect } = $props<{
		upsPal: UPSPal;
		onSelect?: (upsPal: UPSPal, event: MouseEvent) => void;
	}>();

	const upsState = getUpsState();
	const modal = getModalState();
	const appState = getAppState();
	const nav = getNavigationState();
	const toast = getToastState();

	const pal = $derived.by<Pal>(() => {
		let pal = {
			...upsPal.pal_data,
			id: upsPal.id
		} as Pal;
		if (!pal.character_key && upsPal.character_key) {
			pal.character_key = upsPal.character_key;
		}
		return pal;
	});

	const menuItems = $derived([
		{
			label: m.clone_selected_pal({ pal: c.pal }),
			onClick: () => handleClonePal(),
			icon: Copy
		},
		{
			label: m.export(),
			onClick: () => handleExport(),
			icon: Upload
		},
		{
			label: m.add_to_collection(),
			onClick: () => handleAddToCollection(),
			icon: FolderPlus
		},
		{
			label: m.edit_entity({ entity: c.tags }),
			onClick: () => handleManageTags(),
			icon: Tag
		},
		{
			label: m.delete_entity({ entity: m.ups() }),
			onClick: () => handleDeleteFromUPS(),
			icon: Trash
		}
	]);

	const selected: string[] = $derived.by(() => {
		if (upsState.selectedPals.size > 0) {
			return Array.from(upsState.selectedPals).map((id) => id.toString());
		}
		return [];
	});

	function handleClick(event: MouseEvent) {
		if (event.ctrlKey || event.metaKey) {
			// Ctrl+click for selection (following other storage systems pattern)
			if (onSelect) {
				onSelect(upsPal, event);
			}
		} else {
			// Regular click - navigate to edit page with Pal tab
			handlePalEdit();
		}
	}

	function handlePalEdit() {
		// Set the selected pal in app state and navigate to edit page
		const palWithMetadata = {
			...pal,
			// Add metadata to track that this pal comes from UPS
			__ups_source: true,
			__ups_id: upsPal.id
		};
		appState.selectedPal = palWithMetadata;
		nav.saveAndNavigate('/edit/pal');
	}

	function handleKeydown(event: KeyboardEvent) {
		if (event.key === 'Enter' || event.key === ' ') {
			event.preventDefault();
			// For keyboard navigation, treat as regular click (open edit)
			handlePalEdit();
		}
	}

	async function handleClonePal() {
		const confirmed = await modal.showConfirmModal({
			title: m.clone_selected_pal({ pal: c.pal }),
			message: m.delete_entity_by_name_confirm({ name: upsPal.nickname || upsPal.character_id }),
			confirmText: m.clone_selected_pal({ pal: '' }),
			cancelText: m.cancel()
		});

		if (confirmed) {
			await upsState.clonePal(upsPal.id);
		}
	}

	async function handleExport() {
		// @ts-ignore
		const result = await modal.showModal<{ target: string; playerId?: string }>(ExportPalModal, {
			title: m.export_pals({ pals: c.pal, count: 1 }),
			pals: [upsPal]
		});

		if (result) {
			const target = result.target as 'pal_box' | 'dps' | 'gps';
			try {
				await upsState.exportPal(upsPal.id, target, result.playerId);
				toast.add(
					m.successfully_cloned_pal_to_entity({
						pal: upsPal.nickname || upsPal.character_id,
						entity: result.target.toUpperCase()
					}),
					m.success(),
					'success'
				);
			} catch (error) {
				console.error('Export failed:', error);
				toast.add(m.import_failed(), m.error(), 'error');
			}
		}
	}

	async function handleAddToCollection() {
		// @ts-ignore
		const result = await modal.showModal<AddToCollectionResult>(AddToCollectionModal, {
			title: m.add_to_collection(),
			pals: [upsPal]
		});

		if (result) {
			const collectionId = result.removeFromCollection ? undefined : result.collectionId;
			try {
				await upsState.updatePal(upsPal.id, { collection_id: collectionId });
				await upsState.loadAll(); // Refresh to update collection counts

				if (result.removeFromCollection) {
					toast.add(
						m.removed_pals_from_collections({ pals: c.pal, count: 1 }),
						m.success(),
						'success'
					);
				} else {
					toast.add(m.moved_pals_to_collection({ pals: c.pal, count: 1 }), m.success(), 'success');
				}
			} catch (error) {
				console.error('Collection update failed:', error);
				toast.add(m.import_failed(), m.error(), 'error');
			}
		}
	}

	async function handleManageTags() {
		// @ts-ignore
		const result = await modal.showModal<string[]>(EditTagsModal, {
			title: m.edit_entity({ entity: c.tags }),
			pals: [upsPal]
		});

		if (result) {
			try {
				await upsState.updatePal(upsPal.id, { tags: result });
				await upsState.loadAll(); // Refresh to update available tags
				toast.add(m.updated_tags_for_pals({ pals: c.pal, count: 1 }), m.success(), 'success');
			} catch (error) {
				console.error('Tag update failed:', error);
				toast.add(m.import_failed(), m.error(), 'error');
			}
		}
	}

	async function handleDeleteFromUPS() {
		const confirmed = await modal.showConfirmModal({
			title: m.delete_entity({ entity: m.ups() }),
			message: m.delete_entity_by_name_confirm({ name: upsPal.nickname || upsPal.character_id }),
			confirmText: m.delete(),
			cancelText: m.cancel()
		});

		if (confirmed) {
			await upsState.deletePals([upsPal.id]);
		}
	}

	function dummyHandler() {
		// These are required by PalBadge but not used in UPS context
	}
</script>

<ContextMenu items={menuItems} menuClass="bg-surface-700" xOffset={-32}>
	<div onclick={handleClick} onkeydown={handleKeydown} role="button" tabindex="0">
		<PalBadge
			{pal}
			{selected}
			onSelect={(p, e) => {
				if (onSelect) {
					onSelect(p, e);
				}
			}}
			onMove={dummyHandler}
			onAdd={dummyHandler}
			onClone={dummyHandler}
			onDelete={dummyHandler}
		/>
	</div>
</ContextMenu>
