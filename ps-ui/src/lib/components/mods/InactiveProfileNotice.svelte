<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import type { ModProfile } from '$types';
	import * as m from '$i18n/messages';

	let { profile }: { profile: ModProfile } = $props();
</script>

{#if !profile.is_active}
	<p
		class="border-warning-500/40 bg-warning-500/10 flex items-center gap-2 rounded-sm border p-3 text-sm"
		role="status"
	>
		<Icon icon="tabler:info-circle" size={14} class="text-warning-400 shrink-0" />
		{m.mods_profile_inactive({ name: profile.name })}
	</p>
{/if}
