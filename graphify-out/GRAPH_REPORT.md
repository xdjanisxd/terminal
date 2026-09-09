# Graph Report - terminal  (2026-09-09)

## Corpus Check
- Corpus is ~11,497 words - fits in a single context window. You may not need a graph.

## Summary
- 288 nodes · 386 edges · 26 communities (9 shown, 9 thin omitted)
- Extraction: 98% EXTRACTED · 2% INFERRED · 0% AMBIGUOUS · INFERRED: 7 edges (avg confidence: 0.88)
- Token cost: 0 input · 0 output

## Community Hubs (Navigation)
- Cursor and Grid Bounds
- Terminal State Integration Tests
- Architecture and Compatibility
- Parser Adapter Implementation
- Parser Chunking Tests
- Terminal and Input Modes
- Terminal State Semantics
- Cell and Attributes
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
1. `TerminalState` - 31 edges
2. `ScreenGrid` - 20 edges
3. `TerminalDimensions` - 17 edges
4. `Cell` - 12 edges
5. `TerminalModes` - 12 edges
6. `Cursor` - 10 edges
7. `TerminalParserError` - 9 edges
8. `Milestone Roadmap` - 9 edges
9. `CellAttributes` - 8 edges
10. `SemanticPerformer` - 8 edges

## Surprising Connections (you probably didn't know these)
- `M2 Typed Cursor and Erase Slice` --semantically_similar_to--> `Deferred Terminal Compatibility Semantics`  [INFERRED] [semantically similar]
  .agent/TASKS.md → docs/CURRENT_STATE.md
- `Terminal Product Priorities` --semantically_similar_to--> `Keyboard-first Terminal and Workspace Product`  [INFERRED] [semantically similar]
  README.md → docs/PRODUCT.md
- `Selective Graphify Usage` --conceptually_related_to--> `Terminal Application Architecture`  [INFERRED]
  .agent/AGENT.md → docs/ARCHITECTURE.md
- `Terminal Workspace Application` --references--> `Milestone Roadmap`  [EXTRACTED]
  README.md → docs/ROADMAP.md
- `Engineering Agent Contract` --references--> `Current Implementation State`  [EXTRACTED]
  .agent/AGENT.md → docs/CURRENT_STATE.md

## Import Cycles
- None detected.

## Hyperedges (group relationships)
- **Terminal Core Semantic Boundary** — docs_architecture_terminalparser_boundary, docs_architecture_terminalstate_boundary, docs_decisions_adr_0004_vte_custom_terminal_state_vte_custom_state [EXTRACTED 1.00]
- **Cross-platform Release Matrix** — _github_workflows_ci_six_native_runners, docs_decisions_adr_0008_license_naming_target_architectures_license_naming_targets, docs_roadmap_m6_workspaces_release [INFERRED 0.85]
- **Centralized Control Flow** — docs_architecture_centralized_commands, docs_decisions_adr_0006_config_commands_workspaces_config_commands_workspaces, docs_roadmap_m5_input_configuration [EXTRACTED 1.00]

## Communities (26 total, 9 thin omitted)

### Community 0 - "Cursor and Grid Bounds"
Cohesion: 0.06
Nodes (21): Cursor, CursorError, offset_clamped(), Display, Error, Formatter, Result, Self (+13 more)

### Community 2 - "Architecture and Compatibility"
Cohesion: 0.08
Nodes (33): Engineering Agent Contract, Non-negotiable Architecture Boundaries, Selective Graphify Usage, M1 Terminal Core Session Handoff, Engineering Backlog, M2 Typed Cursor and Erase Slice, Terminal Application Architecture, Centralized Command System (+25 more)

### Community 3 - "Parser Adapter Implementation"
Cohesion: 0.09
Nodes (17): Default, Display, Error, Formatter, Option, Result, Self, SemanticPerformer (+9 more)

### Community 4 - "Parser Chunking Tests"
Cohesion: 0.10
Nodes (7): assert_observable_state_eq(), byte_at_a_time_matches_one_shot_input(), every_split_point_matches_one_shot_input(), many_chunk_sizes_match_one_shot_input(), parse_in_chunks(), row_text(), String

### Community 5 - "Terminal and Input Modes"
Cohesion: 0.15
Nodes (6): AutoWrapMode, CharacterInsertionMode, CursorKeyMode, CursorVisibility, InputModes, TerminalModes

### Community 6 - "Terminal State Semantics"
Cohesion: 0.22
Nodes (6): clear_screen_clears_owned_screen_content(), reset_clears_existing_screen_content(), resize_preserves_overlap_and_blanks_new_cells_through_facade(), Result, Self, TerminalState

### Community 8 - "Cell and Attributes"
Cohesion: 0.20
Nodes (6): Cell, CellAttributes, CellColor, Default, Self, Option

### Community 10 - "CI and Engineering Process"
Cohesion: 0.22
Nodes (9): Six-runner Remote CI Gate, Engineering Completion Gates, Engineering Workflow, Cross-platform GitHub Actions CI, Six Native CI Runners, Engineering Conventions, ADR-0001: Rust and Cargo Workspace, ADR-0008: License, Naming, and Target Architectures (+1 more)

### Community 11 - "Product Scope"
Cohesion: 0.40
Nodes (5): ADR-0007: V1 Scope Boundaries, Keyboard-first Terminal and Workspace Product, V1 Non-goals, V1 Product Scope, Terminal Product Priorities

## Knowledge Gaps
- **20 isolated node(s):** `terminal-app`, `terminal-config`, `terminal-platform`, `terminal-renderer`, `terminal-core` (+15 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 143 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **9 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `TerminalState` connect `Terminal State Semantics` to `Cursor and Grid Bounds`, `Terminal State Integration Tests`, `Parser Adapter Implementation`, `Parser Chunking Tests`, `Terminal and Input Modes`?**
  _High betweenness centrality (0.342) - this node is a cross-community bridge._
- **Why does `ScreenGrid` connect `Cursor and Grid Bounds` to `Cell and Attributes`, `Terminal State Semantics`?**
  _High betweenness centrality (0.144) - this node is a cross-community bridge._
- **Why does `row_text()` connect `Terminal State Integration Tests` to `Terminal State Semantics`?**
  _High betweenness centrality (0.133) - this node is a cross-community bridge._
- **What connects `terminal-app`, `terminal-config`, `terminal-platform` to the rest of the system?**
  _20 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Cursor and Grid Bounds` be split into smaller, more focused modules?**
  _Cohesion score 0.05893719806763285 - nodes in this community are weakly interconnected._
- **Should `Terminal State Integration Tests` be split into smaller, more focused modules?**
  _Cohesion score 0.05714285714285714 - nodes in this community are weakly interconnected._
- **Should `Architecture and Compatibility` be split into smaller, more focused modules?**
  _Cohesion score 0.08333333333333333 - nodes in this community are weakly interconnected._