<script lang="ts">
	import { FolderArchive } from 'lucide-svelte';
	import { getAppState } from '$states';

	const appState = getAppState();

	let {
		files = $bindable<FileList>(),
		name,
		border = 'border-2 border-dashed',
		padding = 'p-4 py-8',
		rounded = 'rounded-container-token',
		regionInterface = '',
		regionInterfaceText = '',
		baseClass = '',
		inputClass = '',
		interfaceClass = '',
		lead,
		message,
		meta,
		accept = '.zip',
		onUpload,
		...restProps
	} = $props<{
		files?: FileList;
		name: string;
		border?: string;
		padding?: string;
		rounded?: string;
		regionInterface?: string;
		regionInterfaceText?: string;
		baseClass?: string;
		inputClass?: string;
		interfaceClass?: string;
		lead?: any;
		message?: any;
		meta?: any;
		accept?: string;
		onUpload?: (files: File) => void;
	}>();

	let fileInput: HTMLInputElement;

	const cBase = 'textarea relative flex justify-center items-center';
	const cInput =
		'w-full absolute top-0 left-0 right-0 bottom-0 z-[1] opacity-0 disabled:!opacity-0 cursor-pointer';
	const cInterface = 'flex justify-center items-center text-center';

	let classesBase = $derived(`${cBase} ${border} ${padding} ${rounded} ${baseClass}`);
	let classesInput = $derived(`${cInput} ${inputClass}`);
	let classesInterface = $derived(`${cInterface} ${interfaceClass}`);

	function prunedRestProps() {
		const { class: _, ...prunedProps } = restProps;
		return prunedProps;
	}
</script>

<div
	class="dropzone {classesBase}"
	class:opacity-50={restProps.disabled}
	data-testid="file-dropzone"
>
	<input
		bind:files
		bind:this={fileInput}
		type="file"
		{name}
		class="dropzone-input {classesInput}"
		{...prunedRestProps()}
		{accept}
		multiple={false}
	/>
	<div class="dropzone-interface {classesInterface} {regionInterface}">
		<div class="dropzone-interface-text {regionInterfaceText}">
			{#if lead}
				<div class="dropzone-lead mb-4">
					{@render lead()}
				</div>
			{/if}
			<div class="dropzone-message">
				{#if message}
					{@render message()}
				{:else}
					<strong>Upload a file</strong> or drag and drop
				{/if}
				<div class="mt-2 flex items-center justify-center">
					<FolderArchive class="h-24 w-24" />
				</div>
				{#if files}
					<div class="mt-4 flex flex-row items-center justify-center space-x-2">
						<strong>{files[0].name}</strong>
						<span>({(files[0].size / 1024 / 1024).toFixed(2)} MB)</span>
					</div>
				{/if}
			</div>
			{#if meta}
				<small class="dropzone-meta opacity-75">
					{@render meta()}
				</small>
			{/if}
		</div>
	</div>
</div>
