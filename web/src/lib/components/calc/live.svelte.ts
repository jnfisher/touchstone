// Recomputes whenever the request changes, after a short pause so typing does not fire a request
// per key (MME waits on a timer too). Only the newest answer is kept.
export function live<Req, Res>(req: () => Req | undefined, call: (r: Req) => Promise<Res>, delay = 120) {
	let result = $state.raw<Res>();
	let error = $state<string>();
	let ticket = 0;
	$effect(() => {
		const r = req();
		if (r === undefined) return;
		const mine = ++ticket;
		const t = setTimeout(() => {
			call(r)
				.then((out) => {
					if (mine !== ticket) return;
					result = out;
					error = undefined;
				})
				.catch((e: unknown) => {
					if (mine !== ticket) return;
					error = e instanceof Error ? e.message : String(e);
				});
		}, delay);
		return () => clearTimeout(t);
	});
	return {
		get result() {
			return result;
		},
		get error() {
			return error;
		}
	};
}
