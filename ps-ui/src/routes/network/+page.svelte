<script lang="ts">
	import NetworkPanel from '$lib/components/settings/NetworkPanel.svelte';
	import * as m from '$i18n/messages';
</script>

<div class="mx-auto flex w-full max-w-3xl flex-col gap-4 p-4">
	<h1 class="h1">{m.network_page_title()}</h1>
	<p class="text-surface-400">{m.network_page_desc()}</p>
	<NetworkPanel />
</div>