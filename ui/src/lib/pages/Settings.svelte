<script lang="ts">
	import Modal from '$components/modal/Modal.svelte';
	import { getModalState } from '$states';
	import TextInputModal from '../components/modals/text-input/TextInputModal.svelte';

	const modal = getModalState();

	async function showModal() {
		// @ts-ignore
		const result = await modal.showModal(TextInputModal, { title: 'Edit nickname' });
	}
</script>

<Modal>
	<h3 class="text-2xl">Settings</h3>
</Modal>

<button class="btn" on:click={showModal}>Show Modal</button>
