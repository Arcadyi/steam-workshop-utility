<script lang="ts">
    import Checkbox from "$lib/ui/Checkbox.svelte";
    import Dropdown from "$lib/ui/Dropdown.svelte";
    import Searchbar from "$lib/ui/Searchbar.svelte";
    import {CompactIcon, ExportIcon, ImportIcon} from "$lib/icons";

    let {
        allSelected = false,
        someSelected = false,
        searchQuery = $bindable(""),
        compact = $bindable(false),
        sortBy = $bindable("name"),
        onSelectAll,
        onExport,
        onImport,
    }: {
        allSelected?: boolean;
        someSelected?: boolean;
        searchQuery?: string;
        compact?: boolean;
        sortBy?: string;
        onSelectAll?: (checked: boolean) => void;
        onExport?: () => void;
        onImport?: () => void;
    } = $props();

    const sortOptions = [
        { label: "Name",         value: "name"         },
        { label: "Last Updated", value: "last_updated"  },
        { label: "Size",         value: "size"          },
        { label: "Status",       value: "status"        },
    ];
</script>

<div class="topbar">

    <Checkbox
            checked={allSelected}
            indeterminate={someSelected && !allSelected}
            onchange={onSelectAll}
    />

    <Searchbar bind:value={searchQuery} placeholder="Search mods..." />

    <div class="topbar-right">
        <Dropdown
                options={sortOptions}
                bind:value={sortBy}
                placeholder="Sort by..."
        />

        <button class="topbar-btn" onclick={onImport}>
            <ImportIcon width={15} height={15} />
            <span>Import</span>
        </button>

        <button class="topbar-btn" onclick={onExport}>
            <ExportIcon width={15} height={15} />
            <span>Export</span>
        </button>
        <button
                class="topbar-btn"
                class:active={compact}
                onclick={() => compact = !compact}
                title="Toggle compact view"
        >
            <CompactIcon width={15} height={15} />
        </button>
    </div>

</div>

<style>
    .topbar {
        display: flex;
        align-items: center;
        gap: var(--spacing-xs);
        padding: var(--spacing-xs);
        border: 1px solid var(--glass-border);
        border-radius: var(--radius-m) var(--radius-m) 0 0;
        background: var(--glass-bg);
        flex-shrink: 0;
    }

    .topbar-btn.active {
        border-color: var(--accent);
        color: var(--text-primary);
        background: var(--bg-secondary);
    }

    .topbar-right {
        display: flex;
        align-items: center;
        gap: var(--spacing-xs);
        margin-left: auto;
    }

    .topbar-btn {
        display: flex;
        align-items: center;
        gap: var(--spacing-xxs);
        padding: 0 var(--spacing-xs);
        height: 32px;
        border-radius: var(--radius-s);
        border: 1px solid var(--glass-border);
        background: var(--glass-bg);
        color: var(--text-muted);
        font-size: var(--font-size-small);
        cursor: pointer;
        transition:
                background var(--animation-fast),
                border-color var(--animation-fast),
                color var(--animation-fast);
    }

    .topbar-btn:hover {
        border-color: var(--accent);
        background: var(--bg-secondary);
        color: var(--text-primary);
    }
</style>