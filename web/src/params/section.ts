import { isSection } from '$lib/sections';

export const match = (param: string): boolean => isSection(param);
