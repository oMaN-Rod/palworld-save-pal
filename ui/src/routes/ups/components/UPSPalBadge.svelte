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
	import { goto } from '$app/navigation';
	import type { UPSPal, Pal, AddToCollectionResult } from '$types';

	let { upsPal, onSelect } = $props<{
		upsPal: UPSPal;
		onSelect?: (upsPal: UPSPal, event: MouseEvent) => void;
	}>();

	const upsState = getUpsState();
	const modal = getModalState();
	const appState = getAppState();
	const nav = getNavigationState();
	const toast = getToastState();

	const pal = $derived<Pal>({
		...upsPal.pal_data,
		id: upsPal.id,
		instance_id: upsPal.instance_id,
		character_id: upsPal.character_id,
		nickname: upsPal.nickname,
		level: upsPal.level,
		character_key: upsPal.character_key
	} as Pal);

	const menuItems = $derived([
		{
			label: 'Clone Pal',
			onClick: () => handleClonePal(),
			icon: Copy
		},
		{
			label: 'Export',
			onClick: () => handleExport(),
			icon: Upload
		},
		{
			label: 'Add to Collection',
			onClick: () => handleAddToCollection(),
			icon: FolderPlus
		},
		{
			label: 'Manage Tags',
			onClick: () => handleManageTags(),
			icon: Tag
		},
		{
			label: 'Delete from UPS',
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
		nav.activeTab = 'pal';
		// Navigate to edit page
		goto('/edit');
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
			title: 'Clone Pal',
			message: `Are you sure you want to clone ${upsPal.nickname || upsPal.character_id}?`,
			confirmText: 'Clone',
			cancelText: 'Cancel'
		});

		if (confirmed) {
			await upsState.clonePal(upsPal.id);
		}
	}

	async function handleExport() {
		// @ts-ignore
		const result = await modal.showModal<{ target: string; playerId?: string }>(ExportPalModal, {
			title: 'Export Pal',
			pals: [upsPal]
		});

		if (result) {
			const target = result.target as 'pal_box' | 'dps' | 'gps';
			try {
				await upsState.exportPal(upsPal.id, target, result.playerId);
				toast.add(
					`Successfully exported ${upsPal.nickname || upsPal.character_id} to ${result.target.toUpperCase()}`,
					'Success',
					'success'
				);
			} catch (error) {
				console.error('Export failed:', error);
				toast.add('Export failed. Please try again.', 'Error', 'error');
			}
		}
	}

	async function handleAddToCollection() {
		// @ts-ignore
		const result = await modal.showModal<AddToCollectionResult>(AddToCollectionModal, {
			title: 'Add to Collection',
			pals: [upsPal]
		});

		if (result) {
			const collectionId = result.removeFromCollection ? undefined : result.collectionId;
			try {
				await upsState.updatePal(upsPal.id, { collection_id: collectionId });
				await upsState.loadAll(); // Refresh to update collection counts

				if (result.removeFromCollection) {
					toast.add('Removed pal from collection', 'Success', 'success');
				} else {
					toast.add('Added pal to collection', 'Success', 'success');
				}
			} catch (error) {
				console.error('Collection update failed:', error);
				toast.add('Failed to update collection. Please try again.', 'Error', 'error');
			}
		}
	}

	async function handleManageTags() {
		// @ts-ignore
		const result = await modal.showModal<string[]>(EditTagsModal, {
			title: 'Manage Tags',
			pals: [upsPal]
		});

		if (result) {
			try {
				await upsState.updatePal(upsPal.id, { tags: result });
				await upsState.loadAll(); // Refresh to update available tags
				toast.add('Updated pal tags', 'Success', 'success');
			} catch (error) {
				console.error('Tag update failed:', error);
				toast.add('Failed to update tags. Please try again.', 'Error', 'error');
			}
		}
	}

	async function handleDeleteFromUPS() {
		const confirmed = await modal.showConfirmModal({
			title: 'Delete from UPS',
			message: `Are you sure you want to delete ${upsPal.nickname || upsPal.character_id} from Universal Pal Storage? This action cannot be undone.`,
			confirmText: 'Delete',
			cancelText: 'Cancel'
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
