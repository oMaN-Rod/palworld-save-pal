<script lang="ts">
	import { cn } from '$theme';
	import { Tooltip } from '$components/ui';

	let {
		value = $bindable(0),
		max = $bindable(100),
		height = 'h-2',
		width = 'w-full',
		rounded = 'rounded-none',
		color = 'primary',
		dividend = 1,
		...additionalProps
	} = $props<{
		value: number;
		max: number;
		height?: string;
		width?: string;
		rounded?: string;
		color?: string;
		dividend?: number;
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
				progressBg = 'bg-surface-500';
				break;
		}
	});
</script>

<Tooltip>
	<div class={cn('bg-surface-800', width, height, rounded)} {...additionalProps}>
		<div
			class={cn(
				'h-full overflow-visible whitespace-nowrap pl-1 align-middle text-sm font-bold transition-all',
				progressBg,
				rounded
			)}
			style={`width: ${progressPercentage}%`}
		>
			{value.toFixed(0) / dividend} / {max / dividend}
		</div>
	</div>
	{#snippet popup()}
		<span>{progressPercentage.toFixed(1)}%</span>
	{/snippet}
</Tooltip>

<progress {value} {max} class="sr-only">
	{progressPercentage}%
</progress>
