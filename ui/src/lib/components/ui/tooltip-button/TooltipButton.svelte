<script lang="ts">
	import { Tooltip } from '$components/ui';

	let { icon, children, buttonClass, popupLabel, ...additionalProps } = $props<{
		children: any;
		buttonClass?: string;
		popupLabel: string;
		[key: string]: any;
	}>();
</script>

<Tooltip>
	<button class="btn hover:preset-tonal-secondary p-2 {buttonClass}" {...additionalProps}>
		{@render children()}
	</button>
	{#snippet popup()}
		{popupLabel}
	{/snippet}
</Tooltip>
