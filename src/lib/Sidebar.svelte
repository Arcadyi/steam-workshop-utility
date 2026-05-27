<script lang="ts">
    import {ArrowLeft, ArrowRight} from "$lib/icons";
    import GameCard from "$lib/GameCard.svelte";
    import { invoke } from "@tauri-apps/api/core";
    import { onMount } from "svelte";
    import type { GameEntry } from "$lib/types";

    let {
        onGameSelect,
    }: {
        onGameSelect?: (game: GameEntry) => void;
    } = $props();


    let loading = $state(true);
    let error = $state<string | null>(null);

    let expanded = $state(true);
    let games = $state<GameEntry[]>([]);
    let selectedId = $state<string | null>(null);
    onMount(async () => {
        try {
            games = await invoke("get_games");
        } catch (e) {
            console.error(e);
            error = e as string;
        } finally {
            loading = false;  // add this
        }
    });

    function selectGame(game: GameEntry) {
        selectedId = game.appid;
        console.log("selectGame called", game);
        onGameSelect?.(game);
    }
</script>

<aside class="sidebar" class:expanded>

    <!-- Toggle button -->
    <button class="toggle-btn glass-raised" onclick={() => expanded = !expanded}>
        {#if expanded}
            <ArrowLeft width={18} height={18} />
        {:else}
            <ArrowRight width={18} height={18} />
        {/if}
    </button>

    <!-- Game list -->
    <nav class="game-list glass-raised">
        {#if loading}
            <p class="status-msg">Loading games…</p>
        {:else if error}
            <p class="status-msg error">{error}</p>
        {:else if games.length === 0}
            <p class="status-msg">No games found</p>
        {:else}
            {#each games as game (game.appid)}
                <GameCard
                        {game}
                        {expanded}
                        active={selectedId === game.appid}
                        onclick={() => selectGame(game)}
                />
            {/each}
        {/if}
    </nav>

</aside>

<style>
    .sidebar {
        display: flex;
        flex-direction: column;
        width: 44px;
        align-items: center;
        height: 100%;
        transition: width var(--animation-slow);
        flex: 0 0 auto;
    }

    .sidebar.expanded {
        width: 256px;
    }

    .toggle-btn {
        height: 36px;
        width: 100%;
        border-radius: var(--radius-m) var(--radius-m) 0 0;
        border: 1px solid var(--glass-border);
        background: var(--glass-bg);
        display: grid;
        place-items: center;
        flex-shrink: 0;
        color: var(--text-muted);
        transition:
                background var(--animation-slow),
                border-radius var(--animation-slow),
                border var(--animation-slow);
        position:relative;
    }

    .toggle-btn :global(svg) {
        position: relative;
        z-index: 1;
    }

    .toggle-btn::before {
        content: '';
        position: absolute;
        inset: 0;
        background: var(--gradient-color);
        opacity: 0;
        transition: opacity var(--animation-fast);
        border-radius: inherit;
    }

    .toggle-btn:hover::before {
        opacity: 1;
    }

    .toggle-btn:hover {
        color: var(--text-primary);
    }

    .toggle-btn:active {
        background: var(--bg-card);
    }

    .game-list {
        width: 100%;
        display: flex;
        flex-direction: column;
        align-items: center;
        padding-top: var(--spacing-s);
        padding-left: var(--spacing-xxs);
        padding-right: var(--spacing-xxs);
        border: 1px solid var(--glass-border);
        border-radius: 0 0 var(--radius-s) var(--radius-s);
        overflow-y: auto;
        flex: 1;
        gap: var(--spacing-s);
    }
</style>