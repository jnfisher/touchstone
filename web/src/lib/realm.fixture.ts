// A trimmed response of GET /api/realm/characters/3 (R-001's fixture priest, Cormac), plus a roster, for the
// realm tests; the values are what the server answered for the fixture database.
import type { RealmDetail, RealmSummary } from './realm';

export const cormac = {
	"available": true,
	"id": 3,
	"identity": {
		"alignment": 2,
		"alignment_name": "neutral",
		"class": 5,
		"class_name": "Priest",
		"coins": [
			0,
			0,
			163,
			0,
			51
		],
		"death_date": 0,
		"evil": 0,
		"exp": 1410187,
		"first_name": "Cormac",
		"gang": "Ironwood",
		"last_name": "Fixture",
		"last_on": 1760010800,
		"last_on_iso": "2025-10-09T11:53:20Z",
		"lawful": 0,
		"level": 16,
		"lives": 9,
		"map": 1,
		"race": 2,
		"race_name": "Dwarf",
		"room": 1021,
		"title": "Adventurer",
		"total_cp": 260,
		"unspent_cp": 2
	},
	"weapon": {
		"item": 348,
		"mapped": true,
		"mme_slot": 16,
		"name": "platinum mace"
	},
	"worn": [
		{
			"item": 268,
			"mme_slot": 4,
			"mme_slot_name": "Torso",
			"name": "shimmering white robes",
			"slot": 0,
			"uses": -1
		},
		{
			"item": 401,
			"mme_slot": 0,
			"mme_slot_name": "Head",
			"name": "silvery skullcap",
			"slot": 1,
			"uses": 3
		},
		{
			"item": 335,
			"mme_slot": 8,
			"mme_slot_name": "Hands",
			"name": "padded gloves",
			"slot": 2,
			"uses": -1
		}
	],
	"spellbook": [
		{
			"name": "harm",
			"spell": 12
		},
		{
			"name": "minor healing",
			"spell": 13
		},
		{
			"name": "bless",
			"spell": 14
		}
	],
	"active_spells": [
		{
			"bless_slot": 0,
			"name": "greater bless",
			"rounds": 62,
			"slot": 0,
			"spell": 146,
			"value": 6
		}
	],
	"abilities": [],
	"unmapped": [
		{
			"id": 30,
			"kind": "worn",
			"name": "skullcap",
			"reason": "MME holds one item in its Head slot, which already has silvery skullcap"
		}
	],
	"differs": 1,
	"sheet_error": null,
	"inventory": [
		{
			"item": 348,
			"name": "platinum mace",
			"readied": true,
			"slot": 0,
			"uses": 10
		},
		{
			"item": 268,
			"name": "shimmering white robes",
			"readied": false,
			"slot": 1,
			"uses": -1
		}
	],
	"game": {
		"hp": 129,
		"mana": 122,
		"martial": 19,
		"max_hp": 129
	},
	"sheet": null,
	"comparison": [
		{
			"differs": false,
			"field": "str",
			"game": 60,
			"label": "Strength",
			"tolerance": "exact (whole numbers)",
			"touchstone": 60.0
		},
		{
			"differs": false,
			"field": "max_hp",
			"game": 129,
			"label": "Max HP",
			"range": [
				107,
				152
			],
			"tolerance": "inside MME's min to max range",
			"touchstone": 130.0
		},
		{
			"cause": "no-picklocks-skill",
			"differs": true,
			"field": "picklocks",
			"game": 0,
			"label": "Picklocks",
			"tolerance": "exact (whole numbers)",
			"touchstone": 65.0
		},
		{
			"differs": false,
			"field": "encumbrance_pct",
			"game": 30,
			"label": "Encumbrance %",
			"tolerance": "exact (whole numbers)",
			"touchstone": 30.0
		}
	]
} as unknown as RealmDetail;

const row = (id: number, first: string, race: number, race_name: string, cls: number, class_name: string, level: number, gang = ''): RealmSummary => ({
	id,
	first_name: first,
	last_name: 'Fixture',
	race,
	race_name,
	class: cls,
	class_name,
	level,
	exp: level * 1000,
	gang,
	map: 1,
	room: 1000 + id,
	last_on: 1760000000 + id,
	last_on_iso: '2025-10-09T09:53:20Z',
	lives: 9,
	differs: id % 3
});

export const roster: RealmSummary[] = [
	row(1, 'Aldric', 1, 'Human', 1, 'Warrior', 1, 'Ironwood'),
	row(2, 'Brenna', 5, 'Elf', 8, 'Thief', 9),
	row(3, 'Cormac', 2, 'Dwarf', 5, 'Priest', 16, 'Ironwood'),
	row(4, 'Dara', 1, 'Human', 12, 'Mage', 30)
];
