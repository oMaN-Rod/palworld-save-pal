<script lang="ts">
	import { Card, Tooltip, Combobox, List } from '$components/ui';
	import type { ActiveSkill } from '$types';
	import { Plus, Save, X, Trash, TimerReset, Delete, BicepsFlexed, Brain } from 'lucide-svelte';
	import { activeSkillsData, elementsData, palsData } from '$lib/data';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$utils';
	import { staticIcons } from '$types/icons';

	let { closeModal, pal } = $props<{
		closeModal: (value: any) => void;
		pal: any;
	}>();

	let selectedSkill: string = $state('');
	let learnedSkills: { id: string }[] = $state([]);

	let activeSkills = $derived(Object.values(activeSkillsData.activeSkills));
	let selectOptions = $derived(
		activeSkills
			.sort((a, b) => a.details.element.localeCompare(b.details.element))
			.map((skill) => ({
				value: skill.id,
				label: skill.localized_name
			}))
	);

	const unlearnedSkills = $derived(
		selectOptions.filter((uskill) => !learnedSkills.some((skill) => skill.id === uskill.value))
	);

	async function getActiveSkillIcon(skillId: string): Promise<string | undefined> {
		const skill = activeSkills.find((s) => s.id === skillId);
		if (!skill || skill.localized_name === 'None') return undefined;
		const activeSkill = skill as ActiveSkill;
		const element = elementsData.elements[activeSkill.details.element];
		if (!element) return undefined;
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${element.icon}.webp`);
	}

	function handleAddSkill() {
		if (selectedSkill && !learnedSkills.some((skill) => skill.id === selectedSkill)) {
			learnedSkills = [...learnedSkills, { id: selectedSkill }];
			selectedSkill = '';
		}
	}

	function handleLearnType() {
		const palData = palsData.getPalData(pal.character_key);
		if (!palData) return;

		const elementSkills = activeSkills
			.filter((skill) => {
				const matchesElement = palData.element_types.some((type) => skill.details.element === type);
				if (
					matchesElement &&
					skill.id.toLowerCase().includes(`unique_${pal.character_key.toLowerCase()}`)
				) {
					return true;
				}
				if (matchesElement && !skill.id.toLowerCase().includes('unique_')) {
					return true;
				}
				return false;
			})
			.map((item) => item.id)
			.filter((skillId) => !learnedSkills.some((skill) => skill.id === skillId))
			.map((skillId) => ({ id: skillId }));

		learnedSkills = [...learnedSkills, ...elementSkills];
	}

	function handleLearnAll() {
		const allSkillIds = selectOptions
			.filter((item) => !item.value.includes('Unique'))
			.map((item) => item.value);

		learnedSkills = allSkillIds.map((skillId) => ({ id: skillId }));
	}

	function handleRemoveSkill(skill: { id: string }) {
		learnedSkills = learnedSkills.filter((s) => s.id !== skill.id);
	}

	function handleClear() {
		learnedSkills = [];
	}

	function handleSave() {
		closeModal(learnedSkills.map((skill) => skill.id));
	}

	$effect(() => {
		learnedSkills = pal.learned_skills.map((skillId: string) => ({ id: skillId }));
	});
</script>

<Card class="min-w-[calc(100vw/3)]">
	<h3 class="h3">Edit Learned Skills</h3>
	<div class="mt-4 flex items-center space-x-2">
		<Combobox options={unlearnedSkills} bind:value={selectedSkill}>
			{#snippet selectOption(option)}
				{#await getActiveSkillIcon(option.value) then icon}
					{@const activeSkill = activeSkills.find((s) => s.id === option.value)}
					<div class="grid grid-cols-[auto_1fr_auto] items-center gap-2">
						<img src={icon} alt={option.label} class="h-6 w-6" />
						<div class="flex flex-col">
							<span class="truncate">{option.label}</span>
							<span class="text-xs">{activeSkill?.description}</span>
						</div>
						<div class="flex items-center space-x-1 justify-self-start">
							<TimerReset class="h-4 w-4" />
							<span class="font-bold">{activeSkill?.details.cool_time}</span>
							<span class="text-xs">Pwr</span>
							<span class="font-bold">{activeSkill?.details.power}</span>
						</div>
					</div>
				{/await}
			{/snippet}
		</Combobox>
		<Tooltip position="right">
			<button
				class="btn preset-filled-primary-500 hover:preset-tonal-secondary p-2"
				onclick={handleAddSkill}
			>
				<Plus />
			</button>
			{#snippet popup()}
				<span>Add Skill</span>
			{/snippet}
		</Tooltip>
	</div>

	<div class="mt-4">
		{#if learnedSkills.length > 0}
			<List
				bind:items={learnedSkills}
				listClass="max-h-60 overflow-y-auto"
				canSelect={false}
				multiple={false}
				idKey="id"
			>
				{#snippet listHeader()}
					<div>
						<span class="font-bold">Skills</span>
					</div>
				{/snippet}
				{#snippet listItem(skill)}
					{#await getActiveSkillIcon(skill.id) then icon}
						{@const activeSkill = activeSkills.find((s) => s.id === skill.id)}
						<div class="grid grid-cols-[auto_1fr_auto] items-center gap-2">
							<img src={icon} alt={activeSkill?.localized_name} class="h-6 w-6" />
							<div class="flex flex-col">
								<span class="truncate">{activeSkill?.localized_name}</span>
								<span class="text-xs">{activeSkill?.description}</span>
							</div>
						</div>
					{/await}
				{/snippet}
				{#snippet listItemActions(skill)}
					<button class="btn hover:bg-error-500/25 p-2" onclick={() => handleRemoveSkill(skill)}>
						<Trash size={16} />
					</button>
				{/snippet}
				{#snippet listItemPopup(skill)}
					{@const activeSkill = activeSkills.find((s) => s.id === skill.id)}
					<div class="flex items-center space-x-1 justify-self-start">
						<TimerReset class="h-4 w-4" />
						<span class="font-bold">{activeSkill?.details.cool_time}</span>
						<span class="text-xs">Pwr</span>
						<span class="font-bold">{activeSkill?.details.power}</span>
					</div>
				{/snippet}
			</List>
		{:else}
			<div class="flex w-full items-center justify-center space-x-2">
				<span class="text-2xl font-semibold">No skills learned</span>
				<img src={staticIcons.sadIcon} alt="Sad face" class="h-12 w-12" />
			</div>
		{/if}
	</div>

	<div class="mt-4 flex justify-end space-x-2">
		<Tooltip position="bottom">
			<button class="btn hover:bg-secondary-500/25 px-2" onclick={handleLearnType}>
				<Brain />
			</button>
			{#snippet popup()}
				<span>Learn All Skills<br />Matching Pal Type</span>
			{/snippet}
		</Tooltip>
		<Tooltip position="bottom">
			<button class="btn hover:bg-secondary-500/25 px-2" onclick={handleLearnAll}>
				<BicepsFlexed />
			</button>
			{#snippet popup()}
				<span>Learn All Skills</span>
			{/snippet}
		</Tooltip>
		<Tooltip position="bottom">
			<button class="btn hover:bg-secondary-500/25 px-2" onclick={handleClear}>
				<Delete />
			</button>
			{#snippet popup()}
				<span>Clear</span>
			{/snippet}
		</Tooltip>
		<Tooltip position="bottom">
			<button class="btn hover:bg-secondary-500/25 px-2" onclick={handleSave}>
				<Save />
			</button>
			{#snippet popup()}
				<span>Save</span>
			{/snippet}
		</Tooltip>
		<Tooltip position="bottom">
			<button class="btn hover:bg-secondary-500/25 px-2" onclick={() => closeModal(null)}>
				<X />
			</button>
			{#snippet popup()}
				<span>Cancel</span>
			{/snippet}
		</Tooltip>
	</div>
</Card>
