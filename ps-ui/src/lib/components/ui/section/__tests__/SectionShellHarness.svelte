<script lang="ts">
	import SectionShell from '../SectionShell.svelte';
	import type { SectionDef, SectionPresentation } from '../sectionShell';

	let {
		presentation,
		active = $bindable('active')
	}: { presentation: SectionPresentation; active?: string } = $props();

	const sections: SectionDef[] = [
		{ id: 'active', title: 'Active Skills', body: activeBody, group: 'primary' },
		{ id: 'passive', title: 'Passive Skills', body: passiveBody, group: 'primary' },
		{ id: 'talents', title: 'Talents', body: talentsBody, group: 'primary' }
	];
</script>

{#snippet activeBody()}
	<span>active body</span>
{/snippet}

{#snippet passiveBody()}
	<span>passive body</span>
{/snippet}

{#snippet talentsBody()}
	<span>talents body</span>
{/snippet}

<SectionShell {sections} {presentation} bind:active label="Sections" idPrefix="shell" />
