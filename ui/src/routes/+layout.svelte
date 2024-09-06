<script lang="ts">
	import '../app.css';
	import { NavBar, Toast, Modal } from '$components';

	const { children } = $props();
</script>

<Toast position="bottom-center" />
<Modal>
	<div class="flex h-screen w-full overflow-hidden">
		<NavBar />
		<main class="flex-1 overflow-hidden">
			{@render children()}
		</main>
	</div>
</Modal>
