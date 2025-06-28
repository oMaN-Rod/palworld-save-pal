<script lang="ts">
	import { cn } from '$theme';
	import { staticIcons } from '$types/icons';

	let { size = 'size-10', ...additionalProps } = $props<{
		size?: string;
		[key: string]: any;
	}>();
</script>

<figure class={cn('spinner aspect-square rounded-full', size)} {...additionalProps}>
	<img src={staticIcons.lamball} alt="Loading" class="absolute inset-0 m-auto" />
</figure>

<style>
	.spinner {
		animation: spin 2s linear infinite;
	}

	@keyframes spin {
		from {
			transform: rotate(0deg);
		}
		to {
			transform: rotate(360deg);
		}
	}
</style>
