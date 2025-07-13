<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import { Button } from "$lib/components/ui/button";

  type Pal = {
    instance_id: string;
    character_id: string;
    nickname: string | null;
    level: number;
  };

  type Player = {
    uid: string;
    nickname: string;
    level: number;
    pals: Pal[];
  };

  let players = $state<Player[]>([]);
  let statusMessage = $state(
    "No save file loaded. Click the button to load mock data."
  );
  let isLoading = $state(false);

  async function selectAndLoadFile() {
    isLoading = true;
    statusMessage = "Loading save file...";
    try {
      // NOTE: For testing, we pass a dummy path. The backend uses mock data anyway.
      await invoke("load_save_file", { path: "C:/dummy/path/Level.sav" });
    } catch (e) {
      statusMessage = `Error: ${e}`;
      isLoading = false;
    }
  }

  async function fetchPlayers() {
    try {
      const response = await invoke("graphql", {
        query:
          "{ players { uid nickname level pals { instanceId characterId nickname level } } }",
        operationName: null,
        variables: null,
      });

      // The graphql response nests the data, so we extract it.
      if (response.data && response.data.players) {
        players = response.data.players;
        statusMessage = `Successfully loaded ${players.length} players.`;
      } else {
        throw new Error("Invalid GraphQL response structure");
      }
    } catch (e) {
      statusMessage = `GraphQL Error: ${e}`;
    } finally {
      isLoading = false;
    }
  }

  onMount(() => {
    // Listen for the "save-loaded" event from the Rust backend
    const unlisten = listen("save-loaded", (event) => {
      console.log("Received 'save-loaded' event:", event);
      statusMessage = "Save loaded in backend! Fetching data from GraphQL...";
      fetchPlayers();
    });

    // Cleanup listener when component is destroyed
    return () => {
      unlisten.then((f) => f());
    };
  });
</script>

<div class="container mx-auto">
  <header class="mb-8">
    <h1 class="text-3xl font-bold text-lightest-slate">Save File Dashboard</h1>
    <p class="text-slate">{statusMessage}</p>
  </header>

  <div class="mb-8">
    <Button onclick={selectAndLoadFile} disabled={isLoading}>
      {#if isLoading}
        <svg
          class="animate-spin -ml-1 mr-3 h-5 w-5 text-white"
          xmlns="http://www.w3.org/2000/svg"
          fill="none"
          viewBox="0 0 24 24"
        >
          <circle
            class="opacity-25"
            cx="12"
            cy="12"
            r="10"
            stroke="currentColor"
            stroke-width="4"
          ></circle>
          <path
            class="opacity-75"
            fill="currentColor"
            d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
          ></path>
        </svg>
        Loading...
      {:else}
        Load Mock Save File
      {/if}
    </Button>
  </div>

  {#if players.length > 0}
    <div class="grid grid-cols-1 md:grid-cols-2 gap-6 text-left">
      {#each players as player (player.uid)}
        <div class="bg-light-navy border p-6 rounded-lg shadow-lg">
          <div class="flex justify-between items-start">
            <div>
              <h2 class="text-xl font-semibold text-green">
                {player.nickname}
              </h2>
              <p class="text-xs text-slate font-mono mb-4">UID: {player.uid}</p>
            </div>
            <span
              class="text-lightest-slate font-bold bg-lightest-navy px-3 py-1 rounded-full text-sm"
            >
              Lvl {player.level}
            </span>
          </div>

          <h3 class="font-bold text-light-slate mt-4 mb-2 pt-4">Pals:</h3>
          {#if player.pals.length > 0}
            <ul class="space-y-2">
              {#each player.pals as pal (pal.instance_id)}
                <li
                  class="flex items-center justify-between p-2 rounded-md hover:bg-lightest-navy/50 transition-colors"
                >
                  <span class="font-semibold text-light-slate">
                    {pal.nickname
                      ? `${pal.nickname} (${pal.character_id})`
                      : pal.character_id}
                  </span>
                  <span class="text-slate">Lvl {pal.level}</span>
                </li>
              {/each}
            </ul>
          {:else}
            <p class="text-slate italic">No pals found for this player.</p>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>
