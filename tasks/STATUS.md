# Status

Kept by the orchestrator only.

| Card | Title | Model | Depends | State |
|---|---|---|---|---|
| T-001 | Scaffold the workspace, web app and CI | sonnet | | done |
| T-002 | Fetch tools and the MME procedure index | sonnet | | done |
| T-003 | Inventory MME's features | sonnet | T-002 | done |
| T-010 | World model types | sonnet | T-001, T-002 | in progress |
| T-011 | `.mdb` loader and data fixtures | sonnet | T-010 | todo |
| T-012 | Live silvermere/Quicksilver adapter | sonnet | T-010 | todo |
| T-013 | Serve the World with live reload | sonnet | T-011, T-012 | todo |
| T-020 | VB6 semantics and touchstone-vb6 | sonnet | T-001 | in progress |
| T-021 | Oracle interpreter | sonnet | T-002, T-020 | todo |
| T-022 | Oracle shims and first fixtures | sonnet | T-021, T-011 | todo |
| T-050 | Release and Pi packaging | sonnet | T-013 | todo |

## Backlog

The phase 3 (P3-01 to P3-43, formulas) and phase 4 (P4-01 to P4-30, web UI) cards are listed in
`docs/inventory.md` section 8 with their procedures and dependencies. The orchestrator writes each
as a `tasks/` card file when its dependencies are close to merged, so cards reflect what earlier
cards actually built. Optional cards O-01 to O-07 are decided at the end of phase 4.
