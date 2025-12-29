<script lang="ts">
	import { cn } from '$theme';
	import { missionsData } from '$lib/data';
	import type { Mission, MissionType } from '$types';
	import { Check, Circle, Trash2, CheckCircle } from 'lucide-svelte';
	import { SectionHeader, Tooltip } from '$components/ui';

	let {
		currentMissions = [],
		completedMissions = [],
		selectedMission = $bindable<Mission | undefined>(undefined),
		missionType,
		onClearMission,
		onMarkComplete
	}: {
		currentMissions: string[];
		completedMissions: string[];
		selectedMission?: Mission;
		missionType: MissionType;
		onClearMission?: (missionId: string, isCompleted: boolean) => void;
		onMarkComplete?: (missionId: string) => void;
	} = $props();

	const filteredCurrentMissions = $derived.by(() => {
		return currentMissions
			.map((id) => missionsData.getByKey(id))
			.filter(
				(mission): mission is Mission => mission !== undefined && mission.quest_type === missionType
			);
	});

	const filteredCompletedMissions = $derived.by(() => {
		return completedMissions
			.map((id) => missionsData.getByKey(id))
			.filter(
				(mission): mission is Mission => mission !== undefined && mission.quest_type === missionType
			);
	});

	function handleSelectMission(mission: Mission) {
		selectedMission = mission;
	}

	function isSelected(mission: Mission) {
		return selectedMission?.id === mission.id;
	}

	function handleClear(e: MouseEvent, missionId: string, isCompleted: boolean) {
		e.stopPropagation();
		onClearMission?.(missionId, isCompleted);
	}

	function handleComplete(e: MouseEvent, missionId: string) {
		e.stopPropagation();
		onMarkComplete?.(missionId);
	}
</script>

<div class="flex h-full flex-col space-y-4">
	{#if filteredCurrentMissions.length > 0}
		<div class="flex flex-col gap-2">
			<SectionHeader text="Current Missions" />
			<ul
				class="divide-surface-700 border-surface-700 max-h-[300px] divide-y overflow-y-auto border"
			>
				{#each filteredCurrentMissions as mission, index (index)}
					<li>
						<button
							class={cn(
								'hover:bg-secondary-500/25 group flex w-full items-center gap-2 p-3 text-left transition-colors',
								isSelected(mission) ? 'bg-secondary-500/25' : ''
							)}
							onclick={() => handleSelectMission(mission)}
						>
							<Circle class="text-warning-500 h-4 w-4 shrink-0" />
							<span class="flex-1 truncate">{mission.localized_name}</span>
							<div class="flex gap-1 opacity-0 transition-opacity group-hover:opacity-100">
								<Tooltip label="Mark Complete" position="left">
									<button
										class="hover:bg-success-500/25 rounded p-1"
										onclick={(e) => handleComplete(e, mission.id)}
									>
										<CheckCircle class="text-success-500 h-4 w-4" />
									</button>
								</Tooltip>
								<Tooltip label="Clear Mission" position="left">
									<button
										class="hover:bg-error-500/25 rounded p-1"
										onclick={(e) => handleClear(e, mission.id, false)}
									>
										<Trash2 class="text-error-500 h-4 w-4" />
									</button>
								</Tooltip>
							</div>
						</button>
					</li>
				{/each}
			</ul>
		</div>
	{/if}

	{#if filteredCompletedMissions.length > 0}
		<div class="flex flex-col gap-2">
			<SectionHeader text="Completed Missions" />
			<ul class="divide-surface-700 border-surface-700 divide-y overflow-y-auto border">
				{#each filteredCompletedMissions as mission, index (index)}
					<li>
						<button
							class={cn(
								'hover:bg-secondary-500/25 group flex w-full items-center gap-2 p-3 text-left opacity-75 transition-colors',
								isSelected(mission) ? 'bg-secondary-500/25' : ''
							)}
							onclick={() => handleSelectMission(mission)}
						>
							<Check class="text-success-500 h-4 w-4 shrink-0" />
							<span class="flex-1 truncate">{mission.localized_name}</span>
							<div class="flex gap-1 opacity-0 transition-opacity group-hover:opacity-100">
								<Tooltip label="Clear Mission" position="left">
									<button
										class="hover:bg-error-500/25 rounded p-1"
										onclick={(e) => handleClear(e, mission.id, true)}
									>
										<Trash2 class="text-error-500 h-4 w-4" />
									</button>
								</Tooltip>
							</div>
						</button>
					</li>
				{/each}
			</ul>
		</div>
	{/if}

	{#if filteredCurrentMissions.length === 0 && filteredCompletedMissions.length === 0}
		<div class="text-surface-400 flex h-full items-center justify-center">
			<p>No {missionType.toLowerCase()} missions found</p>
		</div>
	{/if}
</div>
