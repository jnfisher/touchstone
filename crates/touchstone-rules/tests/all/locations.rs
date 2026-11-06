// Fixture tests for the locations family: see docs/porting.md step 5.
//
// Expected values: fixtures/rules/locations.json, produced by the oracle running MME's own
// GetLocations (with AddRoomCmdLocationRow and the name lookups) against the stock database. The
// lookups are answered from the rows under header "data" (the records the cases can reach); GetLairInfo
// from "data"."lairs", as the oracle's lair shim does. ListView sorting is not run: the oracle records
// each SortListView request and the port reports the same row counts.
use super::fixture::load;
use serde_json::Value;
use std::collections::HashMap;
use touchstone_model::{AbilitySlot, Item, Shop, ShopSlot};
use touchstone_rules::lairs::LairInfo;
use touchstone_rules::locations::*;
use touchstone_vb6::Currency;

fn int(v: &Value) -> i32 {
    v.as_i64().unwrap_or_else(|| panic!("not an integer: {v}")) as i32
}

struct Rows {
    items: HashMap<i32, Item>,
    shops: HashMap<i32, Shop>,
    monsters: HashMap<i32, String>,
    spells: HashMap<i32, String>,
    rooms: HashMap<(i32, i32), (String, i32)>,
}

struct Lairs(HashMap<String, Value>);

impl LocationData for Rows {
    fn item(&self, n: i32) -> Option<&Item> {
        self.items.get(&n)
    }
    fn shop(&self, n: i32) -> Option<&Shop> {
        self.shops.get(&n)
    }
    fn monster_name(&self, n: i32) -> Option<String> {
        self.monsters.get(&n).cloned()
    }
    fn spell_name(&self, n: i32) -> Option<String> {
        self.spells.get(&n).cloned()
    }
    fn room(&self, map: i32, room: i32) -> Option<(String, i32)> {
        self.rooms.get(&(map, room)).cloned()
    }
}

impl LocationLairs for Lairs {
    fn get_lair_info(&mut self, group_index: &str, max_regen: i16) -> LairInfo {
        let mut u = LairInfo::default();
        let Some(row) = self.0.get(group_index) else {
            return u;
        };
        let cur = |k: &str| {
            row.get(k).map_or(Currency::from_scaled(0), |v| {
                Currency::from_f64(v.as_f64().unwrap()).unwrap()
            })
        };
        u.group_index = group_index.to_string();
        u.mobs = cur("nMobs");
        u.avg_exp = cur("nAvgExp");
        u.avg_dmg_clear_round = cur("nAvgDmgClearRound");
        u.damage_out = row
            .get("nDamageOut")
            .map_or(0, |v| v.as_i64().unwrap() as i32);
        u.max_regen = Currency::from_scaled(i64::from(max_regen) * 10_000);
        u
    }
}

fn item(r: &Value) -> Item {
    Item {
        number: int(&r[0]),
        name: r[1].as_str().unwrap().to_string(),
        limit: 0,
        encum: 0,
        item_type: int(&r[2]),
        use_count: 0,
        price: int(&r[3]),
        currency: int(&r[4]),
        min: 0,
        max: 0,
        armour_class: 0,
        damage_resist: 0,
        weapon_type: 0,
        armour_type: 0,
        worn: 0,
        accy: 0,
        gettable: false,
        str_req: 0,
        speed: 0,
        not_droppable: false,
        destroy_on_death: false,
        retain_after_uses: false,
        class_rest: [0; 10],
        race_rest: [0; 10],
        negate_spell: [0; 10],
        abilities: [AbilitySlot::default(); 20],
        in_game: false,
        obtained_from: Some(r[5].as_str().unwrap().to_string()),
        references: None,
    }
}

fn shop(r: &Value) -> Shop {
    let mut stock = [ShopSlot::default(); 20];
    for (slot, s) in stock.iter_mut().zip(r[4].as_array().unwrap()) {
        *slot = ShopSlot {
            item: int(&s[0]),
            max: int(&s[1]),
            time: int(&s[2]),
            amount: int(&s[3]),
            pct: int(&s[4]),
        };
    }
    Shop {
        number: int(&r[0]),
        name: String::new(),
        shop_type: int(&r[1]),
        min_lvl: 0,
        max_lvl: 0,
        markup_pct: int(&r[2]),
        class_rest: 0,
        stock,
        in_game: false,
        assigned_to: Some(r[3].as_str().unwrap().to_string()),
    }
}

fn rows(data: &Value) -> Rows {
    let list = |k: &str| data[k].as_array().unwrap().clone();
    Rows {
        items: list("items")
            .iter()
            .map(|r| (int(&r[0]), item(r)))
            .collect(),
        shops: list("shops")
            .iter()
            .map(|r| (int(&r[0]), shop(r)))
            .collect(),
        monsters: list("monsters")
            .iter()
            .map(|r| (int(&r[0]), r[1].as_str().unwrap().to_string()))
            .collect(),
        spells: list("spells")
            .iter()
            .map(|r| (int(&r[0]), r[1].as_str().unwrap().to_string()))
            .collect(),
        rooms: list("rooms")
            .iter()
            .map(|r| {
                (
                    (int(&r[0]), int(&r[1])),
                    (r[2].as_str().unwrap().to_string(), int(&r[3])),
                )
            })
            .collect(),
    }
}

fn lair_table(data: &Value) -> Lairs {
    Lairs(
        data["lairs"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
    )
}

/// A row as the oracle writes it: [Text, Tag, cells], a cell its Text or [Text, Tag, ForeColor].
fn row_json(r: &LocationRow) -> Value {
    let cells: Vec<Value> = r
        .cells
        .iter()
        .map(|c| match &c.tag {
            None => Value::from(c.text.clone()),
            Some(t) => serde_json::json!([c.text, t, null]),
        })
        .collect();
    serde_json::json!([r.text, r.tag, cells])
}

#[test]
fn locations_match_the_oracle() {
    let fx = load("locations");
    let setup = &fx.header["setup"];
    let data = &fx.header["data"];
    let db = rows(data);
    let mut lairs = lair_table(data);
    let mut bad = Vec::new();
    for case in &fx.cases {
        let id = case["id"].as_str().unwrap();
        let inputs = &case["inputs"];
        if let Some(proc) = inputs["proc"].as_str() {
            assert_eq!(proc, "ExtractRoomActions");
            let got = extract_room_actions(inputs["args"][0].as_str().unwrap());
            if case["outputs"]["result"].as_str() != Some(got.as_str()) {
                bad.push(format!(
                    "{id}: got {got:?}, want {}",
                    case["outputs"]["result"]
                ));
            }
            continue;
        }
        let glob = |k: &str| {
            inputs["globals"]
                .get(k)
                .or_else(|| setup["globals"].get(k))
                .unwrap_or_else(|| panic!("no global {k}"))
        };
        let ctl = |name: &str| {
            inputs["forms"]["frmMain"]
                .get(name)
                .or_else(|| setup["forms"]["frmMain"].get(name))
                .unwrap_or_else(|| panic!("no control {name}"))
        };
        let opt = LocationOptions {
            nmr_ver: glob("nNMRVer").as_f64().unwrap(),
            hide_record_numbers: glob("bHideRecordNumbers").as_bool().unwrap(),
            global_filter: ctl("chkGlobalFilter")["Value"].as_i64() == Some(1),
            charm_tag: ctl("txtCharStats")["$array"]["5"]["Tag"]
                .as_str()
                .unwrap()
                .to_string(),
            attack_type: int(glob("nGlobalAttackTypeMME")),
        };
        let mut out = LocationList::default();
        // rows the case preloaded into the list (only their count matters to the port)
        if let Some(pre) = inputs["forms"]["frmMain"]["lvLoc"]["$recordlv"].as_array() {
            for p in pre {
                out.rows.push(LocationRow {
                    kind: LocationKind::Item,
                    text: p["Text"].as_str().unwrap().to_string(),
                    tag: p["Tag"].as_str().map(str::to_string),
                    cells: Vec::new(),
                });
            }
        }
        let mut all_sorts = Vec::new();
        for st in inputs["steps"].as_array().unwrap() {
            let a = st["args"].as_array().unwrap();
            let args = LocationArgs {
                loc: a[0].as_str().unwrap(),
                dont_clear: a[2].as_bool().unwrap(),
                header: a[3].as_str().unwrap(),
                aux_value: int(&a[4]),
                two_columns: a[5].as_bool().unwrap(),
                dont_sort: a[6].as_bool().unwrap(),
                percent_column: a[7].as_bool().unwrap(),
                footer: a[8].as_str().unwrap(),
                limit: int(&a[9]) as i16,
            };
            get_locations(&db, &mut lairs, &opt, &args, &mut out);
            all_sorts.extend(std::mem::take(&mut out.sorts));
        }
        let got_rows: Vec<Value> = out.rows.iter().map(row_json).collect();
        let want_rows = case["outputs"]["rows"].as_array().unwrap();
        let want_sorts: Vec<usize> = case["events"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|e| e["kind"] == "sort_listview")
            .map(|e| e["rows"].as_u64().unwrap() as usize)
            .collect();
        let want_errors: Vec<i32> = case["events"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|e| e["kind"] == "handled_error")
            .filter(|e| {
                matches!(
                    e["source"].as_str(),
                    Some("" | "GetLocations" | "AddRoomCmdLocationRow")
                )
            })
            .map(|e| int(&e["number"]))
            .collect();
        if got_rows != *want_rows || all_sorts != want_sorts || out.errors != want_errors {
            let first = got_rows
                .iter()
                .zip(want_rows)
                .position(|(g, w)| g != w)
                .map(|i| format!("row {i}: got {} want {}", got_rows[i], want_rows[i]));
            bad.push(format!(
                "{id}: {} rows (want {}), sorts {all_sorts:?} (want {want_sorts:?}), errors {:?} (want {want_errors:?}) {}",
                got_rows.len(),
                want_rows.len(),
                out.errors,
                first.unwrap_or_default()
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "{} of {} cases differ; first:\n{}",
        bad.len(),
        fx.cases.len(),
        bad.iter().take(12).cloned().collect::<Vec<_>>().join("\n")
    );
}
