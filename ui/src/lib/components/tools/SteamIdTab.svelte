<script lang="ts">
	import { getToastState } from '$states';
	import { Button, Card, Spinner } from '$components/ui';
	import { sendAndWait } from '$lib/utils/websocketUtils';
	import { MessageType } from '$types';
	import * as m from '$i18n/messages';
	import { Hash, Copy, Check } from 'lucide-svelte';

	const toast = getToastState();

	let steamInput = $state('');
	let steamConverting = $state(false);
	let steamResult: {
		palworld_uid?: string;
		nosteam_uid?: string;
		error?: string;
		from_uid?: boolean;
	} | null = $state(null);
	let copiedField: string | null = $state(null);

	async function handleConvertSteamId() {
		if (!steamInput.trim()) return;
		steamConverting = true;
		steamResult = null;
		try {
			const result = await sendAndWait<{
				palworld_uid?: string;
				nosteam_uid?: string;
				error?: string;
			}>(MessageType.CONVERT_STEAM_ID, { steam_input: steamInput.trim() });
			steamResult = result;
			if (result.error) {
				toast.add(result.error, m.error(), 'error');
			}
		} catch (err: any) {
			steamResult = { error: err.message };
			toast.add(m.tools_conversion_failed({ error: err.message }), m.error(), 'error');
		} finally {
			steamConverting = false;
		}
	}

	async function copyToClipboard(text: string, field: string) {
		await navigator.clipboard.writeText(text);
		copiedField = field;
		setTimeout(() => (copiedField = null), 2000);
	}
</script>

<div class="flex flex-col gap-8">
	<section class="w-full">
		<p class="text-surface-400 mb-6 text-center text-sm">{m.tools_steam_id_description()}</p>

		<Card class="mx-auto max-w-lg">
			<div class="flex flex-col gap-4 p-4">
				<div class="flex flex-col gap-2">
					<label for="steam-input" class="text-surface-300 text-sm font-medium">
						{m.tools_steam_id_input_label()}
					</label>
					<div class="flex gap-2">
						<input
							id="steam-input"
							type="text"
							bind:value={steamInput}
							placeholder={m.tools_steam_id_placeholder()}
							class="bg-surface-800 border-surface-600 text-surface-100 placeholder:text-surface-500 focus:border-primary-500 flex-1 rounded-lg border px-3 py-2 text-sm focus:outline-none"
							onkeydown={(e) => e.key === 'Enter' && handleConvertSteamId()}
						/>
						<Button
							variant="primary"
							onclick={handleConvertSteamId}
							disabled={steamConverting || !steamInput.trim()}
						>
							{#if steamConverting}
								<Spinner />
							{:else}
								<Hash size={16} />
								<span>{m.tools_convert_action()}</span>
							{/if}
						</Button>
					</div>
					<span class="text-surface-500 text-xs">{m.tools_steam_id_accepts()}</span>
				</div>

				{#if steamResult && !steamResult.error}
					<div class="border-surface-700 flex flex-col gap-3 border-t pt-4">
						{#if steamResult.from_uid}
							<p class="text-surface-400 text-xs italic">{m.tools_steam_id_detected_uid()}</p>
						{/if}
						<div class="flex flex-col gap-1">
							<span class="text-surface-400 text-xs font-medium tracking-wider uppercase">
								{m.tools_palworld_uid()}
							</span>
							<div class="flex items-center gap-2">
								<code
									class="bg-surface-900 text-primary-400 flex-1 rounded px-3 py-1.5 font-mono text-sm"
								>
									{steamResult.palworld_uid}
								</code>
								<button
									class="text-surface-400 hover:text-surface-200 p-1"
									onclick={() => copyToClipboard(steamResult!.palworld_uid!, 'palworld')}
								>
									{#if copiedField === 'palworld'}
										<Check size={16} class="text-green-400" />
									{:else}
										<Copy size={16} />
									{/if}
								</button>
							</div>
						</div>
						<div class="flex flex-col gap-1">
							<span class="text-surface-400 text-xs font-medium tracking-wider uppercase">
								{m.tools_nosteam_uid()}
							</span>
							<div class="flex items-center gap-2">
								<code
									class="bg-surface-900 text-primary-400 flex-1 rounded px-3 py-1.5 font-mono text-sm"
								>
									{steamResult.nosteam_uid}
								</code>
								<button
									class="text-surface-400 hover:text-surface-200 p-1"
									onclick={() => copyToClipboard(steamResult!.nosteam_uid!, 'nosteam')}
								>
									{#if copiedField === 'nosteam'}
										<Check size={16} class="text-green-400" />
									{:else}
										<Copy size={16} />
									{/if}
								</button>
							</div>
						</div>
					</div>
				{/if}

				{#if steamResult?.error}
					<div class="border-surface-700 border-t pt-4">
						<p class="text-sm text-red-400">{steamResult.error}</p>
					</div>
				{/if}
			</div>
		</Card>
	</section>
</div>
