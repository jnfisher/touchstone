<script lang="ts">
	// The party screen's boxes: six members and the average row, as frmPasteChar lays them out.
	import { PARTY_COLUMNS, type PartyColumn, type PartyForm } from '$lib/paste';

	let {
		form = $bindable(),
		onedit,
		onmega
	}: {
		form: PartyForm;
		/** A box was edited: the server recounts the averages. */
		onedit: () => void;
		/** The paste button next to a damage box: reads a MegaMUD report from the clipboard. */
		onmega: (member: number, spell: boolean) => void;
	} = $props();

	const cell = 'bg-background border-input h-7 w-full min-w-[3.25rem] rounded border px-1 text-right text-xs tabular-nums';
	const members = [1, 2, 3, 4, 5, 6];
	const get = (c: PartyColumn, k: number) => form[c][k] ?? '';
	function set(c: PartyColumn, k: number, v: string) {
		form[c][k] = v;
	}
</script>

<div class="overflow-x-auto" data-testid="party-table">
	<table class="w-full border-separate border-spacing-x-1 border-spacing-y-1 text-xs">
		<thead>
			<tr class="text-muted-foreground text-left">
				<th class="w-40 font-medium">Member</th>
				{#each PARTY_COLUMNS as c (c.key)}
					<th class="text-right font-medium" title={c.help}>{c.label}</th>
				{/each}
				<th class="text-center font-medium" title="Anti-magic: the character counts toward the anti-magic total">AM</th>
				<th class="text-center font-medium" title="Selecting one character with attack last counts that character's defenses an extra time in the average">Last</th>
			</tr>
		</thead>
		<tbody>
			{#each members as k (k)}
				<tr data-testid="party-member-{k}">
					<td>
						<input
							class="{cell.replace('text-right', 'text-left')}"
							value={form.name[k] ?? ''}
							oninput={(e) => (form.name[k] = e.currentTarget.value)}
							onchange={onedit}
							aria-label="Member {k} name"
						/>
					</td>
					{#each PARTY_COLUMNS as c (c.key)}
						<td>
							<div class="flex items-center gap-0.5">
								<input
									class={cell}
									value={get(c.key, k)}
									oninput={(e) => set(c.key, k, e.currentTarget.value)}
									onchange={onedit}
									aria-label="Member {k} {c.label}"
								/>
								{#if c.key === 'dmg' || c.key === 'spell_dmg'}
									<button
										type="button"
										class="border-input h-7 shrink-0 rounded border px-1 text-[10px]"
										title="Paste a MegaMUD damage report from the clipboard"
										onclick={() => onmega(k, c.key === 'spell_dmg')}>Paste</button
									>
								{/if}
							</div>
						</td>
					{/each}
					<td class="text-center">
						<input
							type="checkbox"
							checked={(form.anti_magic[k] ?? 0) !== 0}
							onchange={(e) => {
								form.anti_magic[k] = e.currentTarget.checked ? 1 : 0;
								onedit();
							}}
							aria-label="Member {k} anti-magic"
						/>
					</td>
					<td class="text-center">
						<input
							type="radio"
							name="atk-last"
							checked={form.atk_last[k] ?? false}
							onchange={() => {
								form.atk_last = form.atk_last.map((_, i) => i === k);
								onedit();
							}}
							aria-label="Member {k} attacks last"
						/>
					</td>
				</tr>
			{/each}
			<tr class="font-medium" data-testid="party-average">
				<td class="text-muted-foreground pl-1">Average ({form.party_total || 0})</td>
				{#each PARTY_COLUMNS as c (c.key)}
					<td><div class="{cell} bg-muted flex items-center justify-end">{get(c.key, 0)}</div></td>
				{/each}
				<td class="text-center tabular-nums">{form.am_total}</td>
				<td></td>
			</tr>
		</tbody>
	</table>
</div>
