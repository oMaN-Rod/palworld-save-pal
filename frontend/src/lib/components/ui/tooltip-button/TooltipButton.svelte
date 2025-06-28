<script lang="ts">
	import { Tooltip } from '$components/ui';

	let {
		icon,
		children,
		buttonClass,
		popupLabel,
		position = 'bottom',
		...additionalProps
	} = $props<{
		children: any;
		buttonClass?: string;
		popupLabel: string;
		position?: 'top' | 'bottom' | 'left' | 'right';
		[key: string]: any;
	}>();
</script>

<Tooltip {position}>
	<button class="btn p-2 {buttonClass}" {...additionalProps}>
		{@render children()}
	</button>
	{#snippet popup()}
		{popupLabel}
	{/snippet}
</Tooltip>
