<script lang="ts">
    let { vertical = false, margin = true } = $props();
</script>

<div class="divider" class:vertical class:margin></div>

<style>
    .divider {
        background: var(--divider-color);
        flex-shrink: 0;
    }

    /* Horizontal (default) */
    .divider:not(.vertical) {
        width: 100%;
        height: 1px;
    }

    /* Vertical */
    .divider.vertical {
        width: 1px;
        height: 100%;
    }

    /* Optional margin so you don't have to add it at the usage site */
    .divider:not(.vertical).margin {
        margin: var(--spacing-xxxxs) 0;
    }

    .divider.vertical.margin {
        margin: 0 var(--spacing-xxxxs);
    }
</style>