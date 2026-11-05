# Porting a formula family (phase 3 cards)

Every phase 3 card follows the same loop. The card names the family, the MME procedures and any
decisions; this page is the rest.

1. **Read** the procedures the card lists through the index
   (`~/.cache/touchstone/mme-index/<sha>/<module>/<proc>.vb`), plus `docs/inventory.md` section 4
   and 4a for the family's inputs. Use `mme_index.py --deps NAME` for callees. Never open whole forms.
2. **Scenarios.** Add `fixtures/scenarios/<family>.json`: inputs that cover every branch of the stock
   path (grids over levels, stats, classes, races, record numbers; edge values such as 0, caps,
   negative modifiers). Add shims to the oracle only for the inputs this family reads.
3. **Fixtures.** `python3 -I tools/oracle/oracle.py gen <family>` writes `fixtures/rules/<family>.json`.
   Commit it. It must be byte-identical on a re-run.
4. **Port** into `crates/touchstone-rules/src/<family>.rs`: plain functions over model types and a
   small input struct for character/global state, using `touchstone-vb6` for every VB6 conversion,
   integer division, rounding and Currency value. Keep MME's order of operations; do not simplify
   arithmetic. Each public function's doc comment names the MME procedure it ports. GreaterMUD and
   Paramud branches are not ported: match on `Profile` and return `Unsupported` for them.
5. **Test** in `crates/touchstone-rules/tests/all/<family>.rs`: load the fixture, run every case, report
   the first mismatches with inputs. All cases must pass. A case you cannot make pass after a real
   root-cause effort goes in `fixtures/known_gaps.json` with an explanation; say so in your report.
6. **Wire** nothing into the server or UI; phase 4 cards do that.

Done when: workspace checks pass, `oracle.py check` reports the family's fixture current, and the
family's test runs every fixture case.
