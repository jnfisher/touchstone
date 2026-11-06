<script lang="ts">
	// frmEditPresets: map, room and caption of one preset, with "Get Current Rooms Stats" filling them from the room shown.
	import { Button } from '$lib/components/ui/button';
	import { inputCls, labelCls } from '$lib/components/calc/styles';
	import type { Preset, PresetDraft } from '$lib/map';

	let {
		preset,
		current,
		onsave,
		oncancel
	}: {
		preset: Preset;
		/** The room the map shows, with its name, for the Get button. */
		current: { map: number; room: number; name: string } | undefined;
		onsave: (d: PresetDraft) => void;
		oncancel: () => void;
	} = $props();

	// the dialog is mounted per edit, so the starting values are read once
	/* svelte-ignore state_referenced_locally */
	let name = $state(preset.name);
	/* svelte-ignore state_referenced_locally */
	let map = $state(String(preset.map));
	/* svelte-ignore state_referenced_locally */
	let room = $state(String(preset.room));
</script>

<div class="bg-background/80 fixed inset-0 z-[60] flex items-center justify-center p-4" role="presentation">
	<!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
	<form
		class="bg-card flex w-full max-w-sm flex-col gap-3 rounded-lg border p-4 shadow-xl"
		role="dialog"
		aria-modal="true"
		aria-label="Editing Preset #{preset.index + 1}"
		data-testid="preset-dialog"
		onsubmit={(e) => {
			e.preventDefault();
			onsave({ name, map, room });
		}}
	>
		<h2 class="text-base font-semibold">Editing Preset #{preset.index + 1}</h2>
		<div class="flex gap-2">
			<label class="w-24"><span class={labelCls}>Map</span><input class={inputCls} inputmode="numeric" aria-label="Preset map" bind:value={map} /></label>
			<label class="w-24"><span class={labelCls}>Room</span><input class={inputCls} inputmode="numeric" aria-label="Preset room" bind:value={room} /></label>
		</div>
		<label><span class={labelCls}>Caption</span><input class={inputCls} aria-label="Preset caption" bind:value={name} /></label>
		<div class="flex flex-wrap justify-between gap-2">
			<Button
				type="button"
				variant="outline"
				size="sm"
				disabled={!current}
				data-testid="preset-get"
				onclick={() => {
					if (!current) return;
					map = String(current.map);
					room = String(current.room);
					name = current.name;
				}}>Get Current Rooms Stats</Button>
			<span class="flex gap-2">
				<Button type="submit" size="sm" data-testid="preset-ok">OK</Button>
				<Button type="button" variant="outline" size="sm" onclick={oncancel}>Cancel</Button>
			</span>
		</div>
	</form>
</div>
