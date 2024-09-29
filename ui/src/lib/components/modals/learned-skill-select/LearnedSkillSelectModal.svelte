<script lang="ts">
	import { Card, Tooltip, Combobox, List } from '$components/ui';
	import type { ActiveSkill, SelectOption } from '$types';
	import { Plus, Save, X, Trash, TimerReset, Delete } from 'lucide-svelte';
	import { activeSkillsData, elementsData } from '$lib/data';
	import { ASSET_DATA_PATH } from '$lib/constants';
	import { assetLoader } from '$utils';

	let { closeModal, pal } = $props<{
		closeModal: (value: any) => void;
		pal: any;
	}>();

	let selectOptions: SelectOption[] = $state([]);
	let activeSkills: ActiveSkill[] = $state([]);
	let selectedSkill: string = $state('');
	let learnedSkills: string[] = $state([]);

	$effect(() => {
		loadSkillOptions();
		learnedSkills = [...pal.learned_skills];
	});

	async function loadSkillOptions() {
		activeSkills = await activeSkillsData.getActiveSkills();
		selectOptions = activeSkills.map((skill) => ({
			value: skill.id,
			label: skill.name
		}));
	}

	async function getActiveSkillIcon(skillId: string): Promise<string | undefined> {
		const skill = activeSkills.find((s) => s.id === skillId);
		if (!skill || skill.name === 'None') return undefined;
		const activeSkill = skill as ActiveSkill;
		const elementObj = await elementsData.searchElement(activeSkill.details.type);
		if (!elementObj) return undefined;
		const iconPath = `${ASSET_DATA_PATH}/img/elements/${elementObj.icon}.png`;
		const icon = await assetLoader.loadImage(iconPath, true);
		return icon;
	}

	function handleAddSkill() {
		if (selectedSkill && !learnedSkills.includes(selectedSkill)) {
			learnedSkills = [...learnedSkills, selectedSkill];
			selectedSkill = '';
		}
	}

	function handleRemoveSkill(skill: string) {
		learnedSkills = learnedSkills.filter((s) => s !== skill);
	}

	function handleClear() {
		learnedSkills = [];
	}

	function handleSave() {
		closeModal(learnedSkills);
	}
</script>

<Card class="bg-surface-500 min-w-[calc(100vw/3)]">
	<h3 class="h3">Edit Learned Skills</h3>
	<div class="mt-4 flex items-center space-x-2">
		<Combobox options={selectOptions} bind:value={selectedSkill}>
			{#snippet selectOption(option)}
				{#await getActiveSkillIcon(option.value) then icon}
					{@const activeSkill = activeSkills.find((s) => s.id === option.value)}
					<div class="grid grid-cols-[auto_1fr_auto] items-center gap-2">
						{#if icon}
							<enhanced:img src={icon} alt={option.label} class="h-6 w-6"></enhanced:img>
						{:else}
							<div class="w-6"></div>
						{/if}
						<div class="flex flex-col">
							<span class="truncate">{option.label}</span>
							<span class="text-xs">{activeSkill?.description}</span>
						</div>
						<div class="flex items-center space-x-1 justify-self-start">
							<TimerReset class="h-4 w-4" />
							<span class="font-bold">{activeSkill?.details.ct}</span>
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
		<List
			bind:items={learnedSkills}
			listClass="max-h-60 overflow-y-auto"
			canSelect={false}
			multiple={false}
		>
			{#snippet listHeader()}
				<div>
					<span class="font-bold">Skills</span>
				</div>
			{/snippet}
			{#snippet listItem(skill)}
				{#await getActiveSkillIcon(skill) then icon}
					{@const activeSkill = activeSkills.find((s) => s.id === skill)}
					<div class="grid grid-cols-[auto_1fr_auto] items-center gap-2">
						{#if icon}
							<enhanced:img src={icon} alt={activeSkill?.name} class="h-6 w-6"></enhanced:img>
						{:else}
							<div class="w-6"></div>
						{/if}
						<div class="flex flex-col">
							<span class="truncate">{activeSkill?.name}</span>
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
				{@const activeSkill = activeSkills.find((s) => s.id === skill)}
				<div class="flex items-center space-x-1 justify-self-start">
					<TimerReset class="h-4 w-4" />
					<span class="font-bold">{activeSkill?.details.ct}</span>
					<span class="text-xs">Pwr</span>
					<span class="font-bold">{activeSkill?.details.power}</span>
				</div>
			{/snippet}
		</List>
	</div>

	<div class="mt-4 flex justify-end space-x-2">
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
