<script lang="ts">
	import BottomSheet from '../BottomSheet.svelte';
	import type { SheetSnap } from '../sheetSnap';

	let {
		open = false,
		snap = $bindable('peek' as SheetSnap),
		onClose
	}: {
		open?: boolean;
		snap?: SheetSnap;
		onClose: () => void;
	} = $props();
</script>

<BottomSheet {open} bind:snap title="Map layers" {onClose}>
	<p>sheet body</p>
</BottomSheet>
