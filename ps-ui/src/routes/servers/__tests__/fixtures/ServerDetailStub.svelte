<script lang="ts">
	import type { Server } from '$types';

	let { server }: { server: Server } = $props();
</script>

<div data-testid="server-detail">{server.name}</div>
