<script lang="ts">
	import { Tabs } from '@skeletonlabs/skeleton-svelte';
	import type { ValueChangeDetails } from '@zag-js/tabs';
	import {
		ConvertTab,
		GamepassBrowserTab,
		SteamIdTab,
		TransferTab,
		UidSwapTab
	} from '$components/tools';
	import * as m from '$i18n/messages';
	import { ArrowRightLeft, Gamepad2, Hash, Repeat, Upload } from 'lucide-svelte';

	let activeTab = $state('convert');
</script>

<div class="flex min-h-screen w-full flex-col items-center py-8">
	<div class="flex w-full max-w-3xl flex-col gap-8">
		<Tabs value={activeTab} onValueChange={(e: ValueChangeDetails) => (activeTab = e.value)}>
			{#snippet list()}
				<Tabs.Control value="convert">
					{#snippet lead()}<ArrowRightLeft size={16} />{/snippet}
					{m.tools_tab_convert()}
				</Tabs.Control>
				<Tabs.Control value="gamepass">
					{#snippet lead()}<Gamepad2 size={16} />{/snippet}
					{m.tools_tab_gamepass()}
				</Tabs.Control>
				<Tabs.Control value="steamid">
					{#snippet lead()}<Hash size={16} />{/snippet}
					{m.tools_tab_steam_id()}
				</Tabs.Control>
				<Tabs.Control value="uidswap">
					{#snippet lead()}<Repeat size={16} />{/snippet}
					{m.tools_tab_uid_swap()}
				</Tabs.Control>
				<Tabs.Control value="transfer">
					{#snippet lead()}<Upload size={16} />{/snippet}
					{m.tools_tab_transfer()}
				</Tabs.Control>
			{/snippet}
			{#snippet content()}
				<Tabs.Panel value="convert"><ConvertTab /></Tabs.Panel>
				<Tabs.Panel value="gamepass">
					<GamepassBrowserTab active={activeTab === 'gamepass'} />
				</Tabs.Panel>
				<Tabs.Panel value="steamid"><SteamIdTab /></Tabs.Panel>
				<Tabs.Panel value="uidswap"><UidSwapTab /></Tabs.Panel>
				<Tabs.Panel value="transfer"><TransferTab /></Tabs.Panel>
			{/snippet}
		</Tabs>
	</div>
</div>
