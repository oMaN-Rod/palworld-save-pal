<script lang="ts">
  import "../app.css";

  let { children } = $props();
</script>

<div class="flex h-screen bg-dark-navy text-slate">
  <!-- Sidebar -->
  <aside
    class="w-64 flex-shrink-0 bg-light-navy border-r border-lightest-navy p-6"
  >
    <div class="text-green font-mono font-bold text-xl mb-8">PSP Next Gen</div>
    <nav class="space-y-2">
      <!-- Navigation links will go here -->
      <a
        href="#"
        class="flex items-center gap-3 rounded-lg bg-lightest-navy px-3 py-2 text-lightest-slate transition-all hover:text-lightest-slate"
      >
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="24"
          height="24"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
          class="h-4 w-4"
          ><path
            d="M4 20h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2H4a2 2 0 0 0-2 2v10a2 2 0 0 0 2 2Z"
          /><path d="m20 12-8.5-4.25a2 2 0 0 0-2 0L4 12" /><path
            d="M4 12v4"
          /><path d="M20 12v4" /><path d="M12 12v8" /></svg
        >
        Save File
      </a>
      <a
        href="#"
        class="flex items-center gap-3 rounded-lg px-3 py-2 text-slate transition-all hover:text-lightest-slate"
      >
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="24"
          height="24"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
          class="h-4 w-4"
          ><path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2" /><circle
            cx="9"
            cy="7"
            r="4"
          /><path d="M22 21v-2a4 4 0 0 0-3-3.87" /><path
            d="M16 3.13a4 4 0 0 1 0 7.75"
          /></svg
        >
        Players
      </a>
      <a
        href="#"
        class="flex items-center gap-3 rounded-lg px-3 py-2 text-slate transition-all hover:text-lightest-slate"
      >
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="24"
          height="24"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
          class="h-4 w-4"
          ><path d="m12 14 4 6" /><path d="M12 14l-4 6" /><path
            d="M12 6V2"
          /><path d="M12 6l4 3.5" /><path d="M12 6 8 9.5" /><path
            d="m20.5 10-4.04 3.5"
          /><path d="m3.5 10 4.04 3.5" /><circle cx="12" cy="12" r="10" /></svg
        >
        Pals
      </a>
      <a
        href="#"
        class="flex items-center gap-3 rounded-lg px-3 py-2 text-slate transition-all hover:text-lightest-slate"
      >
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="24"
          height="24"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
          class="h-4 w-4"
          ><rect width="8" height="18" x="3" y="3" rx="1" /><rect
            width="8"
            height="18"
            x="13"
            y="3"
            rx="1"
          /></svg
        >
        Guilds
      </a>
    </nav>
  </aside>

  <!-- Main Content -->
  <main
    class="flex-1 overflow-y-auto p-8 bg-dark-navy"
    style="background-size: 20px 20px;"
  >
    {@render children()}
  </main>
</div>

<style>
  :root {
    --dark-navy: #0a192f;
    --light-navy: #112240;
    --lightest-navy: #233554;
    --slate: #8892b0;
    --light-slate: #a8b2d1;
    --lightest-slate: #ccd6f6;
    --green: #64ffda;
    --font-mono: "Fira Code", monospace;
  }
</style>
