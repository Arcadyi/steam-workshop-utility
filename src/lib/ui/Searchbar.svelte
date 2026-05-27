<script lang="ts">

    import {SearchIcon} from "$lib/icons";

    let {
        value = $bindable(""),
        placeholder = "Search...",
        onchange,
    }: {
        value?: string;
        placeholder?: string;
        onchange?: (value: string) => void;
    } = $props();
</script>

<div class="search-bar">
  <span class="search-icon">
    <SearchIcon width={14} height={14} />
  </span>
    <input
            type="text"
            class="search-input"
            bind:value
            {placeholder}
            oninput={() => onchange?.(value)}
    />
    {#if value}
        <button class="search-clear" onclick={() => { value = ""; onchange?.(""); }}>
            ✕
        </button>
    {/if}
</div>

<style>
    .search-bar {
        display: flex;
        align-items: center;
        gap: var(--spacing-xs);
        padding: 0 var(--spacing-xs);
        height: 32px;
        border-radius: var(--radius-s);
        border: 1px solid var(--glass-border);
        background: var(--glass-bg);
        transition:
                border-color var(--animation-fast),
                background var(--animation-fast);
        flex: 1;
    }

    .search-bar:focus-within {
        border-color: var(--accent);
        background: var(--bg-secondary);
    }

    .search-icon {
        color: var(--text-muted);
        display: grid;
        place-items: center;
        flex-shrink: 0;
    }

    .search-input {
        flex: 1;
        background: transparent;
        border: none;
        outline: none;
        color: var(--text-primary);
        font-size: var(--font-size-small);
        font-family: inherit;
        min-width: 0;
    }

    .search-input::placeholder {
        color: var(--text-muted);
    }

    .search-clear {
        background: transparent;
        border: none;
        color: var(--text-muted);
        cursor: pointer;
        font-size: 0.7rem;
        display: grid;
        place-items: center;
        padding: 0;
        flex-shrink: 0;
        transition: color var(--animation-fast);
    }

    .search-clear:hover {
        color: var(--text-primary);
    }
</style>