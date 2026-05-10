documentation
=============

architecture
------------
the binary drives a single crossterm/ratatui-powered terminal experience. `main.rs` wires together the evidence store, ffi helpers, timeline renderer, and tui renderer by parsing case data, building a `EvidenceStore`, and spawning the `run_event_loop` that repeatedly redraws the murder board.

modules
-------
- `evidence_store`: owns the serialized evidence records, exposes helpers like `find_suspect_conflicts`, `timeline_gaps`, and `add_manual_evidence`, and is the single source of truth for all ui widgets.
- `parser`: takes raw phase output bytes and turns them into structured `EvidenceStore` data; errors bubble up to early-exit with diagnostics.
- `ffi`: provides `calculate_guilt_safe` and `find_contradictions_safe` wrappers that can be reused anywhere Rust needs quick probability estimates without panics.
- `timeline_render` / `timeline_scrubber`: render timeline snapshots and respond to mouse clicks; the scrubber translates mouse offsets into `TimelineEvent`s.
- `tui`: defines `AppState`, input handling, view layout, and `render_murder_board`. `map_key_event` lives here and maps raw `crossterm::event::KeyCode` into `InputEvent`s.
- `witness_audit`, `motive`, `case_linker`, and `feasibility`: encapsulate domain logic used by the analysis pane and the motive matrix cache; they stay pure rust and expose APIs consumed by the `tui` module.

dataflow
--------
1. entry point calls `parse_case_source` to resolve command-line switches into a `CaseSource` enum (zodiac, beale, or a filepath).
2. `load_case_data` and `parse_phase_output` feed into `EvidenceStore::from_phase_output`.
3. `print_snapshot` uses `ffi` helpers to log guilt/contradiction summaries before switching into tui mode.
4. `run_event_loop` redraws every 100 ms, collects key/mouse events, and updates state via `update_state`.
5. manual inputs write new records via `EvidenceStore::add_manual_evidence`, and `reset_store_for_case` swaps preset data while resetting UI state each time the case modal closes.

runtime
-------
- enable raw mode and mouse capture via `crossterm::terminal::enable_raw_mode` / `execute!` when the tui launches; disable them before exit.
- `Terminal<CrosstermBackend<_>>` holds the drawing context and is passed to `render_murder_board` on every loop iteration.
- the `TimelineView` struct mirrors relevant fields from `AppState` (`zoom_level`, `scroll_position`) and caches snapshots rendered with `timeline_render::render_timeline` for persistence.

building & testing
------------------
- `cargo build --bin sleepy-dogs-analyzer` for local artifacts or `cargo run --bin sleepy-dogs-analyzer` for the full experience.
- `cargo test` runs unit/feature tests; add `--package` or `--lib` filters if needed.
- formatting and linting: `cargo fmt` and `cargo clippy`.

customization
-------------
- switch presets with `--case=zodiac|beale` or supply `--case=/path/to/phase-output.json`.
- augment `test_data` or add new files to `test_data.rs` if you need fresh fixtures.
- plug in a new evidence source by replacing `load_case_data` with the loader for your local database or API (just make sure to keep returning a `Vec<u8>` for parsing).

ffi notes
---------
- `calculate_guilt_safe` takes a reference to `EvidenceStore` and returns an `f32` guilt score; it uses the embedded `case_linker` logic under the hood.
- `find_contradictions_safe` also inspects the store and produces the first contradiction payload it can find; handle the `Option` safely and log it back to `print_snapshot` so diagnostics stay visible even outside the tui.

observability
-------------
- logs are printed directly to stdout/stderr before the tui takes over. capture them by redirecting `cargo run` output to a file: `cargo run ... > logs.txt`.
- there are no background services; everything runs in-process, so panic-proofing is just a matter of handling errors from `parse_phase_output`, file IO, and terminal setup gracefully (see `run_app` error paths).
