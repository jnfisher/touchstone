<script lang="ts">
	// MME's Options menu (`mnuOptionsItems_Click`) in the header, with its captions and order. The NMR import and
	// export and Reload MME items have no counterpart here (see docs/differences.md).
	import { Button } from '$lib/components/ui/button';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
	import { getOptionsStore } from '$lib/options.svelte';
	import { getSettingsStore } from '$lib/settings.svelte';
	import { getDamageStore } from '$lib/monsterdamage.svelte';
	import { onMount } from 'svelte';

	const opts = getOptionsStore();
	const settings = getSettingsStore();
	const damage = getDamageStore();
	onMount(() => {
		void settings.load();
		void damage.refresh();
		void opts.refreshReach();
	});
</script>

<DropdownMenu.Root>
	<DropdownMenu.Trigger data-testid="options-menu">
		{#snippet child({ props })}
			<Button {...props} variant="outline" size="sm">Options</Button>
		{/snippet}
	</DropdownMenu.Trigger>
	<DropdownMenu.Content align="end" class="min-w-80">
		<DropdownMenu.Item data-testid="opt-paste" onSelect={() => opts.pasteCharacter()}>Paste Character</DropdownMenu.Item>
		<DropdownMenu.Item data-testid="opt-reach" disabled={opts.reachBusy} onSelect={() => void opts.calculateReachability()}>
			Calculate Reachability
		</DropdownMenu.Item>
		<DropdownMenu.Item data-testid="opt-damage-char" disabled={!opts.canCalcDamage} onSelect={() => void opts.calcDamage('char')}>
			{opts.charDamageLabel}
		</DropdownMenu.Item>
		<DropdownMenu.Item data-testid="opt-damage-party" disabled={!opts.canCalcPartyDamage} onSelect={() => void opts.calcDamage('party')}>
			{opts.partyDamageLabel}
		</DropdownMenu.Item>
		<DropdownMenu.Item data-testid="opt-damage-clear" disabled={!!damage.busy} onSelect={() => void opts.clearDamage()}>
			Clear Calculated Monster Dmg
		</DropdownMenu.Item>
		<DropdownMenu.Item data-testid="opt-clear-spells" onSelect={() => void opts.clearLearnedSpells()}>
			Clear Learned Spells
		</DropdownMenu.Item>
		<DropdownMenu.Item data-testid="opt-reset-char" onSelect={() => void opts.resetAllCharacterFields()}>
			Reset All Character Fields
		</DropdownMenu.Item>
		<DropdownMenu.Separator />
		<DropdownMenu.Item data-testid="opt-remove-filters" onSelect={() => opts.removeAllFilters()}>Remove All Filters</DropdownMenu.Item>
		<DropdownMenu.Item data-testid="opt-settings" onSelect={() => opts.openSettings()}>Settings</DropdownMenu.Item>
		<DropdownMenu.Separator />
		<DropdownMenu.CheckboxItem
			data-testid="opt-jump"
			checked={opts.jumpToCompare}
			onCheckedChange={() => void opts.toggleJumpToCompare()}
		>
			Jump to Equip/Lists on Add
		</DropdownMenu.CheckboxItem>
		<DropdownMenu.CheckboxItem
			data-testid="opt-shops-first"
			checked={opts.shopsFirst}
			onCheckedChange={() => void opts.toggleShopsFirst()}
		>
			Show shops first in item references
		</DropdownMenu.CheckboxItem>
	</DropdownMenu.Content>
</DropdownMenu.Root>
