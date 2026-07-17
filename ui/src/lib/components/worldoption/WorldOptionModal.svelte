<script lang="ts">
	import { Button, Card, Tooltip } from '$components/ui';
	import { Save, X } from 'lucide-svelte';
	import { onMount } from 'svelte';
	import { focusModal } from '$utils/modalUtils';
	import { Accordion } from '@skeletonlabs/skeleton-svelte';
	import { cn } from '$theme';
	import {
		fieldFor,
		groupsForTab,
		worldOptionTabs,
		type WoTab,
		type WoField
	} from './worldOptionFields';
	import WorldOptionField from './WorldOptionField.svelte';
	import type { WoValue } from './WorldOptionField.svelte';

	type Entry = { key: string; kind: string; value: WoValue };

	let {
		title = 'World Options',
		settings = [],
		technologies = [],
		closeModal
	}: {
		title?: string;
		/** PRESENT settings only, from the backend. */
		settings?: Entry[];
		technologies?: string[];
		closeModal: (value: { entries: { key: string; value: WoValue }[] } | null) => void;
	} = $props();

	let modalContainer: HTMLDivElement;
	let activeTab: WoTab = $state('general');

	// Keys the save actually carries. Everything else renders its default, badged
	// "not set", and is only written if the user touches it.
	const present = $derived(
		new Map<string, WoValue>(settings.map((entry) => [entry.key, entry.value]))
	);

	let edited = $state<Record<string, WoValue>>({});

	function valueFor(field: WoField): WoValue {
		if (field.key in edited) return edited[field.key];
		const stored = present.get(field.key);
		return stored !== undefined ? stored : field.default;
	}

	function isSet(field: WoField): boolean {
		return present.has(field.key) || field.key in edited;
	}

	function setValue(key: string, value: WoValue) {
		edited[key] = value;
	}

	function sameValue(a: WoValue, b: WoValue): boolean {
		if (Array.isArray(a) && Array.isArray(b)) {
			if (a.length !== b.length) return false;
			const sortedA = [...a].sort();
			const sortedB = [...b].sort();
			return sortedA.every((item, i) => item === sortedB[i]);
		}
		return a === b;
	}

	// The patch IS the minimal diff: an edit back to the stored value (or, for an
	// absent key, back to the field default) drops out.
	const patch = $derived.by(() =>
		Object.entries(edited)
			.filter(([key, value]) => {
				const stored = present.get(key);
				const baseline = stored !== undefined ? stored : fieldFor(key)?.default;
				// If we can't resolve a baseline (unknown key), keep it rather than silently drop.
				return baseline === undefined || !sameValue(baseline, value);
			})
			.map(([key, value]) => ({ key, value }))
	);

	function handleSubmit() {
		if (patch.length === 0) {
			closeModal(null);
			return;
		}
		closeModal({ entries: patch });
	}

	onMount(() => {
		focusModal(modalContainer);
	});
</script>

<div bind:this={modalContainer}>
	<Card class="max-w-[750px] min-w-[650px]">
		<div class="mb-4 flex items-center gap-3">
			<h3 class="h3">{title}</h3>
			{#if patch.length > 0}
				<span class="text-secondary-400 text-xs">
					{patch.length} change{patch.length === 1 ? '' : 's'} pending
				</span>
			{/if}
		</div>

		<div class="border-surface-700 mb-4 flex gap-1 border-b">
			{#each worldOptionTabs as tab (tab.id)}
				<button
					class={cn(
						'px-4 py-2 text-sm font-medium transition-colors',
						activeTab === tab.id
							? 'text-secondary-400 border-secondary-400 border-b-2'
							: 'text-surface-400 hover:text-surface-200 border-b-2 border-transparent'
					)}
					onclick={() => (activeTab = tab.id)}
				>
					{tab.label}
				</button>
			{/each}
		</div>

		<div class="max-h-[60vh] overflow-y-auto pr-1">
			<Accordion collapsible>
				{#each groupsForTab(activeTab) as group (group.title)}
					<Accordion.Item
						value={group.title}
						base="rounded-sm bg-surface-900"
						controlHover="hover:bg-secondary-500/25"
					>
						{#snippet control()}
							<span class="text-sm font-medium">{group.title}</span>
						{/snippet}
						{#snippet panel()}
							<div class="grid grid-cols-2 gap-3 p-3">
								{#each group.keys as field (field.key)}
									<WorldOptionField
										{field}
										value={valueFor(field)}
										isSet={isSet(field)}
										{technologies}
										onchange={setValue}
									/>
								{/each}
							</div>
						{/snippet}
					</Accordion.Item>
				{/each}
			</Accordion>
		</div>

		<div class="mt-4 flex justify-end gap-2">
			<Tooltip position="bottom">
				<Button
					variant="ghost"
					size="icon"
					onclick={handleSubmit}
					disabled={patch.length === 0}
					data-modal-primary
				>
					<Save />
				</Button>
				{#snippet popup()}
					<span>Apply changes</span>
				{/snippet}
			</Tooltip>
			<Tooltip position="bottom">
				<Button variant="ghost" size="icon" onclick={() => closeModal(null)}>
					<X />
				</Button>
				{#snippet popup()}
					<span>Cancel</span>
				{/snippet}
			</Tooltip>
		</div>
	</Card>
</div>
