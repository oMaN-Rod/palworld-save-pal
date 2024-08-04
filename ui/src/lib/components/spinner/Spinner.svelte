<script lang="ts">
	import { cn } from '$theme';
	let {
		from = 'from-transparent',
		to = 'to-tertiary-500',
		via = 'via-secondary-500',
		size = 'size-10',
		...additionalProps
	} = $props<{
		from?: string;
		to?: string;
		via?: string;
		size?: string;
		[key: string]: any;
	}>();
</script>

<figure
	class={cn('bg-gradient-conic aspect-square animate-spin rounded-full ', from, to, via, size)}
	{...additionalProps}
></figure>
