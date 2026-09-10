# Graph Report - terminal  (2026-09-10)

## Corpus Check
- 45 files · ~13,162 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 319 nodes · 442 edges · 26 communities (10 shown, 8 thin omitted)
- Extraction: 98% EXTRACTED · 2% INFERRED · 0% AMBIGUOUS · INFERRED: 7 edges (avg confidence: 0.88)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `1d0a452c`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- TerminalDimensions
- Terminal State Integration Tests
- Architecture and Compatibility
- SemanticPerformer
- tests/parser.rs
- Terminal and Input Modes
- Terminal State Semantics
- ScreenGrid
- CI and Engineering Process
- Product Scope
- Repository Source of Truth
- Application Package
- Configuration Package
- Terminal Core Package
- Platform Package
- PTY Package
- Renderer Package
- Workspace Package

## God Nodes (most connected - your core abstractions)
1. `TerminalState` - 34 edges
2. `ScreenGrid` - 21 edges
3. `TerminalDimensions` - 17 edges
4. `Cell` - 12 edges
5. `TerminalModes` - 12 edges
6. `Cursor` - 10 edges
7. `TerminalParserError` - 9 edges
8. `SemanticPerformer` - 9 edges
9. `Milestone Roadmap` - 9 edges
10. `CellAttributes` - 8 edges

## Surprising Connections (you probably didn't know these)
- `Terminal Product Priorities` --semantically_similar_to--> `Keyboard-first Terminal and Workspace Product`  [INFERRED] [semantically similar]
  README.md → docs/PRODUCT.md
- `M2 Typed Cursor and Erase Slice` --semantically_similar_to--> `Deferred Terminal Compatibility Semantics`  [INFERRED] [semantically similar]
  .agent/TASKS.md → docs/CURRENT_STATE.md
- `Selective Graphify Usage` --conceptually_related_to--> `Terminal Application Architecture`  [INFERRED]
  .agent/AGENT.md → docs/ARCHITECTURE.md
- `Terminal Workspace Application` --references--> `Milestone Roadmap`  [EXTRACTED]
  README.md → docs/ROADMAP.md
- `Six-runner Remote CI Gate` --references--> `Six Native CI Runners`  [EXTRACTED]
  .agent/SESSION.md → .github/workflows/ci.yml

## Import Cycles
- None detected.

## Hyperedges (group relationships)
- **Centralized Control Flow** — docs_architecture_centralized_commands, docs_decisions_adr_0006_config_commands_workspaces_config_commands_workspaces, docs_roadmap_m5_input_configuration [EXTRACTED 1.00]
- **Terminal Core Semantic Boundary** — docs_architecture_terminalparser_boundary, docs_architecture_terminalstate_boundary, docs_decisions_adr_0004_vte_custom_terminal_state_vte_custom_state [EXTRACTED 1.00]
- **Cross-platform Release Matrix** — _github_workflows_ci_six_native_runners, docs_decisions_adr_0008_license_naming_target_architectures_license_naming_targets, docs_roadmap_m6_workspaces_release [INFERRED 0.85]

## Communities (26 total, 8 thin omitted)

### Community 0 - "TerminalDimensions"
Cohesion: 0.07
Nodes (18): Cursor, CursorError, offset_clamped(), Display, Error, Formatter, Result, Self (+10 more)

### Community 1 - "Terminal State Integration Tests"
Cohesion: 0.05
Nodes (6): assert_default_cell(), erase_in_display_supports_all_directions_without_moving_the_cursor(), erase_in_line_supports_all_directions_without_moving_the_cursor(), filled_state(), row_text(), String

### Community 2 - "Architecture and Compatibility"
Cohesion: 0.08
Nodes (33): Engineering Agent Contract, Non-negotiable Architecture Boundaries, Selective Graphify Usage, M1 Terminal Core Session Handoff, Engineering Backlog, M2 Typed Cursor and Erase Slice, Terminal Application Architecture, Centralized Command System (+25 more)

### Community 3 - "SemanticPerformer"
Cohesion: 0.08
Nodes (20): default_one(), Default, Display, Error, Formatter, Option, Result, Self (+12 more)

### Community 4 - "tests/parser.rs"
Cohesion: 0.07
Nodes (10): assert_observable_state_eq(), byte_at_a_time_matches_one_shot_input(), every_split_point_matches_one_shot_input(), many_chunk_sizes_match_one_shot_input(), parse_in_chunks(), parser_mode_dispatch_is_chunk_safe_at_every_byte_boundary(), row_text(), String (+2 more)

### Community 5 - "Terminal and Input Modes"
Cohesion: 0.15
Nodes (6): AutoWrapMode, CharacterInsertionMode, CursorKeyMode, CursorVisibility, InputModes, TerminalModes

### Community 6 - "Terminal State Semantics"
Cohesion: 0.20
Nodes (9): clear_screen_clears_owned_screen_content(), CursorMovement, EraseDirection, EraseRegion, reset_clears_existing_screen_content(), resize_preserves_overlap_and_blanks_new_cells_through_facade(), Result, Self (+1 more)

### Community 8 - "ScreenGrid"
Cohesion: 0.12
Nodes (9): Cell, CellAttributes, CellColor, Default, Self, Option, ScreenGrid, Parser (+1 more)

### Community 10 - "CI and Engineering Process"
Cohesion: 0.22
Nodes (9): Six-runner Remote CI Gate, Engineering Completion Gates, Engineering Workflow, Cross-platform GitHub Actions CI, Six Native CI Runners, Engineering Conventions, ADR-0001: Rust and Cargo Workspace, ADR-0008: License, Naming, and Target Architectures (+1 more)

### Community 11 - "Product Scope"
Cohesion: 0.40
Nodes (5): ADR-0007: V1 Scope Boundaries, Keyboard-first Terminal and Workspace Product, V1 Non-goals, V1 Product Scope, Terminal Product Priorities

## Knowledge Gaps
- **19 isolated node(s):** `terminal-app`, `terminal-config`, `terminal-platform`, `terminal-renderer`, `terminal-core` (+14 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 157 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **8 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `TerminalState` connect `Terminal State Semantics` to `Terminal State Integration Tests`, `SemanticPerformer`, `tests/parser.rs`, `Terminal and Input Modes`, `ScreenGrid`?**
  _High betweenness centrality (0.382) - this node is a cross-community bridge._
- **Why does `ScreenGrid` connect `ScreenGrid` to `TerminalDimensions`, `Terminal State Semantics`?**
  _High betweenness centrality (0.144) - this node is a cross-community bridge._
- **Why does `TerminalDimensions` connect `TerminalDimensions` to `ScreenGrid`, `Terminal State Semantics`?**
  _High betweenness centrality (0.067) - this node is a cross-community bridge._
- **What connects `terminal-app`, `terminal-config`, `terminal-platform` to the rest of the system?**
  _19 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `TerminalDimensions` be split into smaller, more focused modules?**
  _Cohesion score 0.06606606606606606 - nodes in this community are weakly interconnected._
- **Should `Terminal State Integration Tests` be split into smaller, more focused modules?**
  _Cohesion score 0.05094130675526024 - nodes in this community are weakly interconnected._
- **Should `Architecture and Compatibility` be split into smaller, more focused modules?**
  _Cohesion score 0.08333333333333333 - nodes in this community are weakly interconnected._