sleepy dogs analyzer
====================

idea
----
a moody, terminal-first investigative dashboard that lets you throw evidence at suspects, spin up custom cases, and watch the “murder board” reshape itself while you zoom around timelines. it blends noir storytelling vibes with serious evidence scoring so you always know which threads are heating up.

commands
--------
- `cargo run --bin sleepy-dogs-analyzer` → launches the tui; append `--case=zodiac` or `--case=beale` or `--case=/path/to/json` to switch presets.
- `cargo test` → keeps the rust modules honest, especially around `parser`, `evidence_store`, and `ffi`.
- `cargo fmt` / `cargo clippy` → keep your style tense and warnings in check.

keys
----
- `q` → quit the dashboard cleanly.
- `+` / `=` and `-` → zoom the timeline view in and out.
- `t`, `s`, `e`, `a`, `i` → jump between timeline, suspects, evidence, analysis, and insights panes.
- `n` → add a quick observation to the notes stack tied to the current zoom position.
- `m` → open the manual evidence form (tab/backtab to move fields, type to fill, enter to submit).
- `c` → open case modal, then `z`, `b`, or `f` to pick zodiac/beale/custom; number keys `1`–`9` pick suspects quickly.
- arrow keys → scroll through evidence views; `v` selects the first visible timestamp entry.

expands
-------
- live FFI scoring that prints guilt snapshots before you even open the mui.
- custom evidence entries let you type reliability percentages, timestamps, and descriptions without leaving the board.
- case swapping keeps the same layout but reloads the store, timeline view, and guilt cache in one go.

creds
-----
no external api keys required. data comes from the included `test_data` presets or whatever json you point `--case` at. if you want secure storage for real investigations, bolt on your own vault and swap in a new loader before the tui boots.
