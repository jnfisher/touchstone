export type Theme = 'dark' | 'light';

export const THEME_KEY = 'touchstone.theme';

/** Dark unless the saved choice says light; unreadable or odd values mean the default. */
export function readTheme(storage: Pick<Storage, 'getItem'> | undefined): Theme {
	try {
		return storage?.getItem(THEME_KEY) === 'light' ? 'light' : 'dark';
	} catch {
		return 'dark';
	}
}

export function createThemeStore(
	storage: Pick<Storage, 'getItem' | 'setItem'> | undefined,
	apply: (t: Theme) => void = () => {}
) {
	const initial = readTheme(storage);
	let current = $state<Theme>(initial);
	apply(initial);
	return {
		get current() {
			return current;
		},
		set(t: Theme) {
			current = t;
			try {
				storage?.setItem(THEME_KEY, t);
			} catch {
				// Private mode or blocked storage: the choice just lasts for this page.
			}
			apply(t);
		},
		toggle() {
			this.set(current === 'dark' ? 'light' : 'dark');
		}
	};
}

export type ThemeStore = ReturnType<typeof createThemeStore>;
