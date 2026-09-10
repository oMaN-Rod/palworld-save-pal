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

<div class={cn('card', hover ? 'card-hover' : '', background, rounded, padding, _class)}>
	{@render children()}
</div>
