<script lang="ts">
	import { Tooltip } from '$components/ui';
	import {
		activeSkillsData,
		elementsData,
		passiveSkillsData,
		workSuitabilityData
	} from '$lib/data';
	import { cn } from '$theme';
	import { PalGender, type PresetProfile, type WorkSuitability } from '$types';
	import { ASSET_DATA_PATH, staticIcons } from '$types/icons';
	import { assetLoader, calculateFilters } from '$utils';
	import { Rating } from '@skeletonlabs/skeleton-svelte';
	import { Lock, Unlock } from 'lucide-svelte';

	const suitabilityImageMap = {
		EmitFlame: 'kindling',
		Watering: 'watering',
		Seeding: 'planting',
		GenerateElectricity: 'generating',
		Handcraft: 'handiwork',
		Collection: 'gathering',
		Deforest: 'deforesting',
		Mining: 'mining',
		OilExtraction: 'extracting',
		ProductMedicine: 'production',
		Cool: 'cooling',
		Transport: 'transporting',
		MonsterFarm: 'farming'
	};

	let { preset = $bindable() } = $props<{
		preset: PresetProfile;
	}>();

	let elementIcons = $derived.by(() => {
		const icons: Record<string, string> = {};
		for (const elementType of Object.keys(elementsData.elements)) {
			const elementObj = elementsData.elements[elementType];
			if (elementObj) {
				icons[elementType] = assetLoader.loadImage(
					`${ASSET_DATA_PATH}/img/${elementObj.badge_icon}.webp`
				) as string;
			}
		}
		return icons;
	});

	let passiveSkillIcons = $derived.by(() => {
		const icons: Record<string, string> = {};
		for (const skill of Object.values(passiveSkillsData.passiveSkills)) {
			if (icons[skill.details.rank]) continue;
			icons[skill.details.rank] = assetLoader.loadImage(
				`${ASSET_DATA_PATH}/img/rank_${skill.details.rank}.webp`
			) as string;
		}
		return icons;
	});

	function getPassiveSkillIconFilter(skillId: string): string {
		const skill = passiveSkillsData.passiveSkills[skillId];
		if (!skill || skill.localized_name === 'None') return '';
		switch (skill.details.rank) {
			case 1:
				return '';
			case 2:
			case 3:
				return calculateFilters('#fcdf19');
			case 4:
				return calculateFilters('#68ffd8');
			default:
				return calculateFilters('#FF0000');
		}
	}
</script>

<div class="space-y-4">
	<div class="grid grid-cols-2 gap-2">
		<div class="flex flex-col">
			<h5 class="h5 border-b-surface-600 mb-2 border-b-2 py-4">Attributes</h5>
			<div class="flex w-full justify-center gap-2 rounded-sm p-4">
				<div class="flex flex-col items-center justify-center">
					<Rating value={preset.pal_preset.rank} count={4} itemClasses="text-gray" disabled />
					<span class="text-surface-400 font-bold">LEVEL</span>
					<span class="text-4xl font-bold">{preset.pal_preset.level}</span>
				</div>
				<Tooltip>
					<div class="flex flex-col">
						<div class="relative flex items-center justify-center">
							{#if preset.pal_preset.is_boss}
								<div class="absolute -left-4 -top-1 h-6 w-6 xl:h-8 xl:w-8">
									<img src={staticIcons.alphaIcon} alt="Alpha" class="pal-element-badge" />
								</div>
							{/if}
							{#if preset.pal_preset.is_lucky}
								<div class="absolute -left-4 -top-1 h-6 w-6 xl:h-8 xl:w-8">
									<img src={staticIcons.luckyIcon} alt="Lucky" class="pal-element-badge" />
								</div>
							{/if}
							<img
								src={assetLoader.loadMenuImage(preset.pal_preset.character_id as string)}
								alt={preset.pal_preset.character_id}
								class="ml-4 h-20 w-20 rounded-full"
							/>

							{#if preset.pal_preset.gender}
								<div
									class={cn(
										'absolute -right-4 -top-1 h-6 w-6 xl:h-8 xl:w-8',
										preset.pal_preset.gender == PalGender.MALE
											? 'text-primary-300'
											: 'text-tertiary-300'
									)}
								>
									<img
										src={assetLoader.loadImage(
											`${ASSET_DATA_PATH}/img/${preset.pal_preset.gender}.webp`
										)}
										alt={preset.pal_preset.gender}
									/>
								</div>
							{/if}
							<div class="absolute -bottom-4 -right-6 h-6 w-6 xl:h-8 xl:w-8">
								{#if preset.pal_preset.lock}
									<Lock class="h-4 w-4" />
								{:else}
									<Unlock class="h-4 w-4" />
								{/if}
							</div>
						</div>
					</div>
					{#snippet popup()}
						{#snippet label(label: string, value: string | number)}
							<div class="flex items-center space-x-2">
								<span class="font-bold">{label}:</span>
								<span>{value}</span>
							</div>
						{/snippet}
						<div class="grid grid-cols-2 gap-2">
							{@render label('Lucky', preset.pal_preset!.is_lucky ? 'Yes' : 'No')}
							{@render label('Alpha', preset.pal_preset!.is_boss ? 'Yes' : 'No')}
							{#if preset.pal_preset!.gender}
								{@render label('Gender', preset.pal_preset!.gender)}
							{/if}
							{#if preset.pal_preset?.level}
								{@render label('Level', preset.pal_preset!.level)}
							{/if}
							{#if preset.pal_preset?.rank}
								{@render label('Rank', preset.pal_preset!.rank)}
							{/if}
							{#if preset.pal_preset?.lock}
								{@render label('Locked to', preset.pal_preset!.character_id || 'None')}
							{/if}
						</div>
					{/snippet}
				</Tooltip>
			</div>
		</div>

		<div class="space-y-2">
			<h5 class="h5 border-b-surface-600 mb-2 border-b-2 py-4">Stats</h5>
			<div class="flex flex-col">
				<span class="text-surface-300 mr-1">IVs</span>
				<div class="flex justify-center space-x-2">
					<div class="chip bg-green-700">
						<img src={staticIcons.hpIcon} alt="HP" class="h-4 w-4" />
						<span class="text-sm font-bold">
							{preset.pal_preset.talent_hp}
						</span>
					</div>
					<div class="chip bg-red-700">
						<img src={staticIcons.attackIcon} alt="Attack" class="h-6 w-6" />
						<span class="text-sm font-bold">
							{preset.pal_preset.talent_shot}
						</span>
					</div>
					<div class="chip bg-blue-700">
						<img src={staticIcons.defenseIcon} alt="Defense" class="h-6 w-6" />
						<span class="text-sm font-bold">
							{preset.pal_preset.talent_defense}
						</span>
					</div>
				</div>
			</div>
			<div class="flex flex-col">
				<span class="text-surface-300 mr-1">Souls</span>
				<div class="flex justify-center space-x-2">
					<div class="chip bg-green-700">
						<img src={staticIcons.hpIcon} alt="HP" class="h-4 w-4" />
						<span class="text-sm font-bold">
							{preset.pal_preset.rank_hp}
						</span>
					</div>
					<div class="chip bg-red-700">
						<img src={staticIcons.attackIcon} alt="Attack" class="h-6 w-6" />
						<span class="text-sm font-bold">
							{preset.pal_preset.rank_attack}
						</span>
					</div>
					<div class="chip bg-blue-700">
						<img src={staticIcons.defenseIcon} alt="Defense" class="h-6 w-6" />
						<span class="text-sm font-bold">
							{preset.pal_preset.rank_defense}
						</span>
					</div>
					<div class="chip bg-purple-700">
						<img src={staticIcons.workSpeedIcon} alt="Defense" class="h-6 w-6" />
						<span class="text-sm font-bold">
							{preset.pal_preset.rank_craftspeed}
						</span>
					</div>
				</div>
			</div>
		</div>
	</div>

	{#if preset.pal_preset.active_skills?.length || preset.pal_preset.passive_skills?.length || preset.pal_preset.learned_skills?.length}
		<div>
			<h5 class="h5 border-b-surface-600 mb-2 border-b-2 py-4">Skills</h5>
			<div class="grid grid-cols-2 gap-2">
				{#if preset.pal_preset.active_skills && preset.pal_preset.active_skills.length > 0}
					<div class="flex gap-2 rounded-sm p-4">
						<span class="font-bold">Active Skills:</span>
						<span class="border-r-surface-600 border-r pr-2">
							{preset.pal_preset.active_skills.length}
						</span>
						<div class="ml-4 mt-1">
							{#each preset.pal_preset.active_skills as skillId}
								{@const skill = activeSkillsData.getByKey(skillId)}
								{#if skill}
									<div class="flex items-center space-x-2">
										{#if skill.details.element && elementIcons[skill.details.element]}
											<img
												src={elementIcons[skill.details.element]}
												alt={skill.details.element}
												class="h-4 w-4"
											/>
										{/if}
										<span>{skill.localized_name}</span>
										<span class="text-xs"
											>({skill.details.power === 0 ? 'NA' : skill.details.power})</span
										>
									</div>
								{:else}
									<div>{skillId}</div>
								{/if}
							{/each}
						</div>
					</div>
				{/if}

				{#if preset.pal_preset.passive_skills && preset.pal_preset.passive_skills.length > 0}
					<div class="flex gap-2 rounded-sm p-4">
						<span class="font-bold">Passive Skills:</span>
						<span class="border-r-surface-600 border-r pr-2">
							{preset.pal_preset.passive_skills.length}
						</span>
						<div class="ml-4 mt-1 grid grid-cols-2 gap-2">
							{#each preset.pal_preset.passive_skills as skillId}
								{@const skill = passiveSkillsData.passiveSkills[skillId]}
								{#if skill}
									<div class="flex items-center space-x-2">
										{#if passiveSkillIcons[skill.details.rank]}
											<img
												src={passiveSkillIcons[skill.details.rank]}
												alt={`Rank ${skill.details.rank}`}
												class="h-4 w-4"
												style="filter: {getPassiveSkillIconFilter(skillId)};"
											/>
										{/if}
										<span>{skill.localized_name}</span>
									</div>
								{:else}
									<div>{skillId}</div>
								{/if}
							{/each}
						</div>
					</div>
				{/if}

				{#if preset.pal_preset.learned_skills && preset.pal_preset.learned_skills.length > 0}
					<div class="">
						<span class="font-bold">Learned Skills:</span>
						{preset.pal_preset.learned_skills.length}
					</div>
				{/if}
			</div>
		</div>
	{/if}

	{#if preset.pal_preset.work_suitability && Object.keys(preset.pal_preset.work_suitability).length > 0}
		<h5 class="h5 border-b-surface-600 mb-2 border-b-2 py-4">Work Suitability</h5>
		<div class="flex gap-2">
			{#each Object.entries(preset.pal_preset.work_suitability) as [ws, value]}
				{@const suitability: WorkSuitability = ws as WorkSuitability}
				{@const iconPath = assetLoader.loadImage(
					`${ASSET_DATA_PATH}/img/${suitabilityImageMap[suitability]}.webp`
				)}
				<Tooltip>
					<div
						class="border-l-surface-600 bg-surface-800 relative w-full overflow-hidden rounded-none border-l-2 p-0 shadow-none"
					>
						<div class="flex w-full items-center">
							<img src={iconPath} alt="{suitability} icon" class="ml-2 h-4 w-4 2xl:h-6 2xl:w-6" />
							<span class="p-2 font-bold 2xl:text-lg">+ {value}</span>
						</div>
					</div>
					{#snippet popup()}
						<div class="flex items-center space-x-2">
							{#if value !== 0}
								<span class="p-2 text-lg font-bold">+ {value}</span>
							{/if}
							<span class="text-lg">
								{workSuitabilityData.workSuitability[suitability].localized_name ??
									suitabilityImageMap[suitability].charAt(0).toUpperCase() +
										suitabilityImageMap[suitability].slice(1)}
							</span>
						</div>
					{/snippet}
				</Tooltip>
			{/each}
		</div>
	{/if}
</div>
