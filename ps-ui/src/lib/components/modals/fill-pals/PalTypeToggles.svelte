<script lang="ts">
	import { Tooltip } from '$components/ui';
	import * as m from '$i18n/messages';
	import { c } from '$lib/utils/commonTranslations';
	import { Switch } from '@skeletonlabs/skeleton-svelte';
	import type { CheckedChangeDetails } from '@zag-js/switch';
	import type { PalData } from '$types';

	interface Toggle {
		label: string;
		count: number;
		checked: boolean;
		onChange: (checked: boolean) => void;
	}

	let { toggles }: { toggles: Toggle[] } = $props();
</script>

<div class="mt-2 grid grid-cols-2 gap-2 overflow-y-auto p-2">
	{#each toggles as toggle (toggle.label)}
		<Tooltip
			label={m.add_count_pals({ count: toggle.count, type: toggle.label, pals: c.pals })}
			baseClass="flex items-center space-x-2"
		>
			<Switch
				name={toggle.label}
				checked={toggle.checked}
				onCheckedChange={(mode: CheckedChangeDetails) => toggle.onChange(mode.checked)}
			/>
			<span>{toggle.label}</span>
		</Tooltip>
	{/each}
</div>