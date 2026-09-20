<script lang="ts" module>
	export type SectionTab = { id: string; label: string };
</script>

<script lang="ts">
	let {
		tabs,
		active = $bindable(),
		label
	}: { tabs: SectionTab[]; active: string; label: string } = $props();

	let buttons: (HTMLButtonElement | null)[] = $state([]);

	function activate(index: number) {
		const tab = tabs[index];
		if (!tab) return;
		active = tab.id;
		buttons[index]?.focus();
	}

	function onKeydown(event: KeyboardEvent) {
		const index = tabs.findIndex((tab) => tab.id === active);
		if (index === -1) return;

		switch (event.key) {
			case 'ArrowRight':
				event.preventDefault();
				activate((index + 1) % tabs.length);
				break;
			case 'ArrowLeft':
				event.preventDefault();
				activate((index - 1 + tabs.length) % tabs.length);
				break;
			case 'Home':
				event.preventDefault();
				activate(0);
				break;
			case 'End':
				event.preventDefault();
				activate(tabs.length - 1);
				break;
		}
	}
</script>

<div
	class="border-surface-700/40 bg-surface-900/40 flex shrink-0 border-b"
	role="tablist"
	aria-label={label}
>
	{#each tabs as tab, index (tab.id)}
		<button
			bind:this={buttons[index]}
			type="button"
			role="tab"
			aria-selected={tab.id === active}
			tabindex={tab.id === active ? 0 : -1}
			class="h-11 flex-1 text-sm transition-colors"
			class:font-bold={tab.id === active}
			class:text-primary-300={tab.id === active}
			class:border-primary-400={tab.id === active}
			class:border-b-2={tab.id === active}
			class:text-surface-300={tab.id !== active}
			onclick={() => (active = tab.id)}
			onkeydown={onKeydown}
		>
			{tab.label}
		</button>
	{/each}
</div>
