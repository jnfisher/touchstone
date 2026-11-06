import { isCalc } from '$lib/calc';

export const match = (param: string): boolean => isCalc(param);
