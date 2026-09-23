<script lang="ts">
	let { closeModal }: { closeModal: (value?: unknown) => void } = $props();
</script>

<button type="button" data-testid="modal-body" onclick={() => closeModal('ok')}>panel</button>
