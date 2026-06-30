<script lang="ts">
	import { Tabs } from '@skeletonlabs/skeleton-svelte';
	import type { ValueChangeDetails } from '@zag-js/tabs';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';
	import PlayersTable from '$lib/components/bulk/PlayersTable.svelte';

	let selectedTab = $state('players');
	let playerSelection = $state(new Set<string>());
</script>

<div class="flex h-full flex-col gap-4 p-4">
	<Tabs value={selectedTab} onValueChange={(e: ValueChangeDetails) => (selectedTab = e.value)}>
		{#snippet list()}
			<Tabs.Control value="players">{c.players}</Tabs.Control>
			<Tabs.Control value="pals">{c.pals}</Tabs.Control>
			<Tabs.Control value="guilds">{c.guilds}</Tabs.Control>
		{/snippet}
		{#snippet content()}
			<Tabs.Panel value="players">
				<PlayersTable bind:selected={playerSelection} />
			</Tabs.Panel>
			<Tabs.Panel value="pals">
				<p class="text-surface-300">{c.pals}</p>
			</Tabs.Panel>
			<Tabs.Panel value="guilds">
				<p class="text-surface-300">{c.guilds}</p>
			</Tabs.Panel>
		{/snippet}
	</Tabs>
</div>
