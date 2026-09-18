<script lang="ts">
	import { Seo } from '$lib/components/seo';

	const UPDATED = '18 September 2026';
</script>

<Seo
	pathname="/privacy"
	title="Privacy — PalStudio"
	description="What PalStudio does and does not collect: save files stay on your device, analytics are anonymous, and there are no accounts."
/>

<div class="mx-auto max-w-3xl px-4 py-10 text-surface-200">
	<h1 class="text-3xl font-bold text-white">Privacy</h1>
	<p class="mt-2 text-sm text-surface-400">Last updated {UPDATED}</p>

	<section class="mt-8 space-y-3">
		<p>
			PalStudio has no accounts and no user database. We do not ask for your name, your email
			address, or your Steam or Xbox identity, and there is nothing to log in to.
		</p>
		<p>
			This page describes the web app at palstudio.app and the PalStudio desktop app. Both are
			open source, so every claim here can be checked against the code.
		</p>
	</section>

	<section class="mt-8 space-y-3">
		<h2 class="text-xl font-semibold text-white">Your save files</h2>
		<p>
			Save files are read and written on your own device. In the browser they are parsed by
			WebAssembly running in the page; on the desktop they are parsed by the app itself. Editing a
			save never uploads it to us, and we never receive a copy.
		</p>
		<p>
			Before writing to a save, PalStudio copies the original files first — to
			<code class="rounded bg-surface-800 px-1 py-0.5 text-sm">.ps-backup/</code> in the browser, or
			a timestamped folder under
			<code class="rounded bg-surface-800 px-1 py-0.5 text-sm">backups/</code> on the desktop. Those
			backups stay on your machine, and they are never pruned automatically, so you may want to
			clear out old ones yourself.
		</p>
	</section>

	<section class="mt-8 space-y-3">
		<h2 class="text-xl font-semibold text-white">Remote sessions</h2>
		<p>
			If you pair a phone or another computer with your PC, that is the one feature where save data
			does travel over the network. The two devices connect directly to each other over WebRTC, and
			the connection is encrypted end to end.
		</p>
		<p>
			When a direct connection is impossible — commonly on mobile hotspots or carrier-grade NAT —
			the encrypted traffic is relayed through a TURN server we operate at turn.palstudio.app. The
			relay forwards encrypted packets; it cannot read what passes through it, and nothing is
			written to disk there.
		</p>
		<p>
			Pairing itself goes through a broker that passes sealed messages between the two devices. A
			pairing room holds only the codes needed to introduce them, and it is deleted after five
			minutes.
		</p>
	</section>

	<section class="mt-8 space-y-3">
		<h2 class="text-xl font-semibold text-white">Analytics</h2>
		<p>
			The web app loads Cloudflare Web Analytics. It is cookieless and records page views without
			building a profile of you: no cookie is set for it, no identifier follows you between sites,
			and the data is not sold or shared. We use it only to see which pages get used.
		</p>
		<p>The desktop app loads no analytics at all.</p>
	</section>

	<section class="mt-8 space-y-3">
		<h2 class="text-xl font-semibold text-white">What is stored on your device</h2>
		<p>
			Your settings, presets and recent selections are kept in your browser's local storage, or in a
			local database in the desktop app. They stay on your device and are not synced anywhere.
		</p>
		<p>
			One cookie is set: it remembers the language you picked, so the site opens in that language
			next time. It contains nothing else.
		</p>
	</section>

	<section class="mt-8 space-y-3">
		<h2 class="text-xl font-semibold text-white">Connections the app makes</h2>
		<p>Besides loading the site itself, PalStudio may contact:</p>
		<ul class="list-disc space-y-1 pl-6">
			<li>GitHub, to check whether a newer release exists and to download mod frameworks.</li>
			<li>Nexus Mods, if you use the built-in mod browser, using the API key you supply.</li>
			<li>
				Our pairing broker and TURN relay, only while a remote session is being set up or running.
			</li>
		</ul>
		<p>
			These services receive your IP address because that is how the internet works, and each has
			its own privacy policy.
		</p>
	</section>

	<section class="mt-8 space-y-3">
		<h2 class="text-xl font-semibold text-white">Children</h2>
		<p>
			PalStudio is a tool for a video game and is not directed at children under 13. We do not
			knowingly collect personal information from anyone.
		</p>
	</section>

	<section class="mt-8 space-y-3">
		<h2 class="text-xl font-semibold text-white">Changes and contact</h2>
		<p>
			If this policy changes we will update the date above. Questions are welcome on our Discord or
			as an issue on GitHub.
		</p>
	</section>

	<section class="mt-8 space-y-3 border-t border-surface-700 pt-6">
		<h2 class="text-xl font-semibold text-white">Not affiliated with Pocketpair</h2>
		<p class="text-sm text-surface-400">
			PalStudio is a fan-made tool. It is not affiliated with, endorsed by, or sponsored by
			Pocketpair, Inc. Palworld and all related names, artwork and game data are the property of
			their respective owners, and are used here only so the tool can read and display your save.
		</p>
	</section>
</div>
