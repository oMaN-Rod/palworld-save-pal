<script lang="ts">
	import { cn } from '$theme';

	let {
		background = '',
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

<div class={cn('card', background, rounded, padding, _class)}>
	{@render children()}
</div>
