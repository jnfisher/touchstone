# Status

Kept by the orchestrator only.

| Card | Title | Model | Depends | State |
|---|---|---|---|---|
| T-001 | Scaffold the workspace, web app and CI | sonnet | | done |
| T-002 | Fetch tools and the MME procedure index | sonnet | | done |
| T-003 | Inventory MME's features | sonnet | T-002 | done |
| T-010 | World model types | sonnet | T-001, T-002 | done |
| T-011 | `.mdb` loader and data fixtures | sonnet | T-010 | done |
| T-012 | Live silvermere/Quicksilver adapter | sonnet | T-010 | done |
| T-013 | Serve the World with live reload | sonnet | T-011, T-012 | in progress |
| T-020 | VB6 semantics and touchstone-vb6 | sonnet | T-001 | done |
| T-021 | Oracle interpreter | sonnet | T-002, T-020 | in progress |
| T-022 | Oracle shims and first fixtures | sonnet | T-021, T-011 | todo |
| T-014 | Derived (NMR) fields for live sources | sonnet | T-015 | todo |
| T-015 | Stock parity check, .mdb vs silvermere | sonnet | T-013 | todo |
| P4-00 | Explorers with raw columns | sonnet | T-013 | todo |
| P3-13 | Monster attack simulator | sonnet | T-022 | todo |
| P3-01 | Exp tables and training cost | sonnet | T-022 | todo |
| P3-20 | Exp/hr helpers and Model D | sonnet | T-022 | todo |
| T-050 | Release and Pi packaging | sonnet | T-013 | todo |

## Backlog

The phase 3 (P3-01 to P3-43, formulas) and phase 4 (P4-01 to P4-30, web UI) cards are listed in
`docs/inventory.md` section 8 with their procedures and dependencies. The orchestrator writes each
as a `tasks/` card file when its dependencies are close to merged, so cards reflect what earlier
cards actually built. Optional cards O-01 to O-07 are decided at the end of phase 4.
