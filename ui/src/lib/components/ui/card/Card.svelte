<script lang="ts">
	import { cn } from '$theme';

	let {
		background = 'preset-filled-surface-100-900',
		rounded = 'rounded',
		class: _class = '',
		children
	} = $props<{
		background?: string;
		rounded?: string;
		class?: string;
		children: any;
	}>();
</script>

<div class={cn('card rounded-sm p-4', background, rounded, _class)}>
	{@render children()}
</div>
