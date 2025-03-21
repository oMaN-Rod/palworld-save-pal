<script lang="ts">
    import { Spinner } from '$components';
    import Input from '$components/ui/input/Input.svelte';
    import { getAppState, getModalState, getSocketState } from '$states';
    import { MessageType } from '$types';
	import type { SFTPConnectionRequest, SFTPConnectionResponse, SFTPFileItem } from '$types/sftp';
    import { sendAndWait } from '$utils/websocketUtils';

    const appState = getAppState();
    const modal = getModalState();
    const ws = getSocketState();

    let error = $state("");
    let isConnecting = $state(false);
    let hostname = $state('');
    let username = $state('');
    let password = $state('');
    
    // New state variables
    let currentPath = $state("");
    let files = $state<SFTPFileItem[]>([]);
    let isLoading = $state(false);

    async function handleConnect() {
        if (!hostname || !username || !password) return;
        isConnecting = true;
        error = "";
		
        const request: SFTPConnectionRequest = {
            hostname,
            username,
            password
        };

        try {
            const resp = await sendAndWait<SFTPConnectionResponse>(
                MessageType.SETUP_SFTP_CONNECTION, 
                request
            );

            if (resp.success) {
                files = resp.files;
                currentPath = resp.path;
                isConnecting = false;
            } else {
                error = resp.message || "Failed to connect";
                isConnecting = false;
            }
        } catch (e) {
            console.error(e);
            isConnecting = false;
            error = "An error occurred";
        }
    }

    async function navigateToDirectory(dirName: string) {
        // isLoading = true;
        // try {
        //     const newPath = currentPath ? `${currentPath}/${dirName}` : dirName;
        //     const resp = await sendAndWait(MessageType.NAVIGATE_DIRECTORY, {
        //         path: newPath
        //     });

        //     if (resp.success) {
        //         files = resp.files;
        //         currentPath = resp.path;
        //     } else {
        //         error = resp.message || "Failed to navigate";
        //     }
        // } catch (e) {
        //     console.error(e);
        //     error = "Failed to navigate directory";
        // } finally {
        //     isLoading = false;
        // }
    }

    async function navigateUp() {
        // if (!currentPath) return;
        
        // const parentPath = currentPath.split('/').slice(0, -1).join('/');
        // isLoading = true;
        
        // try {
        //     const resp = await sendAndWait(MessageType.NAVIGATE_DIRECTORY, {
        //         path: parentPath
        //     });

        //     if (resp.success) {
        //         files = resp.files;
        //         currentPath = resp.path;
        //     } else {
        //         error = resp.message || "Failed to navigate";
        //     }
        // } catch (e) {
        //     console.error(e);
        //     error = "Failed to navigate directory";
        // } finally {
        //     isLoading = false;
        // }
    }

    async function cancelConnection() {
        isConnecting = false;
        error = "";
        files = [];
        currentPath = "";
    }
</script>

<div class="grid h-full grid-cols-[25%_1fr] gap-2 p-2">
    <div class="flex flex-col gap-4 p-4 border-gray-400 border-r-1">
        <!-- Existing connection form -->
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

    <div class="flex flex-col h-full p-4">
        {#if isConnecting}
            <div class="flex flex-col items-center gap-1">
                <Spinner size="size-32" />
                <p class="mt-4">Attempting to connect!</p>
                <button class="btn bg-secondary-600 hover:bg-secondary-800 p-2" onclick={cancelConnection}>
                    Cancel
                </button>
            </div>
        {:else if files.length > 0}
            <div class="flex flex-col gap-4">
                <div class="flex items-center gap-2">
                    <h2 class="text-xl">Current Path: {currentPath || '/'}</h2>
                    {#if currentPath}
                        <button 
                            class="px-2 py-1 text-sm bg-gray-200 rounded hover:bg-gray-300"
                            onclick={navigateUp}
                        >
                            Go Up
                        </button>
                    {/if}
                </div>
                
                {#if isLoading}
                    <Spinner size="size-8" />
                {:else}
                    <div class="grid grid-cols-4 gap-4">
                        {#each files as file}
                            <button 
                                class="p-2 border rounded cursor-pointer hover:bg-gray-100"
                                onclick={() => file.is_dir && navigateToDirectory(file.name)}
                            >
                                <div class="flex items-center gap-2">
                                    {#if file.is_dir}
                                        📁
                                    {:else}
                                        📄
                                    {/if}
                                    {file.name}
                                </div>
                            </button>
                        {/each}
                    </div>
                {/if}
            </div>
        {/if}
    </div>
</div>
