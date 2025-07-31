<script lang="ts">
	import { Card, Tooltip, Combobox, List, TooltipButton } from '$components/ui';
	import { type Pal, type SelectOption, type SkillType } from '$types';
	import { Save, X, Delete, Trash, Plus } from 'lucide-svelte';
	import { activeSkillsData, passiveSkillsData } from '$lib/data';
	import { ActiveSkillOption, PassiveSkillOption } from '$components';

	let {
		title = '',
		type = 'Active',
		pal,
		closeModal
	} = $props<{
		title?: string;
		type?: SkillType;
		pal?: Pal;
		closeModal: (values: string[] | null) => void;
	}>();

	let values: string[] = $state([]);
	const selectOptions: SelectOption[] = $derived.by(() => {
		if (type === 'Active') {
			return Object.values(activeSkillsData.activeSkills)
				.filter((skill) => {
					if (skill.id.toLowerCase().includes(`unique_${pal.character_key.toLowerCase()}`)) {
						return true;
					}
					if (!skill.id.toLowerCase().includes('unique_')) {
						return true;
					}
					return false;
				})
				.filter((aSkill) => !Object.values(pal.active_skills).some((skill) => skill === aSkill.id))
				.filter((aSkill) => !values.some((v) => v === aSkill.id))
				.sort((a, b) => a.details.element.localeCompare(b.details.element))
				.map((s) => ({
					value: s.id,
					label: s.localized_name
				}));
		} else {
			return Object.values(passiveSkillsData.passiveSkills)
				.filter((pSkill) => !Object.values(pal.passive_skills).some((p) => p === pSkill.id))
				.filter((pSkill) => !values.some((v) => v === pSkill.id))
				.sort((a, b) => b.details.rank - a.details.rank)
				.map((s) => ({
					value: s.id,
					label: s.localized_name
				}));
		}
	});

	function handleClear() {
		closeModal('Empty');
	}

	function handleClose(values: string[] | null) {
		closeModal(values);
	}
	$inspect(values);
</script>

<Card class="min-w-[calc(100vw/3)]">
	<h3 class="h3">{title}</h3>
	<div class="flex w-full items-center">
		<Combobox
			label={`${type} Skills`}
			options={selectOptions}
			placeholder={`Choose ${type} Skills...`}
			onChange={(value) => values.push(value as string)}
		>
			{#snippet selectOption(option)}
				{#if type === 'Active'}
					<ActiveSkillOption {option} />
				{:else if type === 'Passive'}
					<PassiveSkillOption {option} />
				{/if}
			{/snippet}
		</Combobox>
		<TooltipButton
			onclick={() => values.push(...selectOptions.map((option) => option.value))}
			popupLabel="Add all Skills"
		>
			<Plus />
		</TooltipButton>
	</div>

	{#if values.length > 0}
		<List items={values} listClass="max-h-60 overflow-y-auto" canSelect={false}>
			{#snippet listHeader()}
				<div>
					<span class="font-bold">{type} Skills</span>
				</div>
			{/snippet}
			{#snippet listItem(skill)}
				{#if type === 'Passive'}
					{@const passiveSkill = passiveSkillsData.passiveSkills[skill]}
					<PassiveSkillOption option={{ label: passiveSkill.localized_name, value: skill }} />
				{:else if type === 'Active'}
					{@const activeSkill = activeSkillsData.getByKey(skill)}
					<ActiveSkillOption
						option={{ label: activeSkill?.localized_name || skill, value: skill }}
					/>
				{/if}
			{/snippet}
			{#snippet listItemActions(skill)}
				<button
					class="btn hover:bg-error-500/25 p-2"
					onclick={() => (values = values.filter((s) => s !== skill))}
				>
					<Trash size={16} />
				</button>
			{/snippet}
			{#snippet listItemPopup(skill)}
				{#if type === 'Passive'}
					{@const passiveSkill = passiveSkillsData.passiveSkills[skill]}
					<div class="flex grow flex-col">
						<span class="grow truncate">{passiveSkill.localized_name}</span>
						<span class="text-xs">{passiveSkill?.description}</span>
					</div>
				{:else if type === 'Active'}
					{@const activeSkill = activeSkillsData.getByKey(skill)}
					<div class="flex grow flex-col">
						<span class="grow truncate">{activeSkill?.localized_name || skill}</span>
						<span class="text-xs">{activeSkill?.description}</span>
					</div>
				{/if}
			{/snippet}
		</List>
	{/if}

	<div class="mt-2 flex flex-row items-center space-x-2">
		<Tooltip position="bottom">
			<button class="btn hover:bg-secondary-500 px-2" onclick={handleClear}>
				<Delete />
			</button>
			{#snippet popup()}
				<span>Clear</span>
			{/snippet}
		</Tooltip>
		<Tooltip position="bottom">
			<button class="btn hover:bg-secondary-500 px-2" onclick={() => handleClose(values)}>
				<Save />
			</button>
			{#snippet popup()}
				<span>Save</span>
			{/snippet}
		</Tooltip>
		<Tooltip position="bottom">
			<button class="btn hover:bg-secondary-500 px-2" onclick={() => handleClose(null)}>
				<X />
			</button>
			{#snippet popup()}
				<span>Cancel</span>
			{/snippet}
		</Tooltip>
	</div>
</Card>
