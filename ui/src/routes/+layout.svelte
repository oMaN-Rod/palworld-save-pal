<script lang="ts">
	import '../app.css';
	import { NavBar, Toast, Modal } from '$components';
	import {
		activeSkillsData,
		elementsData,
		itemsData,
		palsData,
		passiveSkillsData,
		presetsData
	} from '$lib/data';

	const { children } = $props();

	$effect(() => {
		const loadData = async () => {
			await activeSkillsData.getActiveSkills();
			await passiveSkillsData.getPassiveSkills();
			await elementsData.getAllElements();
			await itemsData.getAllItems();
			await palsData.getAllPals();
			await presetsData.getAllPresets();
		};
		loadData();
	});
</script>

<Toast position="bottom-center" />
<Modal>
	<div class="flex h-screen w-full overflow-hidden">
		<NavBar />
		<main class="flex-1 overflow-hidden">
			{@render children()}
		</main>
	</div>
</Modal>
