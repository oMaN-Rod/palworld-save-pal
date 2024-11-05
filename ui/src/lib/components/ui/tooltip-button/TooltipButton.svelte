<script lang="ts">
	import { Tooltip } from '$components/ui';

	let { icon, children, popupLabel, ...additionalProps } = $props<{
		children: any;
		popupLabel: string;
		[key: string]: any;
	}>();
</script>

<Tooltip>
	<button class="btn hover:preset-tonal-secondary p-2" {...additionalProps}>
		{@render children()}
	</button>
	{#snippet popup()}
		{popupLabel}
	{/snippet}
</Tooltip>
