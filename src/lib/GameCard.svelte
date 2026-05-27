<script lang="ts">
    import { steamHeaderUrl, steamIconUrl } from "$lib/steam";
    import { GamepadIcon, CheckIcon, CloseIcon } from "$lib/icons";
    import type { GameEntry } from "$lib/types";
    import {onMount} from "svelte";
    import {invoke} from "@tauri-apps/api/core";
    import {error} from "@sveltejs/kit";

    let { game, active = false, expanded = true, onclick }: {
        game: GameEntry;
        active?: boolean;
        expanded?: boolean;
        onclick?: () => void;
    } = $props();

    const iconUrl = $derived(steamIconUrl(game.appid));
    const headerUrl = $derived(steamHeaderUrl(game.appid));

    let iconError = $state(false);
    let headerError = $state(false);
</script>

<button
        class="game-card"
        class:active
        class:expanded
        {onclick}
        title={game.name}
        aria-pressed={active}
        aria-expanded={expanded}
>
    {#if iconUrl && !iconError}
        <img
                src={iconUrl}
                alt=""
                class="game-icon"
                onerror={() => { iconError = true; }}
        />
    {:else}
        <div class="game-icon-fallback" aria-hidden="true">
            <GamepadIcon width={20} height={20} />
        </div>
    {/if}

    <div class="game-card-expanded-view">
        {#if !headerError}
            <img
                    src={headerUrl}
                    alt=""
                    class="game-header-bg"
                    onerror={() => { headerError = true; }}
            />
        {:else}
            <div class="game-header-fallback">
                <GamepadIcon width={32} height={32} />
            </div>
        {/if}

        <div class="game-name-row">
            <span class="game-name">{game.name}</span>
        </div>
        <div class="game-details">
            <span class="item-count">{game.num_items} items</span>
            <div class="badge success">
                <span aria-hidden="true"><CheckIcon width={12} height={12} /></span>
            </div>

            <div class="badge fail">
                <span aria-hidden="true"><CloseIcon width={12} height={12} /></span>
                <span>{game.num_ood}</span>
            </div>
        </div>
    </div>
</button>

<style>
    .game-card {
        display: flex;
        align-items: center;
        gap: var(--spacing-xs);
        border-radius: var(--radius-xl);
        border: none;
        border-left: 2px solid transparent;
        background: var(--glass-bg);
        width: 100%;
        color: var(--text-muted);
        cursor: pointer;
        overflow: hidden;
        white-space: nowrap;
        flex-shrink: 0;
        position: relative;
        transition:
                background var(--animation-slow),
                border-color var(--animation-normal),
                color var(--animation-slow);
    }

    /* Active gradient overlay */
    .game-card::before {
        content: '';
        position: absolute;
        inset: 0;
        background: var(--gradient-color);
        opacity: 0;
        transition: opacity var(--animation-fast);
        pointer-events: none;
        z-index: 0;
    }

    .game-card.active::before  { opacity: 1; }
    .game-card.expanded        { border-radius: var(--radius-xs); }

    .game-card:hover            { background: var(--bg-secondary); color: var(--text-primary); }
    .game-card.active           { border-left-color: var(--accent); color: var(--text-primary); }

    .game-icon {
        width: 32px;
        height: 32px;
        border-radius: 50%;
        border: 1px solid var(--glass-border-light);
        flex-shrink: 0;
        object-fit: cover;
        position: relative;
        z-index: 1;
        box-shadow: var(--shadow-down);
        transition: width var(--animation-slow), height var(--animation-slow);
    }

    .game-icon-fallback {
        width: 32px;
        height: 32px;
        border-radius: 50%;
        border: 1px solid var(--glass-border-light);
        flex-shrink: 0;
        display: grid;
        place-items: center;
        background: var(--bg-secondary);
        color: var(--text-muted);
        position: relative;
        z-index: 1;
        transition: width var(--animation-slow), height var(--animation-slow);
    }

    .game-card-expanded-view {
        display: flex;
        flex-direction: column;
        justify-content: flex-end;
        position: relative;
        z-index: 1;
        opacity: 0;
        width: 0;
        height: 0;
        overflow: hidden;
        pointer-events: none;
        background-size: cover;
        background-position: center;
        transition:
                opacity var(--animation-slow),
                width var(--animation-slow),
                height var(--animation-slow);
        border-top: 1px solid var(--glass-border-light);
        border-right: 1px solid var(--glass-border-light);
        border-bottom: 1px solid var(--glass-border-light);
        border-left: 1px solid var(--glass-border-light);
        border-radius: var(--spacing-xs);
    }

    .game-card.expanded .game-card-expanded-view {
        opacity: 1;
        width: 211px;
        height: 98px;
        pointer-events: auto;
    }

    .game-header-bg {
        position: absolute;
        inset: 0;
        width: 100%;
        height: 100%;
        object-fit: cover;
        z-index: 0;
    }

    .game-header-fallback {
        position: absolute;
        inset: 0;
        display: grid;
        place-items: center;
        background: var(--bg-secondary);
        color: var(--text-muted);
    }

    .game-name-row,
    .game-details {
        position: relative;
        z-index: 1;
    }

    /* ── Name row ───────────────────────────────────── */
    .game-name-row {
        padding: var(--spacing-xs);
        background: var(--gradient-bottom-fade);
        border-bottom: none;
        border-radius: var(--radius-xs) var(--radius-xs) 0 0;
    }

    .game-name {
        display: block;
        font-size: var(--font-size-small);
        font-weight: var(--font-weight-bold);
        color: var(--text-primary);
        overflow: hidden;
        text-overflow: ellipsis;
    }

    .game-details {
        display: flex;
        align-items: center;
        gap: 0;
        height: 20px;
        background: var(--glass-bg);
        border: 1px solid var(--glass-border);
        border-top: none;
        border-radius: 0 0 var(--radius-xs) var(--radius-xs);
    }

    .item-count {
        flex: 1;
        font-size: var(--font-size-small);
        color: var(--text-muted);
        padding-inline: var(--spacing-xs);
        overflow: hidden;
        text-overflow: ellipsis;
    }

    .badge {
        display: flex;
        align-items: center;
        gap: var(--spacing-xxs);
        padding-inline: var(--spacing-xs);
        height: 100%;
        color: var(--text-primary);
        font-size: var(--font-size-small);
        border-left: 1px solid var(--glass-border-light);
    }

    .badge.success { background: var(--success); }

    .badge.fail {
        background: var(--fail);
        border-radius: 0 0 var(--radius-xs) 0;
    }
</style>