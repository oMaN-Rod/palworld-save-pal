<script lang="ts">
	import { cn } from '$theme';
	import { Tooltip } from '$components/ui';

	let {
		value = $bindable(0),
		max = $bindable(100),
		height = 'h-2',
		rounded = 'rounded-none',
		color = '',
		dividend = 1,
		showLabel = true,
		...additionalProps
	} = $props<{
		value: number;
		max: number;
		height?: string;
		width?: string;
		rounded?: string;
		color?: string;
		dividend?: number;
		showLabel?: boolean;
		[key: string]: any;
	}>();

	const progressPercentage = $derived((value / max) * 100);
	let progressBg: string = $state('');

	$effect(() => {
		if (max != 0 && value > max) {
			value = max;
		}
	});

	$effect(() => {
		switch (color) {
			case 'primary':
				progressBg = 'bg-primary-500';
				break;
			case 'secondary':
				progressBg = 'bg-secondary-500';
				break;
			case 'tertiary':
				progressBg = 'bg-tertiary-500';
				break;
			case 'success':
				progressBg = 'bg-success-500';
				break;
			case 'warning':
				progressBg = 'bg-warning-500';
				break;
			case 'error':
				progressBg = 'bg-error-500';
				break;
			case 'orange':
				progressBg = 'bg-orange-500';
				break;
			case 'green':
				progressBg = 'bg-green-500';
				break;
			default:
				progressBg = 'bg-[#34f1fd]';
				break;
		}
	});
</script>

<Tooltip baseClass="w-full">
	<div class={cn('bg-surface-800', height, rounded)} {...additionalProps}>
		<div
			class={cn(
				'h-full overflow-visible whitespace-nowrap pl-1 text-start align-middle text-sm font-bold transition-all',
				progressBg,
				rounded
			)}
			style={`width: ${progressPercentage}%`}
		>
			{#if showLabel}
				{value.toFixed(0) / dividend} / {max / dividend}
			{/if}
		</div>
	</div>
	{#snippet popup()}
		<span>{value.toFixed(0) / dividend} / {max / dividend} ({progressPercentage.toFixed(1)}%)</span>
	{/snippet}
</Tooltip>

<progress {value} {max} class="sr-only">
	{progressPercentage}%
</progress>
