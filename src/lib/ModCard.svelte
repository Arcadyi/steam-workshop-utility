<script lang="ts">
    import type { WorkshopItem, ItemStatus } from "$lib/types";
    import Checkbox from "$lib/ui/Checkbox.svelte";
    import { CheckIcon, CloseIcon, FolderIcon, RefreshIcon, SteamIcon, TrashIcon } from "$lib/icons";
    import Divider from "$lib/Divider.svelte";

    let {
        item,
        compact = false,
        onToggleSelect,
    }: {
        item: WorkshopItem;
        compact?: boolean;
        onToggleSelect?: (selected: boolean) => void;
    } = $props();

    let imageLoadFailed = $state(false);

    function formatSize(bytes: number): string {
        if (bytes >= 1024 * 1024 * 1024)
            return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
        return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    }

    function formatDate(ts: number | null): string {
        if (!ts) return "Unknown";
        return new Date(ts * 1000).toLocaleString(undefined, {
            year: "numeric",
            month: "short",
            day: "numeric",
            hour: "2-digit",
            minute: "2-digit"
        });
    }

    const statusColor: Record<ItemStatus, string> = {
        Unknown:    "var(--text-muted)",
        UpToDate:   "var(--success)",
        OutOfDate:  "var(--fail)",
    };

    const statusLabel: Record<ItemStatus, string> = {
        Unknown:   "Unknown",
        UpToDate:  "Up to date",
        OutOfDate: "Outdated",
    };
</script>

<div
        class="mod-card"
        class:compact
        class:selected={item.selected}
        role="button"
        tabindex="0"
        onclick={(e) => {
        // If the user clicked inside the checkbox, do nothing here (let the checkbox handle it)
        if ((e.target as HTMLElement).closest('.checkbox-wrapper')) return;

        onToggleSelect?.(!item.selected);
    }}
        onkeydown={(e) => {
        if ((e.key === "Enter" || e.key === " ")) {
            if ((e.target as HTMLElement).closest('.checkbox-wrapper')) return;
            onToggleSelect?.(!item.selected);
        }
    }}
>

    <div class="mod-card-left">
        <Checkbox
                checked={item.selected}
                onchange={(v) => onToggleSelect?.(v)}
        />
        {#if item.preview_url && !imageLoadFailed}
            <img
                    src={item.preview_url}
                    alt=""
                    class="mod-thumbnail"
                    onerror={() => imageLoadFailed = true}
            />
        {:else}
            <div class="mod-thumbnail-fallback">
                <SteamIcon width={compact ? 16 : 32} height={compact ? 16 : 32} />
            </div>
        {/if}
    </div>

    <div class="mod-card-body">
        <div class="mod-card-header">
            <span class="mod-name">{item.name ?? item.item_id}</span>
            {#if item.incompatible}
                <span class="mod-badge incompatible">Incompatible</span>
            {/if}
        </div>
        <span class="mod-status" style="color: {statusColor[item.status]}">
            {statusLabel[item.status]}
        </span>
        <Divider/>

        {#if !compact}
            <div class="mod-card-details">
                <span class="mod-detail">📦 {formatSize(item.disk_size)}</span>
                <span class="mod-detail">🕒 Local: {formatDate(item.local_timestamp)}</span>
                <span class="mod-detail">🔄 Remote: {formatDate(item.remote_timestamp)}</span>

                {#if item.supported_versions.length > 0}
                    <span class="mod-detail">🎮 {item.supported_versions.join(", ")}</span>
                {/if}
            </div>
        {/if}
    </div>

    <div class="mod-card-actions">
        <button class="action-btn" title="Open in Steam" onclick={(e) => { e.stopPropagation(); }}>
            <SteamIcon width={15} height={15} />
        </button>
        <button class="action-btn" title="Open folder" onclick={(e) => { e.stopPropagation(); }}>
            <FolderIcon width={15} height={15} />
        </button>
        <button class="action-btn" title="Force update" onclick={(e) => { e.stopPropagation(); }}>
            <RefreshIcon width={15} height={15} />
        </button>
        <button class="action-btn danger" title="Unsubscribe" onclick={(e) => { e.stopPropagation(); }}>
            <TrashIcon width={15} height={15} />
        </button>
    </div>

</div>

<style>
    .mod-card {
        display: flex;
        align-items: flex-start;
        gap: var(--spacing-xs);
        padding: var(--spacing-xs);
        border-radius: var(--radius-s);
        border: 1px solid var(--glass-border);
        background: var(--glass-bg-light);
        transition:
                background var(--animation-fast),
                border-color var(--animation-fast);
        position: relative;
    }

    .mod-card:hover {
        background: var(--glass-bg);
        border-color: var(--glass-border-light);
    }

    .mod-card.selected {
        border-color: var(--accent);
        background: var(--glass-bg);
    }

    .mod-card-left {
        display: flex;
        align-items: center;
        gap: var(--spacing-xs);
        flex-shrink: 0;
    }

    .mod-thumbnail {
        width: 100px;
        height: 100px;
        border-radius: var(--radius-xs);
        object-fit: cover;
        border: 1px solid var(--glass-border);
        flex-shrink: 0;
    }

    .mod-card.compact .mod-thumbnail {
        width: 32px;
        height: 32px;
        border-radius: 50%;
    }

    .mod-thumbnail-fallback {
        width: 100px;
        height: 100px;
        border-radius: var(--radius-xs);
        background: var(--bg-secondary);
        border: 1px solid var(--glass-border);
        flex-shrink: 0;
        display: grid;
        place-items: center;
        color: var(--text-muted);
    }

    .mod-card.compact .mod-thumbnail-fallback {
        width: 32px;
        height: 32px;
        border-radius: 50%;
    }

    .mod-card.compact .mod-thumbnail-fallback {
        width: 32px;
        height: 32px;
        border-radius: 50%;
    }

    .mod-card-body {
        display: flex;
        flex-direction: column;
        gap: var(--spacing-xxs);
        flex: 1;
        min-width: 0;
    }

    .mod-card-header {
        display: flex;
        align-items: center;
        gap: var(--spacing-xs);
        padding-bottom: var(--spacing-xxs);
        flex-wrap: wrap;
    }

    .mod-name {
        font-size: var(--font-size-small);
        font-weight: var(--font-weight-semibold);
        color: var(--text-primary);
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        flex: 1;
        min-width: 0;
    }

    .mod-status {
        font-size: var(--font-size-tiny);
        font-weight: var(--font-weight-medium);
        flex-shrink: 0;
    }

    .mod-badge {
        font-size: var(--font-size-tiny);
        padding: 1px var(--spacing-xxs);
        border-radius: var(--radius-xxl);
        flex-shrink: 0;
    }

    .mod-badge.incompatible {
        background: var(--fail);
        color: white;
    }

    .mod-card-details {
        display: flex;
        align-items: start;
        flex-direction: column;
        margin: var(--spacing-xxs);
        gap: var(--spacing-xxs);
        flex-wrap: wrap;
    }

    .mod-detail {
        font-size: var(--font-size-tiny);
        color: var(--text-muted);
        white-space: nowrap;
    }

    .mod-card-actions {
        display: flex;
        align-items: center;
        flex-direction: column;
        gap: var(--spacing-xxs);
        flex-shrink: 0;
        transition:
                opacity var(--animation-fast),
                x var(--animation-fast),
                y var(--animation-fast);
    }

    .mod-card.compact .mod-card-actions {
        flex-direction: row;
    }

    .action-btn {
        width: 28px;
        height: 28px;
        border-radius: var(--radius-xs);
        border: 1px solid var(--glass-border);
        background: transparent;
        color: var(--text-muted);
        display: grid;
        place-items: center;
        cursor: pointer;
        transition:
                background var(--animation-fast),
                color var(--animation-fast),
                border-color var(--animation-fast);
    }

    .action-btn:hover {
        background: var(--bg-secondary);
        color: var(--text-primary);
        border-color: var(--glass-border-light);
    }

    .action-btn.danger:hover {
        background: var(--fail);
        color: white;
        border-color: var(--fail);
    }
</style>