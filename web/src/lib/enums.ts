// Display names for the numeric codes in the database. Ported from the modMMudFunc Get*Enum
// functions, including their "Unknown (n)" fallback.

const unknown = (n: number) => `Unknown (${n})`;
const pick = (names: Record<number, string>, n: number) => names[n] ?? unknown(n);

export const weaponType = (n: number) =>
	pick({ 0: '1H Blunt', 1: '2H Blunt', 2: '1H Sharp', 3: '2H Sharp' }, n);

export const classWeaponType = (n: number) =>
	pick(
		{
			0: '1H Blunt',
			1: '2H Blunt',
			2: '1H Sharp',
			3: '2H Sharp',
			4: 'Any 1H',
			5: 'Any 2H',
			6: 'Any Sharp',
			7: 'Any Blunt',
			8: 'Any Weapon',
			9: 'Staff'
		},
		n
	);

export function armourType(n: number): string {
	if (n >= 3 && n <= 6) return 'Leather';
	return pick(
		{ 0: 'Natural', 1: 'Silk', 2: 'Ninja', 7: 'Chainmail', 8: 'Scalemail', 9: 'Platemail' },
		n
	);
}

export const wornType = (n: number) =>
	pick(
		{
			0: 'Nowhere',
			1: 'Everywhere',
			2: 'Head',
			3: 'Hands',
			4: 'Finger',
			5: 'Feet',
			6: 'Arms',
			7: 'Back',
			8: 'Neck',
			9: 'Legs',
			10: 'Waist',
			11: 'Torso',
			12: 'Off-Hand',
			13: 'Finger',
			14: 'Wrist',
			15: 'Ears',
			16: 'Worn',
			17: 'Wrist',
			18: 'Eyes',
			19: 'Face'
		},
		n
	);

/** MME falls back to the bare number for unnamed item types. */
export const itemType = (n: number) =>
	(
		{
			0: 'Armour',
			1: 'Weapon',
			2: 'Projectile',
			3: 'Sign',
			4: 'Food',
			5: 'Drink',
			6: 'Light',
			7: 'Key',
			8: 'Container',
			9: 'Scroll',
			10: 'Special'
		} as Record<number, string>
	)[n] ?? String(n);

export const costType = (n: number) =>
	pick({ 0: 'Copper', 1: 'Silver', 2: 'Gold', 3: 'Platinum', 4: 'Runic' }, n);

export function mageryName(n: number, level = 0): string {
	const base = pick({ 0: 'None', 1: 'Mage', 2: 'Priest', 3: 'Druid', 4: 'Bard', 5: 'Kai' }, n);
	return n === 0 ? base : `${base}-${level}`;
}

export const shopType = (n: number) =>
	pick(
		{
			0: 'General',
			1: 'Weapons',
			2: 'Armour',
			3: 'Items',
			4: 'Spells',
			5: 'Hospital',
			6: 'Tavern',
			7: 'Bank',
			8: 'Training',
			9: 'Inn',
			10: 'Specific',
			11: 'Gang Shop',
			12: 'Deed Shop'
		},
		n
	);

export const spellTargets = (n: number) =>
	pick(
		{
			0: 'User',
			1: 'Self',
			2: 'Self or User',
			3: 'Divided Area (not self)',
			4: 'Monster',
			5: 'Divided Area (incl self)',
			6: 'Any',
			7: 'Item',
			8: 'Monster or User',
			9: 'Divided Attack Area',
			10: 'Divided Party Area',
			11: 'Full Area',
			12: 'Full Attack Area',
			13: 'Full Party Area'
		},
		n
	);

/** Falls back to the bare number, like MME. */
export const spellAttackType = (n: number) =>
	(
		{ 0: 'Cold', 1: 'Fire', 2: 'Stone', 3: 'Lightning', 4: 'Normal', 5: 'Water', 6: 'Poison' } as Record<
			number,
			string
		>
	)[n] ?? String(n);

export const monsterType = (n: number) =>
	pick({ 0: 'Solo', 1: 'Leader', 2: 'Follower', 3: 'Stationary' }, n);

export const monsterAlignment = (n: number) =>
	pick(
		{
			0: 'Good',
			1: 'Evil',
			2: 'Chaotic Evil',
			3: 'Neutral',
			4: 'Lawful Good',
			5: 'Neutral Evil',
			6: 'Lawful Evil'
		},
		n
	);

export const monsterAttackType = (n: number) =>
	pick({ 0: 'None', 1: 'Normal', 2: 'Spell', 3: 'Rob' }, n);
