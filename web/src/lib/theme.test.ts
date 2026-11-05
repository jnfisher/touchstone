import { describe, expect, it } from 'vitest';
import { createThemeStore, readTheme, THEME_KEY, type Theme } from './theme.svelte';

function fakeStorage(initial: Record<string, string> = {}) {
	const data = { ...initial };
	return {
		data,
		getItem: (k: string) => data[k] ?? null,
		setItem: (k: string, v: string) => {
			data[k] = v;
		}
	};
}

describe('theme store', () => {
	it('is dark by default', () => {
		expect(readTheme(undefined)).toBe('dark');
		expect(createThemeStore(fakeStorage()).current).toBe('dark');
	});

	it('restores a saved light choice', () => {
		expect(createThemeStore(fakeStorage({ [THEME_KEY]: 'light' })).current).toBe('light');
	});

	it('toggles, persists and applies', () => {
		const storage = fakeStorage();
		const applied: Theme[] = [];
		const store = createThemeStore(storage, (t) => applied.push(t));
		store.toggle();
		expect(store.current).toBe('light');
		expect(storage.data[THEME_KEY]).toBe('light');
		store.toggle();
		expect(store.current).toBe('dark');
		expect(applied).toEqual(['dark', 'light', 'dark']);
	});

	it('survives storage that throws', () => {
		const broken = {
			getItem: () => {
				throw new Error('blocked');
			},
			setItem: () => {
				throw new Error('blocked');
			}
		};
		const store = createThemeStore(broken);
		store.toggle();
		expect(store.current).toBe('light');
	});
});
