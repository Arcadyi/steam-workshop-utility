<script lang="ts">
    import { getCurrentWindow } from "@tauri-apps/api/window";
    import {CloseIcon, MaximizeIcon, MinimizeIcon, SteamIcon} from "$lib/icons";

    const appWindow = getCurrentWindow();

    async function minimize() {
        await appWindow.minimize();
    }

    async function maximize() {
        await appWindow.toggleMaximize();
    }

    async function close() {
        await appWindow.close();
    }
</script>

<div class="titlebar" data-tauri-drag-region>
    <!-- Left -->
    <div class="titlebar-left">
        <SteamIcon
                width={32}
                height={32}
        />
        <span class="app-name">SWU</span>
        <span class="app-version">v0.7.4</span>
    </div>

    <!-- Center -->
    <div class="titlebar-center" data-tauri-drag-region>
        <span class="app-title">Steam Workshop Utility</span>
    </div>

    <!-- Right -->
    <div class="titlebar-right">
        <button class="titlebar-button" onclick={minimize}>
            <MinimizeIcon
                    width={16}
                    height={16}
            />
        </button>
        <button class="titlebar-button" onclick={maximize}>
            <MaximizeIcon
                    width={15}
                    height={15}
            />
        </button>

        <button class="red-button titlebar-button" onclick={close}>
            <CloseIcon
                    width={16}
                    height={16}
            />
        </button>
    </div>

</div>

<style>
    .titlebar {
        height: 48px;
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: var(--spacing-s) var(--spacing-xs);
        user-select: none;
    }

    .titlebar-left,
    .titlebar-right {
        display: flex;
        align-items: center;
        width: 200px; /* equal width keeps center truly centered */
    }

    .titlebar-right {
        justify-content: flex-end;
    }

    .titlebar-center {
        flex: 1;
        text-align: center;
    }

    .titlebar-button {
        width: 32px;
        height: 32px;
        border: 1px solid var(--glass-border);
        border-radius: 0;
        background: transparent;
        color: var(--text-primary);
        cursor: pointer;
        display: grid;
        place-items: center;
        margin-left: -1px;
        transition:
                background var(--animation-slow),
                width var(--animation-slow),
                height var(--animation-slow);
        box-shadow: var(--glass-shadow);
    }

    .titlebar-button:hover {
        background: var(--accent);
        width: 34px;
        height: 34px;
    }

    .titlebar-button:active {
        background: var(--bg-secondary);
    }

    .titlebar-button:first-child {
        border-radius: 50% 5% 5% 50%;
    }

    .titlebar-button:last-child {
        border-radius: 5% 50% 50% 5%;
    }

    .titlebar-button.red-button:hover {
        background: var(--accent-secondary);
    }

    .titlebar-button.red-button:active {
        background: var(--bg-secondary);
    }

    .app-name {
        font-weight: var(--font-weight-heavy);
        font-size: var(--font-size-subheader);
        padding-left: var(--spacing-xxxs);
    }

    .app-version {
        padding-top: 5px;
        padding-left: var(--spacing-xs);
        font-size: var(--font-size-tiny);
        color: var(--text-muted);
    }

    .app-title {
        font-size: 0.85rem;
        color: var(--text-muted);
    }

</style>