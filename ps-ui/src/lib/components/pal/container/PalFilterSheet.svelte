<script lang="ts">
	import type { Snippet } from 'svelte';

	import BottomSheet from '$components/ui/sheet/BottomSheet.svelte';
	import * as m from '$i18n/messages';

	let {
		open = $bindable(false),
		onClose,
		children
	}: {
		open?: boolean;
		onClose: () => void;
		children: Snippet;
	} = $props();
</script>

<BottomSheet {open} title={m.filter({ count: 2 })} {onClose} snaps={['peek', 'tall']}>
	{@render children()}
</BottomSheet>
