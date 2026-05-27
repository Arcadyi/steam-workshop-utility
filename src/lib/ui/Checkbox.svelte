<script lang="ts">
    let {
        checked = false,
        indeterminate = false,
        disabled = false,
        onchange,
        label = "",
    }: {
        checked?: boolean;
        indeterminate?: boolean;
        disabled?: boolean;
        onchange?: (checked: boolean) => void;
        label?: string;
    } = $props();

    function handleChange(e: Event) {
        const target = e.target as HTMLInputElement;
        onchange?.(target.checked);
    }
</script>

<label
        class="checkbox-wrapper"
        class:disabled
>
    <input
            type="checkbox"
            class="checkbox-input"
            {checked}
            {indeterminate}
            {disabled}
            onchange={handleChange}
            onclick={(e) => e.stopPropagation()}
    />
    <div class="checkbox-visual" class:checked class:indeterminate>
        {#if indeterminate}
            <svg viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg">
                <line x1="4" y1="8" x2="12" y2="8" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
            </svg>
        {:else if checked}
            <svg viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg">
                <polyline points="3,8 6.5,11.5 13,4.5" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
        {/if}
    </div>
    {#if label}
        <span class="checkbox-label">{label}</span>
    {/if}
</label>

<style>
    .checkbox-wrapper {
        display: inline-flex;
        align-items: center;
        gap: var(--spacing-xs);
        cursor: pointer;
        user-select: none;
    }

    .checkbox-wrapper.disabled {
        opacity: 0.5;
        cursor: not-allowed;
        pointer-events: none;
    }

    .checkbox-input {
        position: absolute;
        opacity: 0;
        width: 0;
        height: 0;
        pointer-events: none;
    }

    .checkbox-visual {
        width: 18px;
        height: 18px;
        border-radius: 50%;
        border: 2px solid var(--glass-border-light);
        background: var(--glass-bg);
        display: grid;
        place-items: center;
        flex-shrink: 0;
        color: var(--text-primary);
        transition:
                background var(--animation-fast),
                border-color var(--animation-fast);
        position: relative;
    }

    .checkbox-wrapper:hover .checkbox-visual {
        border-color: var(--accent);
    }

    .checkbox-visual.checked {
        background: var(--gradient-color);
        border-color: var(--accent);
    }

    .checkbox-visual.indeterminate {
        background: var(--glass-bg);
        border-color: var(--accent);
    }

    .checkbox-visual svg {
        width: 10px;
        height: 10px;
    }

    .checkbox-label {
        font-size: var(--font-size-small);
        color: var(--text-muted);
    }

    .checkbox-wrapper:hover .checkbox-label {
        color: var(--text-primary);
    }
</style>