<script lang="ts">
	import { Spinner } from '$components';
	import Input from '$components/ui/input/Input.svelte';
	import { getAppState, getModalState } from '$states';

	const appState = getAppState();
	const modal = getModalState();

	let error = $state("");
	let isConnecting = $state(false);
	let hostname = $state('');
	let username = $state('');
	let password = $state('');

	async function handleConnect() {
		if (!hostname || !username || !password) return

		isConnecting = true;
		error = "An error occurred";

		console.log(hostname, username, password);
	}

	async function cancelConnection() {
		isConnecting = false;
		error = ""
	}
</script>

<div class="grid h-full grid-cols-[25%_1fr] gap-2 p-2">
	<div class="flex flex-col gap-4 p-4 border-gray-400 border-r-1">
		<div class="flex flex-col gap-2">
			<h1 class="text-2xl font-bold">SFTP Connection</h1>
			<p class="text-sm text-gray-500">Connect to your dedicated server via SFTP!</p>
		</div>
		<div class="flex flex-col gap-4">
			<div class="flex flex-col">
				<label for="hostname" class="text-sm font-medium">Hostname</label>
				<Input 
					type="text" 
					id="hostname"
					bind:value={hostname}
					inputClass="w-full"
					placeholder="e.g. sftp.example.com"
				/>
			</div>
			<div class="flex flex-col">
				<label for="username" class="text-sm font-medium">Username</label>
				<Input 
					type="text"
					id="username"
					bind:value={username}
					inputClass="w-full"
					placeholder="Enter username"
				/>
			</div>
			<div class="flex flex-col">
				<label for="password" class="text-sm font-medium">Password</label>
				<Input 
					type="password"
					id="password"
					bind:value={password}
					inputClass="w-full"
					placeholder="Enter password"
				/>
			</div>
			{#if error}
				<p class="text-sm text-red-500">{error}</p>
			{/if}
			<button 
				class="px-4 py-2 text-white bg-blue-500 rounded hover:bg-blue-600 disabled:bg-gray-400"
				disabled={isConnecting || (!hostname || !username || !password)}
				onclick={handleConnect}
			>
				{isConnecting ? 'Connecting...' : 'Connect'}
			</button>
		</div>
	</div>

	<div class="flex justify-center items-center h-full">
		{#if isConnecting}
			<div class="flex flex-col items-center gap-1">
				<Spinner size="size-32" />
				<p class="mt-4">Attempting to connect!</p>
				<button class="btn bg-secondary-600 hover:bg-secondary-800 p-2" onclick={cancelConnection}>
					Cancel
				</button>
			</div>
		{/if}
	</div>
</div>
