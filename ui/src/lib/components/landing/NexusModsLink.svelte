<script lang="ts">
	import nexusModsLogo from '$lib/assets/img/app/nexus_mods.webp';
	import { Link } from '.';

	const NEXUS = 'https://www.nexusmods.com/palworld/mods/1827';
</script>

<Link href={NEXUS}>
	<img src={nexusModsLogo} alt="Nexus Mods" class="h-8" />
</Link>
