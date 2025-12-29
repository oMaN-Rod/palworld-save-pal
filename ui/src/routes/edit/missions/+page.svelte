<script lang="ts">
	import { MissionDetails, MissionList } from '$components';
	import { Tooltip, TooltipButton } from '$components/ui';
	import { getAppState, getModalState, getToastState } from '$states';
	import { Tabs } from '@skeletonlabs/skeleton-svelte';
	import type { ValueChangeDetails } from '@zag-js/tabs';
	import { EntryState, type Mission, type MissionType } from '$types';
	import { CheckCheck, Trash2, ListX } from 'lucide-svelte';
	import { ConfirmModal } from '$components';

	const appState = getAppState();
	const modal = getModalState();
	const toast = getToastState();

	let activeTab: MissionType = $state('Main');
	let selectedMission: Mission | undefined = $state(undefined);

	function handleTabChange(e: ValueChangeDetails) {
		activeTab = e.value as MissionType;
		selectedMission = undefined;
	}

	function getMissionPrefix(type: MissionType): string {
		return type === 'Main' ? 'Main_' : 'Sub_';
	}

	async function clearAllCurrentMissions() {
		if (!appState.selectedPlayer) return;
		const prefix = getMissionPrefix(activeTab);
		const count = appState.selectedPlayer.current_missions.filter((m) =>
			m.startsWith(prefix)
		).length;
		if (count === 0) {
			toast.add('No current missions to clear', undefined, 'warning');
			return;
		}
		// @ts-ignore
		const confirmed = await modal.showModal(ConfirmModal, {
			title: 'Clear All Current Missions',
			message: `Are you sure you want to clear all ${count} current ${activeTab.toLowerCase()} missions?`
		});
		if (!confirmed) return;
		appState.selectedPlayer.current_missions = appState.selectedPlayer.current_missions.filter(
			(m) => !m.startsWith(prefix)
		);
		appState.selectedPlayer.state = EntryState.MODIFIED;
		selectedMission = undefined;
		toast.add(`Cleared ${count} current ${activeTab.toLowerCase()} missions`);
	}

	async function clearAllCompletedMissions() {
		if (!appState.selectedPlayer) return;
		const prefix = getMissionPrefix(activeTab);
		const count = appState.selectedPlayer.completed_missions.filter((m) =>
			m.startsWith(prefix)
		).length;
		if (count === 0) {
			toast.add('No completed missions to clear', undefined, 'warning');
			return;
		}
		// @ts-ignore
		const confirmed = await modal.showModal(ConfirmModal, {
			title: 'Clear All Completed Missions',
			message: `Are you sure you want to clear all ${count} completed ${activeTab.toLowerCase()} missions?`
		});
		if (!confirmed) return;
		appState.selectedPlayer.completed_missions = appState.selectedPlayer.completed_missions.filter(
			(m) => !m.startsWith(prefix)
		);
		appState.selectedPlayer.state = EntryState.MODIFIED;
		selectedMission = undefined;
		toast.add(`Cleared ${count} completed ${activeTab.toLowerCase()} missions`);
	}

	async function markAllCurrentAsComplete() {
		if (!appState.selectedPlayer) return;
		const prefix = getMissionPrefix(activeTab);
		const currentOfType = appState.selectedPlayer.current_missions.filter((m) =>
			m.startsWith(prefix)
		);
		if (currentOfType.length === 0) {
			toast.add('No current missions to complete', undefined, 'warning');
			return;
		}
		// @ts-ignore
		const confirmed = await modal.showModal(ConfirmModal, {
			title: 'Mark All as Complete',
			message: `Are you sure you want to mark all ${currentOfType.length} current ${activeTab.toLowerCase()} missions as complete?`
		});
		if (!confirmed) return;
		appState.selectedPlayer.current_missions = appState.selectedPlayer.current_missions.filter(
			(m) => !m.startsWith(prefix)
		);
		appState.selectedPlayer.completed_missions = [
			...appState.selectedPlayer.completed_missions,
			...currentOfType
		];
		appState.selectedPlayer.state = EntryState.MODIFIED;
		selectedMission = undefined;
		toast.add(`Marked ${currentOfType.length} ${activeTab.toLowerCase()} missions as complete`);
	}

	function handleClearMission(missionId: string, isCompleted: boolean) {
		if (!appState.selectedPlayer) return;
		if (isCompleted) {
			appState.selectedPlayer.completed_missions =
				appState.selectedPlayer.completed_missions.filter((m) => m !== missionId);
		} else {
			appState.selectedPlayer.current_missions = appState.selectedPlayer.current_missions.filter(
				(m) => m !== missionId
			);
		}
		appState.selectedPlayer.state = EntryState.MODIFIED;
		if (selectedMission?.id === missionId) {
			selectedMission = undefined;
		}
		toast.add('Mission cleared');
	}

	function handleMarkComplete(missionId: string) {
		if (!appState.selectedPlayer) return;
		appState.selectedPlayer.current_missions = appState.selectedPlayer.current_missions.filter(
			(m) => m !== missionId
		);
		if (!appState.selectedPlayer.completed_missions.includes(missionId)) {
			appState.selectedPlayer.completed_missions = [
				...appState.selectedPlayer.completed_missions,
				missionId
			];
		}
		appState.selectedPlayer.state = EntryState.MODIFIED;
		if (selectedMission?.id === missionId) {
			selectedMission = undefined;
		}
		toast.add('Mission marked as complete');
	}
</script>

{#if appState.selectedPlayer}
	<div class="relative flex h-full flex-col p-4">
		<div class="mb-4 flex items-center justify-between">
			<Tabs
				listBorder="border-none"
				listClasses="btn-group preset-outlined-surface-200-800 w-auto flex-col md:flex-row rounded-sm"
				value={activeTab}
				onValueChange={handleTabChange}
			>
				{#snippet list()}
					<Tabs.Control
						value="Main"
						classes="px-6"
						base="border-none hover:bg-secondary-500/50 rounded-sm"
						labelBase="btn"
						stateActive="bg-secondary-800"
						padding="p-0"
					>
						Main Missions
					</Tabs.Control>
					<Tabs.Control
						value="Sub"
						classes="px-6"
						base="border-none hover:bg-secondary-500/50 rounded-sm"
						labelBase="btn"
						stateActive="bg-secondary-800"
						padding="p-0"
					>
						Sub Missions
					</Tabs.Control>
				{/snippet}
				{#snippet content()}
					<Tabs.Panel value="Main">
						<div class="mt-4 grid h-[calc(100vh-200px)] grid-cols-[25%_1fr] gap-4">
							<div class="overflow-y-auto">
								<MissionList
									currentMissions={appState.selectedPlayer?.current_missions ?? []}
									completedMissions={appState.selectedPlayer?.completed_missions ?? []}
									bind:selectedMission
									missionType="Main"
									onClearMission={handleClearMission}
									onMarkComplete={handleMarkComplete}
								/>
							</div>
							<div class="overflow-y-auto">
								<MissionDetails mission={selectedMission} />
							</div>
						</div>
					</Tabs.Panel>
					<Tabs.Panel value="Sub">
						<div class="mt-4 grid h-[calc(100vh-200px)] grid-cols-[25%_1fr] gap-4">
							<div class="overflow-y-auto">
								<MissionList
									currentMissions={appState.selectedPlayer?.current_missions ?? []}
									completedMissions={appState.selectedPlayer?.completed_missions ?? []}
									bind:selectedMission
									missionType="Sub"
									onClearMission={handleClearMission}
									onMarkComplete={handleMarkComplete}
								/>
							</div>
							<div class="overflow-y-auto">
								<MissionDetails mission={selectedMission} />
							</div>
						</div>
					</Tabs.Panel>
				{/snippet}
			</Tabs>
			<div class="absolute right-4 top-4 flex items-center gap-2">
				<TooltipButton
					buttonClass="preset-outlined-surface-200-800 rounded-sm p-2 hover:bg-secondary-500/50"
					popupLabel="Mark All Current as Complete"
					position="bottom"
					onclick={markAllCurrentAsComplete}
				>
					<CheckCheck class="h-5 w-5" />
				</TooltipButton>
				<TooltipButton
					buttonClass="preset-outlined-surface-200-800 rounded-sm p-2 hover:bg-secondary-500/50"
					popupLabel="Clear All Current Missions"
					position="bottom"
					onclick={clearAllCurrentMissions}
				>
					<ListX class="h-5 w-5" />
				</TooltipButton>
				<TooltipButton
					buttonClass="preset-outlined-surface-200-800 rounded-sm p-2 hover:bg-secondary-500/50"
					popupLabel="Clear All Completed Missions"
					position="bottom"
					onclick={clearAllCompletedMissions}
				>
					<Trash2 class="h-5 w-5" />
				</TooltipButton>
			</div>
		</div>
	</div>
{:else}
	<div class="flex h-full w-full items-center justify-center">
		<h2 class="h2">Select a Player to view missions</h2>
	</div>
{/if}
