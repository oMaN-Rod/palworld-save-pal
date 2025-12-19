<script lang="ts">
	import { goto } from "$app/navigation";
	import { getAppState } from "$states";
	import { onMount } from "svelte";

    const appState = getAppState();

    onMount(() => {
        if (appState.selectedPlayer) {
            goto(`/edit/player`);
        }
    })
</script>

<div class="flex w-full flex-col items-center justify-center">
    <h2 class="h2">Select a Player to edit 🚀</h2>
</div>