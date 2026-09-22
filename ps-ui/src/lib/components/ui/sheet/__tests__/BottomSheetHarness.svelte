<script lang="ts">
	import BottomSheet from '../BottomSheet.svelte';
	import type { SheetSnap } from '../sheetSnap';

	let {
		open = false,
		snap = $bindable('peek' as SheetSnap),
		modal = true,
		onClose
	}: {
		open?: boolean;
		snap?: SheetSnap;
		modal?: boolean;
		onClose: () => void;
	} = $props();
</script>

<BottomSheet {open} bind:snap {modal} title="Map layers" {onClose}>
	<p>sheet body</p>
</BottomSheet>
