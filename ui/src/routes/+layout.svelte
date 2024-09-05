<script lang="ts">
	import '../app.css';
	import { NavBar, Toast, Modal } from '$components';

	const { children } = $props();
</script>

<Toast position="bottom-center" />
<Modal>
	<div class="flex h-[100vh] w-full flex-row overflow-hidden">
		<NavBar />
		{@render children()}
	</div>
</Modal>
