<script lang="ts">
	import BottomSheet from '../BottomSheet.svelte';

	let { active = false, onClose }: { active?: boolean; onClose: () => void } = $props();
</script>

<!-- Like every call site: a parent block gates the sheet and `open` stays constant. -->
{#if active}
	<BottomSheet open title="Map layers" {onClose}>
		<p>sheet body</p>
	</BottomSheet>
{/if}
