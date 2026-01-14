<script lang="ts">
	import { cn } from '$theme';

	let {
		background = 'preset-filled-surface-100-900',
		rounded = 'rounded-sm',
		padding = 'p-4',
		class: _class = '',
		children
	} = $props<{
		background?: string;
		rounded?: string;
		padding?: string;
		class?: string;
		children: any;
	}>();
</script>

<div class={cn('card rounded-xs', background, rounded, padding, _class)}>
	{@render children()}
</div>
