<script lang="ts">
	// frmCoinConvert: two boxes that convert into each other, each with its own coin.
	import { onMount, untrack } from 'svelte';
	import {
		calcClient,
		COIN_DEFAULTS,
		COINS,
		coinCharm,
		coinButtonRequest,
		coinIndex,
		coinTypedRequest,
		type CharmButton,
		type CoinForm
	} from '$lib/calc';
	import CalcFrame from './CalcFrame.svelte';
	import { useCurrentCharacter } from './current.svelte';
	import UseCharacter from './UseCharacter.svelte';
	import { headingCls, inputCls } from './styles';

	let form = $state<CoinForm>({ text: [...COIN_DEFAULTS.text], coin: [...COIN_DEFAULTS.coin] });
	let weights = $state<[string, string]>(['', '']);
	let error = $state<string>();
	let timer: ReturnType<typeof setTimeout> | undefined;
	let ticket = 0;

	// cmdCharm: the character's Charm (while its global filter is on) makes a markup or discount.
	const current = useCurrentCharacter();
	const charm = $derived(current.active ? coinCharm(current.character) : undefined);
	let charmButton = $state<CharmButton>({ enabled: false, caption: 'No Charm Effect', percent: null });

	const other = (box: 0 | 1): 0 | 1 => (box === 0 ? 1 : 0);

	const charmField = () => (charm === undefined ? {} : { charm });

	function fail(e: unknown) {
		error = e instanceof Error ? e.message : String(e);
	}

	// Typing in one box fills the other after a short pause (CalcDelay / Timer1 in MME).
	function typed(box: 0 | 1) {
		clearTimeout(timer);
		const mine = ++ticket;
		const body = { ...coinTypedRequest(form, box), ...charmField() };
		timer = setTimeout(() => {
			calcClient
				.coin(body)
				.then((r) => {
					if (mine !== ticket) return;
					form.text[other(box)] = r.destination_text;
					weights = r.weights;
					charmButton = r.charm;
					error = undefined;
				})
				.catch(fail);
		}, 120);
	}

	// A coin button re-expresses its own box's amount in the new coin.
	function pick(box: 0 | 1, coin: number) {
		const previous = form.coin[box];
		if (previous === coin) return;
		form.coin[box] = coin;
		const mine = ++ticket;
		calcClient
			.coin({ ...coinButtonRequest(form, box, previous), ...charmField() })
			.then((r) => {
				if (mine !== ticket) return;
				form.text[box] = r.destination_text;
				form.coin[box] = coinIndex(r.current_coin);
				weights[box] = r.weights[box];
				charmButton = r.charm;
				error = undefined;
			})
			.catch(fail);
	}

	// Apply the discount or markup to the bottom amount.
	function applyCharm() {
		const mine = ++ticket;
		calcClient
			.coin({
				source: 1,
				text: form.text[1],
				top: form.coin[0],
				bottom: form.coin[1],
				apply_charm: true,
				...charmField()
			})
			.then((r) => {
				if (mine !== ticket) return;
				form.text[1] = r.destination_text;
				weights[1] = r.weights[1];
				charmButton = r.charm;
				error = undefined;
			})
			.catch(fail);
	}

	onMount(() => () => clearTimeout(timer));
	// Load, and whenever the character (so its Charm) changes, ask again so the button is current.
	$effect(() => {
		charm;
		untrack(() => typed(0));
	});

	const rows = [
		{ box: 0 as const, name: 'Top' },
		{ box: 1 as const, name: 'Bottom' }
	];
</script>

<CalcFrame title="Coin Converter" testid="calc-coin">
	{#snippet inputs()}
		<div class="flex flex-col gap-4">
			<UseCharacter {current} />
			{#each rows as row, i (row.box)}
				<fieldset class="flex flex-col gap-2">
					<legend class="sr-only">{row.name} amount</legend>
					<input
						type="text"
						inputmode="decimal"
						autocomplete="off"
						aria-label="{row.name} amount"
						bind:value={form.text[row.box]}
						oninput={() => typed(row.box)}
						data-testid="coin-{row.box}"
						class="{inputCls} h-10 text-lg"
					/>
					<div class="flex flex-wrap gap-1" role="radiogroup" aria-label="{row.name} coin">
						{#each COINS as c, idx (c.name)}
							<button
								type="button"
								role="radio"
								aria-checked={form.coin[row.box] === idx}
								onclick={() => pick(row.box, idx)}
								data-testid="coin-{row.box}-{c.name.toLowerCase()}"
								class="h-7 rounded-md border px-2.5 text-xs transition-colors {form.coin[row.box] === idx
									? 'bg-primary text-primary-foreground border-transparent'
									: 'bg-background hover:bg-accent'}"
							>
								{c.name}
							</button>
						{/each}
					</div>
					<p class="text-muted-foreground h-4 text-xs tabular-nums" data-testid="coin-{row.box}-weight">
						{weights[row.box]}
					</p>
				</fieldset>
				{#if i === 0}
					<p class="text-muted-foreground text-center text-sm select-none" aria-hidden="true">
						\/ Convert /\
					</p>
				{/if}
			{/each}
			<button
				type="button"
				class="bg-secondary text-secondary-foreground min-h-9 rounded-md px-3 py-1 text-sm whitespace-pre-line disabled:opacity-50"
				disabled={!charmButton.enabled || charm === undefined}
				onclick={applyCharm}
				data-testid="coin-charm"
			>
				{charm === undefined ? 'No Charm Effect' : charmButton.caption.replace(/\r\n/g, '\n')}
			</button>
			{#if error}<p class="text-destructive text-sm" role="alert">{error}</p>{/if}
		</div>
	{/snippet}
	{#snippet results()}
		<h3 class={headingCls}>The currency conversion rates are</h3>
		<ul class="flex flex-col gap-1.5 text-sm">
			<li><span class="tabular-nums">100</span> platinum pieces == 1 runic coin</li>
			<li><span class="tabular-nums">100</span> gold crowns == 1 platinum piece</li>
			<li><span class="tabular-nums">10</span> silver nobles == 1 gold crown</li>
			<li><span class="tabular-nums">10</span> copper farthings == 1 silver noble</li>
		</ul>
		<p class="text-muted-foreground mt-4 text-xs">
			Type in either box and the other follows. A coin button shows the same amount in that coin. The
			copper total is capped at 9,999,999,999. Weight is the coin count divided by 3, rounded down.
		</p>
	{/snippet}
</CalcFrame>
