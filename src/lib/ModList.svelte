<script lang="ts">
    import { invoke } from "@tauri-apps/api/core";
    import type { GameEntry, WorkshopItem } from "$lib/types";
    import ModCard from "$lib/ModCard.svelte";

    let {
        game,
        searchQuery = "",
        sortBy = "name",
        compact = false,
        onSelectionChange,
    }: {
        game: GameEntry | null;
        searchQuery?: string;
        sortBy?: string;
        compact?: boolean;
        onSelectionChange?: (allSelected: boolean, someSelected: boolean) => void;
    } = $props();

    let items = $state<WorkshopItem[]>([]);
    let loading = $state(false);
    let error = $state<string | null>(null);
    let isScrolling = $state(false);
    let scrollTimeout = $state<number | undefined>(undefined);

    $effect(() => {
        const g = game;
        if (g) {
            loadItems(g);
        } else {
            items = [];
        }
    });

    async function loadItems(g: GameEntry) {
        loading = true;
        error = null;
        try {
            const result = await invoke("get_workshop_items", { game: g });
            items = result as WorkshopItem[];
        } catch (e) {
            error = e as string;
        } finally {
            loading = false;
        }
    }

    const filteredItems = $derived.by(() => {
        let result = [...items];

        if (searchQuery.trim()) {
            const q = searchQuery.toLowerCase();
            result = result.filter(item =>
                item.name?.toLowerCase().includes(q) ||
                item.item_id.includes(q)
            );
        }

        result.sort((a, b) => {
            switch (sortBy) {
                case "name":
                    return (a.name ?? a.item_id).localeCompare(b.name ?? b.item_id);
                case "last_updated":
                    return (b.remote_timestamp ?? 0) - (a.remote_timestamp ?? 0);
                case "size":
                    return b.disk_size - a.disk_size;
                case "status":
                    return a.status.localeCompare(b.status);
                default:
                    return 0;
            }
        });

        return result;
    });

    function toggleItem(item_id: string, selected: boolean) {
        items = items.map(i =>
            i.item_id === item_id ? { ...i, selected } : i
        );
        const selectedCount = items.filter(i => i.selected).length;
        onSelectionChange?.(
            selectedCount === items.length,
            selectedCount > 0
        );
    }

    export function selectAll(val: boolean) {
        items = items.map(i => ({ ...i, selected: val }));
        onSelectionChange?.(val, val);
    }

    function handleScroll() {
        isScrolling = true;
        clearTimeout(scrollTimeout);
        scrollTimeout = window.setTimeout(() => {
            isScrolling = false;
        }, 150);
    }
</script>

<div
    class="mod-list"
    class:is-scrolling={isScrolling}
    onscroll={handleScroll}
>
    {#if !game}
        <div class="empty-state">
            <p>Select a game from the sidebar to view workshop items</p>
        </div>
    {:else if loading}
        <div class="empty-state">
            <p>Loading workshop items…</p>
        </div>
    {:else if error}
        <div class="empty-state error">
            <p>{error}</p>
        </div>
    {:else if filteredItems.length === 0}
        <div class="empty-state">
            <p>{items.length === 0 ? "No workshop items found" : "No items match your search"}</p>
        </div>
    {:else}
        {#each filteredItems as item (item.item_id)}
            <ModCard
                    {item}
                    {compact}
                    onToggleSelect={(selected) => toggleItem(item.item_id, selected)}
            />
        {/each}
    {/if}
</div>

<style>
    .mod-list {
        content-visibility: auto;
        contain-intrinsic-size: 118px;
        display: flex;
        flex-direction: column;
        flex: 1;
        overflow-y: auto;
        gap: var(--spacing-xs);
        padding: var(--spacing-xs);
        border-radius: 0 0 var(--radius-s) var(--radius-s);
        border: 1px solid var(--glass-border);
    }

    .empty-state {
        flex: 1;
        display: flex;
        align-items: center;
        justify-content: center;
        color: var(--text-muted);
        font-size: var(--font-size-small);
    }

    .empty-state.error {
        color: var(--fail);
    }

    .mod-list.is-scrolling > :global(*) {
        pointer-events: none;
    }
</style>