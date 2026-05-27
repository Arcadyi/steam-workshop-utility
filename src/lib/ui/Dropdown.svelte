<script lang="ts">
    import { onMount } from "svelte";
    import {ChevronnDownIcon} from "$lib/icons";

    let {
        options = [],
        value = $bindable(""),
        placeholder = "Select...",
        onchange,
    }: {
        options: { label: string; value: string }[];
        value?: string;
        placeholder?: string;
        onchange?: (value: string) => void;
    } = $props();

    let open = $state(false);
    let dropdownEl = $state<HTMLDivElement | null>(null);

    const selected = $derived(options.find(o => o.value === value) ?? null);

    function select(option: { label: string; value: string }) {
        value = option.value;
        onchange?.(option.value);
        open = false;
    }

    function toggle() {
        open = !open;
    }

    // close when clicking outside
    function handleClickOutside(e: MouseEvent) {
        if (dropdownEl && !dropdownEl.contains(e.target as Node)) {
            open = false;
        }
    }

    // close on Escape
    function handleKeydown(e: KeyboardEvent) {
        if (e.key === "Escape") open = false;
    }

    onMount(() => {
        document.addEventListener("mousedown", handleClickOutside);
        document.addEventListener("keydown", handleKeydown);
        return () => {
            document.removeEventListener("mousedown", handleClickOutside);
            document.removeEventListener("keydown", handleKeydown);
        };
    });
</script>

<div class="dropdown" bind:this={dropdownEl}>

    <!-- Trigger button -->
    <button class="dropdown-trigger" onclick={toggle} aria-haspopup="listbox" aria-expanded={open}>
    <span class="dropdown-value">
      {selected ? selected.label : placeholder}
    </span>
        <span class="dropdown-chevron" class:open>
      <ChevronnDownIcon width={14} height={14} />
    </span>
    </button>

    <!-- Options list -->
    {#if open}
        <div class="dropdown-menu" role="listbox">
            {#each options as option (option.value)}
                <button
                        class="dropdown-option"
                        class:selected={option.value === value}
                        role="option"
                        aria-selected={option.value === value}
                        onclick={() => select(option)}
                >
                    {option.label}
                </button>
            {/each}
        </div>
    {/if}

</div>

<style>
    .dropdown {
        position: relative;
        display: inline-block;
    }

    .dropdown-trigger {
        display: flex;
        align-items: center;
        gap: var(--spacing-xs);
        padding: 0 var(--spacing-xs);
        height: 32px;
        border-radius: var(--radius-s);
        border: 1px solid var(--glass-border);
        background: var(--glass-bg);
        color: var(--text-primary);
        cursor: pointer;
        font-size: var(--font-size-small);
        white-space: nowrap;
        transition:
                background var(--animation-fast),
                border-color var(--animation-fast);
    }

    .dropdown-trigger:hover {
        border-color: var(--accent);
        background: var(--bg-secondary);
    }

    .dropdown-chevron {
        display: grid;
        place-items: center;
        color: var(--text-muted);
        transition: transform var(--animation-fast);
    }

    .dropdown-chevron.open {
        transform: rotate(180deg);
    }

    .dropdown-menu {
        position: absolute;
        top: calc(100% + 4px);
        left: 0;
        min-width: 100%;
        background: var(--glass-bg);
        backdrop-filter: blur(var(--glass-blur));
        -webkit-backdrop-filter: blur(var(--glass-blur));
        border: 1px solid var(--glass-border);
        border-radius: var(--radius-s);
        box-shadow: var(--glass-shadow);
        z-index: 100;
        overflow: hidden;
        display: flex;
        flex-direction: column;
    }

    .dropdown-option {
        padding: var(--spacing-xxs) var(--spacing-xs);
        font-size: var(--font-size-small);
        color: var(--text-muted);
        text-align: left;
        background: transparent;
        border: none;
        border-radius: 0;
        cursor: pointer;
        transition:
                background var(--animation-fast),
                color var(--animation-fast);
        white-space: nowrap;
    }

    .dropdown-option:hover {
        background: var(--bg-secondary);
        color: var(--text-primary);
    }

    .dropdown-option.selected {
        color: var(--text-primary);
        background: var(--gradient-color);
    }
</style>