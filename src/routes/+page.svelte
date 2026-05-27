<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import "../style.css";
  import Titlebar from "$lib/Titlebar.svelte";
  import Sidebar from "$lib/Sidebar.svelte";
  import Topbar from "$lib/Topbar.svelte";
  import ModList from "$lib/ModList.svelte";

  import type { GameEntry } from "$lib/types";

  let selectedGame = $state<GameEntry | null>(null);
  let searchQuery = $state("");
  let sortBy = $state("name");
  let compact = $state(false);
  let allSelected = $state(false);
  let someSelected = $state(false);
  let modListRef = $state<ModList | null>(null);

  function handleSelectionChange(all: boolean, some: boolean) {
    allSelected = all;
    someSelected = some;
  }

  function handleContextMenu(e: MouseEvent) {
    e.preventDefault();
  }
</script>

<main oncontextmenu={handleContextMenu}>
  <svg width="0" height="0" style="position: absolute;" aria-hidden="true">
    <defs>
      <linearGradient id="app-gradient" x1="0%" y1="0%" x2="100%" y2="0%">
        <stop offset="0%" stop-color="rgba(182, 59, 28, 1)" />
        <stop offset="100%" stop-color="rgba(235, 175, 23, 1)" />
      </linearGradient>
    </defs>
  </svg>
  <Titlebar />
  <div class="app-body">
    <Sidebar onGameSelect={(game) => selectedGame = game} />
    <div class="main-content">
      <Topbar
              {allSelected}
              {someSelected}
              bind:searchQuery
              bind:sortBy
              bind:compact
              onSelectAll={(v) => modListRef?.selectAll(v)}
              onExport={() => console.log('export')}
              onImport={() => console.log('import')}
      />
      <ModList
              bind:this={modListRef}
              game={selectedGame}
              {searchQuery}
              {sortBy}
              {compact}
              onSelectionChange={handleSelectionChange}
      />
    </div>
  </div>
</main>

<style>
  main {
    display: flex;
    flex-direction: column;
    height: 100vh;
    overflow: hidden;
    position: relative;
  }

  .app-body {
    display: flex;
    flex: 1;
    height: 100%;
    overflow: hidden;
    position: relative;
    z-index: 1;
    padding: var(--spacing-m);
  }

  .main-content {
    display: flex;
    padding-left: var(--spacing-l);
    flex-direction: column;
    flex: 1;
    height: 100%;
    overflow: hidden;
    min-width: 0;
  }
</style>