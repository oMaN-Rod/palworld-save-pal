<script lang="ts">
	import { cn } from '$theme';

	let {
		background = 'preset-filled-surface-100-900',
		rounded = 'rounded',
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

<div class={cn('card rounded-sm', background, rounded, _class, padding)}>
	{@render children()}
</div>
