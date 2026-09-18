<script lang="ts">
	import { cn } from '$theme';

	let {
		background = '',
		rounded = 'rounded-sm',
		padding = 'p-4',
		hover = false,
		class: _class = '',
		children
	} = $props<{
		background?: string;
		rounded?: string;
		padding?: string;
		hover?: boolean;
		class?: string;
		children: any;
	}>();
</script>

<div class={cn('bg-surface-900 rounded', hover ? 'border border-secondary-500' : '', background, rounded, padding, _class)}>
	{@render children()}
</div>
