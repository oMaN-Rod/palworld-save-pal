<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { Button, Card, Input, Spinner } from '$components/ui';
	import { onMount, untrack } from 'svelte';
	import { focusModal } from '$utils/modalUtils';
	import { PUBLIC_DESKTOP_MODE } from '$env/static/public';
	import { getModsState, getSocketState } from '$states';
	import { getRemoteMode } from '$lib/signal/remoteMode.svelte';
	import { MessageType } from '$types';
	import type { Decision, ModError, PendingDecisions, RecordedModError } from '$types';
	import * as m from '$i18n/messages';
	import ManifestReview from './ManifestReview.svelte';
	import UploadPicker from './UploadPicker.svelte';
	import { modTypeLabel } from './modLabels';

	let { targetId, closeModal }: { targetId: string; closeModal: (installed: boolean) => void } =
		$props();

	const SELECT_FILE = '__select__';
	const ARCHIVE_ACCEPT = '.zip,.7z,.rar,.tar,.gz,.tgz,.pak,.lua,.dll';

	const modsState = getModsState();
	const socket = getSocketState();
	const remoteMode = getRemoteMode();
	const isDesktopMode = PUBLIC_DESKTOP_MODE === 'true';

	let modalContainer: HTMLDivElement;
	let path = $state('');
	let pathEntry = $state(!isDesktopMode);
	let analyzeSent = $state(false);
	let sentAnalyzePath = $state<string | null>(null);
	let awaitingAnalysis = $state(false);
	let analysisInterrupted = $state(false);
	let installSent = $state(false);
	let awaitingInstall = $state(false);
	let sentInstall: { seq: number; path: string; enable: boolean } | null = null;
	let interrupted = $state(false);
	let offered = $state<PendingDecisions | null>(null);
	let outcome = $state<{ name: string; enabled: boolean; enableError?: ModError } | null>(null);
	let customName = $state('');
	let enable = $state(true);

	const target = $derived(modsState.targets.find((entry) => entry.id === targetId));
	const analysis = $derived(analyzeSent ? modsState.analysis[targetId] : undefined);
	const decisions = $derived<Decision[]>(offered?.decisions ?? analysis?.manifest.decisions ?? []);
	const installing = $derived(modsState.installing[targetId] ?? false);
	const analyzing = $derived(modsState.analyzing[targetId] ?? false);
	const reading = $derived(analyzing && sentAnalyzePath !== SELECT_FILE);
	const analyzeError = $derived(analyzeSent ? refusalFor(MessageType.MOD_ANALYZE) : undefined);
	const installError = $derived(installSent ? refusalFor(MessageType.MOD_INSTALL) : undefined);
	const serverIncapable = $derived(
		target?.kind === 'server' && analysis?.manifest.source.server_capable === false
	);
	const step = $derived(outcome ? 'outcome' : analysis ? 'review' : 'choose');

	/** A refusal that could not name its target may still answer this modal's request. */
	function refusalFor(type: MessageType): RecordedModError | undefined {
		const own = modsState.lastErrorFor(type, targetId);
		if (own) return own;
		const untargeted = modsState.lastErrorFor(type);
		return untargeted && (untargeted.target_id ?? targetId) === targetId ? untargeted : undefined;
	}

	function analyze(requested: string) {
		if (!requested || analyzing) return;
		modsState.clearLastError(MessageType.MOD_ANALYZE, targetId);
		modsState.clearLastError(MessageType.MOD_INSTALL, targetId);
		installSent = false;
		interrupted = false;
		analysisInterrupted = false;
		offered = null;
		analyzeSent = true;
		sentAnalyzePath = requested;
		awaitingAnalysis = true;
		modsState.analyze(targetId, requested);
	}

	function chooseAnother() {
		analyzeSent = false;
		installSent = false;
		interrupted = false;
		analysisInterrupted = false;
		offered = null;
	}

	function install() {
		if (!analysis || installing) return;
		modsState.clearLastError(MessageType.MOD_INSTALL, targetId);
		interrupted = false;
		installSent = true;
		awaitingInstall = true;
		const installPath = offered?.path ?? analysis.path;
		const name = customName.trim();
		modsState.install(targetId, installPath, {
			acceptDefaults: decisions.length > 0,
			enable,
			customName: name === analysis.manifest.display_name ? undefined : name
		});
		sentInstall = { seq: modsState.installSeq[targetId] ?? 0, path: installPath, enable };
	}

	$effect(() => {
		const manifest = analysis?.manifest;
		if (!manifest) return;
		untrack(() => {
			customName = manifest.display_name;
			enable = true;
		});
	});

	$effect(() => {
		if (analyzeError?.code === 'desktop_only') untrack(() => (pathEntry = true));
	});

	$effect(() => {
		if (step) untrack(() => focusModal(modalContainer));
	});

	/** A typed path is never cancelled, so an empty ending means the request was lost; a pick is lost only on a drop. */
	$effect(() => {
		const busy = analyzing;
		const answered = analysis !== undefined || analyzeError !== undefined;
		if (!awaitingAnalysis || busy) return;
		untrack(() => {
			awaitingAnalysis = false;
			if (answered) return;
			analysisInterrupted = sentAnalyzePath !== SELECT_FILE || !socket.connected;
		});
	});

	$effect(() => {
		const busy = installing;
		const needs = modsState.needsDecisions[targetId];
		const refused = installError;
		const record = modsState.lastInstall[targetId];
		if (!awaitingInstall || busy) return;
		untrack(() => {
			awaitingInstall = false;
			if (needs) {
				offered = needs;
				return;
			}
			if (refused) return;
			if (!record || record.seq !== sentInstall?.seq || record.path !== sentInstall.path) {
				interrupted = true;
				return;
			}
			outcome = {
				name: customName.trim() || analysis?.manifest.display_name || '',
				enabled: sentInstall.enable,
				enableError: record.enable_error
			};
		});
	});

	function refusalText(error: RecordedModError): string {
		switch (error.code) {
			case 'nothing_routed':
				return m.mods_review_nothing_routed();
			case 'already_installed':
				return m.mods_install_already_installed();
			case 'already_managed':
				return m.mods_install_already_managed();
			case 'extract_failed':
				return m.mods_install_extract_failed({ message: error.message });
			case 'invalid_path':
				return m.mods_install_invalid_path({ message: error.message });
			case 'desktop_only':
				return m.mods_install_desktop_only();
			case 'remote_denied':
				return m.mods_install_remote_denied();
			default:
				return error.message;
		}
	}

	function enableErrorText(error: ModError): string {
		if (error.code === 'not_subscribed_on_target') return m.mods_panel_error_not_subscribed();
		if (error.code !== 'not_supported_on_target') return error.message;
		const kind = String(error.kind ?? '');
		if (kind === 'workshop' && target?.kind === 'server') return m.mods_list_docker_workshop();
		if (kind === 'nativedll' && target?.kind === 'client') return m.mods_list_native_client();
		return m.mods_list_not_supported({ kind: modTypeLabel(kind) });
	}

	/** The shared modal turns any Enter into a click on the primary button, which here installs. */
	function keepEnterOnControls(node: HTMLElement) {
		const onKeydown = (event: KeyboardEvent) => {
			if (event.key !== 'Enter' || !(event.target instanceof Element)) return;
			const control = event.target.closest(
				'button, a[href], [role="button"], input[type="checkbox"]'
			);
			if (control && !control.hasAttribute('data-modal-primary')) event.stopPropagation();
		};
		node.addEventListener('keydown', onKeydown);
		return () => node.removeEventListener('keydown', onKeydown);
	}

	function isOutsideContent(eventTarget: EventTarget | null): boolean {
		const dialog = modalContainer.closest('[role="dialog"]');
		return (
			eventTarget instanceof Node &&
			dialog !== null &&
			dialog.contains(eventTarget) &&
			!modalContainer.contains(eventTarget)
		);
	}

	/** Closing before the install replies would leave its outcome with nowhere to show. */
	function blockDismissal(event: Event) {
		if (!awaitingInstall) return;
		const dismissing =
			event instanceof KeyboardEvent ? event.key === 'Escape' : isOutsideContent(event.target);
		if (!dismissing) return;
		event.preventDefault();
		event.stopImmediatePropagation();
	}

	onMount(() => {
		modsState.clearLastError(MessageType.MOD_ANALYZE, targetId);
		modsState.clearLastError(MessageType.MOD_INSTALL, targetId);
		window.addEventListener('keydown', blockDismissal, true);
		window.addEventListener('click', blockDismissal, true);
		return () => {
			window.removeEventListener('keydown', blockDismissal, true);
			window.removeEventListener('click', blockDismissal, true);
		};
	});
</script>

<div bind:this={modalContainer} {@attach keepEnterOnControls}>
	<Card class="flex max-h-[85vh] w-[640px] max-w-full flex-col gap-4 overflow-y-auto">
		<h3 class="h3">{m.mods_install_title()}</h3>

		{#if outcome}
			{#if outcome.enableError}
				<div class="border-warning-500/40 bg-warning-500/10 rounded-sm border p-3 text-sm">
					<p class="text-warning-400 flex items-center gap-2 font-medium">
						<Icon icon="tabler:alert-triangle" size={14} class="shrink-0" />
						{m.mods_install_not_enabled({ name: outcome.name })}
					</p>
					<p class="text-surface-300 mt-1">{enableErrorText(outcome.enableError)}</p>
				</div>
			{:else}
				<div
					role="status"
					class="border-success-500/40 bg-success-500/10 rounded-sm border p-3 text-sm"
				>
					<p class="text-success-400 flex items-center gap-2 font-medium">
						<Icon icon="tabler:check" size={14} class="shrink-0" />
						{m.mods_panel_installed({ name: outcome.name })}
					</p>
					{#if outcome.enabled}
						<p class="text-surface-300 mt-1">{m.mods_install_enabled_hint()}</p>
					{/if}
				</div>
			{/if}
			<div class="flex justify-end">
				<Button variant="ghost" onclick={() => closeModal(true)} data-modal-primary>
					{m.mods_close()}
				</Button>
			</div>
		{:else if analysis}
			<ManifestReview manifest={analysis.manifest} {decisions} />

			{#if serverIncapable}
				<p class="text-warning-400 flex items-start gap-2 text-sm">
					<Icon icon="tabler:alert-triangle" size={14} class="mt-0.5 shrink-0" />
					{m.mods_install_server_incapable()}
				</p>
			{/if}

			{#if decisions.length > 0}
				<p class="text-warning-400 text-sm" role="status">{m.mods_install_needs_decisions()}</p>
			{/if}

			<Input label={m.mods_install_name_label()} bind:value={customName} disabled={installing} />
			<label
				class="flex w-fit items-center gap-2 text-sm {installing
					? 'cursor-not-allowed opacity-50'
					: 'cursor-pointer'}"
			>
				<input
					type="checkbox"
					class="accent-primary-500 size-4"
					bind:checked={enable}
					disabled={installing}
				/>
				{m.mods_install_enable()}
			</label>

			{#if interrupted}
				<p class="text-error-400 text-sm" role="alert">{m.mods_install_interrupted()}</p>
			{:else if installError}
				<p class="text-error-400 text-sm" role="alert">{refusalText(installError)}</p>
			{/if}

			{#if installing && !awaitingInstall}
				<p class="text-surface-400 text-xs">{m.mods_install_busy()}</p>
			{/if}

			<div class="flex flex-wrap items-center justify-between gap-2">
				<Button variant="ghost" disabled={installing} onclick={chooseAnother}>
					{m.mods_install_choose_another()}
				</Button>
				<div class="flex items-center gap-2">
					<Button variant="ghost" disabled={awaitingInstall} onclick={() => closeModal(false)}>
						{m.mods_panel_cancel()}
					</Button>
					<Button
						variant="primary"
						disabled={installing}
						loading={installing && awaitingInstall}
						onclick={install}
						data-modal-primary
					>
						{decisions.length > 0 ? m.mods_install_with_defaults() : m.mods_install_submit()}
					</Button>
				</div>
			</div>
		{:else}
			{#if remoteMode.active}
				<UploadPicker
					{targetId}
					purpose="install"
					accept={ARCHIVE_ACCEPT}
					label={m.mods_upload_choose_label()}
					onUploaded={analyze}
				/>
			{:else if !pathEntry}
				<Button
					variant="neutral"
					class="flex items-center gap-2 self-start"
					disabled={analyzing}
					onclick={() => analyze(SELECT_FILE)}
					data-modal-primary
				>
					<Icon icon="tabler:folder-open" size={14} />
					{m.mods_install_choose()}
				</Button>
			{:else}
				<div class="flex flex-col gap-1">
					<div class="flex items-end gap-2">
						<div class="grow">
							<Input
								label={m.mods_install_path_label()}
								bind:value={path}
								placeholder={m.mods_panel_install_path_placeholder()}
								disabled={analyzing}
							/>
						</div>
						<Button
							variant="secondary"
							class="mb-2"
							disabled={analyzing || path.trim().length === 0}
							loading={reading}
							onclick={() => analyze(path.trim())}
							data-modal-primary
						>
							{m.mods_install_analyze()}
						</Button>
					</div>
					<p class="text-surface-500 text-xs">{m.mods_panel_install_path_hint()}</p>
				</div>
			{/if}

			<p class="text-surface-400 text-xs">{m.mods_install_supported()}</p>

			{#if reading}
				<p class="text-surface-400 flex items-center gap-2 text-sm">
					<Spinner size="size-4" />
					<span>{m.mods_install_analyzing()}</span>
				</p>
			{/if}

			{#if analysisInterrupted}
				<p class="text-error-400 text-sm" role="alert">{m.mods_install_analyze_interrupted()}</p>
			{:else if analyzeError}
				<p class="text-error-400 text-sm" role="alert">{refusalText(analyzeError)}</p>
			{/if}

			<div class="flex justify-end">
				<Button variant="ghost" onclick={() => closeModal(false)}>{m.mods_close()}</Button>
			</div>
		{/if}
	</Card>
</div>
