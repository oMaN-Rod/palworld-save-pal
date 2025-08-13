<script lang="ts">
	import { PalBadge, TextInputModal } from '$components';
	import ContextMenu from '$components/ui/context-menu/ContextMenu.svelte';
	import { Copy, Trash, Edit, Eye, Upload, Download, FolderPlus, Tag, Share } from 'lucide-svelte';
	import { getUpsState, getModalState, getAppState } from '$states';
	import type { UPSPal, Pal } from '$types';

	let { upsPal, onSelect } = $props<{
		upsPal: UPSPal;
		onSelect?: (upsPal: UPSPal) => void;
	}>();

	const upsState = getUpsState();
	const modal = getModalState();
	const appState = getAppState();

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
			label: 'View Details',
			onClick: () => handleViewDetails(),
			icon: Eye
		},
		{
			label: 'Edit Pal',
			onClick: () => handleEditPal(),
			icon: Edit
		},
		{
			label: 'Clone Pal',
			onClick: () => handleClonePal(),
			icon: Copy
		},
		{
			label: 'Export to Pal Box',
			onClick: () => handleExportToPalBox(),
			icon: Upload
		},
		{
			label: 'Export to GPS',
			onClick: () => handleExportToGPS(),
			icon: Share
		},
		{
			label: 'Export to DPS',
			onClick: () => handleExportToDPS(),
			icon: Download
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

	function handleClick() {
		if (onSelect) {
			onSelect(upsPal);
		}
	}

	function handleKeydown(event: KeyboardEvent) {
		if (event.key === 'Enter' || event.key === ' ') {
			event.preventDefault();
			handleClick();
		}
	}

	async function handleViewDetails() {
		console.log('View details for UPS pal:', upsPal);
	}

	async function handleEditPal() {
		// @ts-ignore
		const result = await modal.showModal<string>(TextInputModal, {
			title: 'Edit Pal',
			value: upsPal.nickname || '',
			inputLabel: 'Enter new nickname:'
		});

		if (result !== null && result !== upsPal.nickname) {
			await upsState.updatePal(upsPal.id, { nickname: result || undefined });
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

	async function handleExportToPalBox() {
		if (!appState.saveFile || !appState.selectedPlayer) {
			await modal.showConfirmModal({
				title: 'Export Failed',
				message: 'Please select a player first.',
				confirmText: 'OK',
				cancelText: ''
			});
			return;
		}

		const confirmed = await modal.showConfirmModal({
			title: 'Export to Pal Box',
			message: `Export ${upsPal.nickname || upsPal.character_id} to ${appState.selectedPlayer.nickname}'s Pal Box?`,
			confirmText: 'Export',
			cancelText: 'Cancel'
		});

		if (confirmed) {
			await upsState.exportPal(upsPal.id, 'pal_box', appState.selectedPlayer.uid);
		}
	}

	async function handleExportToGPS() {
		if (!appState.saveFile || !appState.gps) {
			await modal.showConfirmModal({
				title: 'Export Failed',
				message: 'GPS is not available in this save file.',
				confirmText: 'OK',
				cancelText: ''
			});
			return;
		}

		const confirmed = await modal.showConfirmModal({
			title: 'Export to GPS',
			message: `Export ${upsPal.nickname || upsPal.character_id} to Global Pal Storage?`,
			confirmText: 'Export',
			cancelText: 'Cancel'
		});

		if (confirmed) {
			await upsState.exportPal(upsPal.id, 'gps');
		}
	}

	async function handleExportToDPS() {
		if (!appState.saveFile || !appState.selectedPlayer?.dps) {
			await modal.showConfirmModal({
				title: 'Export Failed',
				message: 'Please select a player with DPS access.',
				confirmText: 'OK',
				cancelText: ''
			});
			return;
		}

		const confirmed = await modal.showConfirmModal({
			title: 'Export to DPS',
			message: `Export ${upsPal.nickname || upsPal.character_id} to ${appState.selectedPlayer.nickname}'s DPS?`,
			confirmText: 'Export',
			cancelText: 'Cancel'
		});

		if (confirmed) {
			await upsState.exportPal(upsPal.id, 'dps', appState.selectedPlayer.uid);
		}
	}

	async function handleAddToCollection() {
		const collections = upsState.filteredCollections;
		if (collections.length === 0) {
			await modal.showConfirmModal({
				title: 'No Collections',
				message: 'Create a collection first to organize your Pals.',
				confirmText: 'OK',
				cancelText: ''
			});
			return;
		}

		await modal.showConfirmModal({
			title: 'Add to Collection',
			message: 'Collection functionality coming soon!',
			confirmText: 'OK',
			cancelText: ''
		});
	}

	async function handleManageTags() {
		await modal.showConfirmModal({
			title: 'Manage Tags',
			message: 'Tag management functionality coming soon!',
			confirmText: 'OK',
			cancelText: ''
		});
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
			onMove={dummyHandler}
			onAdd={dummyHandler}
			onClone={dummyHandler}
			onDelete={dummyHandler}
		/>
	</div>
</ContextMenu>
