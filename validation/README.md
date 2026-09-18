# Optional external diagnostics

These Python helpers are development/debug inputs, not part of the Rust runtime. No Python environment or installed packages are shipped. No diagnostics were executed during final closeout.

- `check_decoupled.py` / `decoupled-results.txt`: shadow checks completed on llvmpipe before the owner stopped testing. Not a native build or target-GPU benchmark.
- `check_glass_transport.py` / `glass-transport-results.txt`: retained **failing** glass fixture. The first expected transmitted-water assertion failed; subsequent assertions did not complete. Fixture versus production cause is unresolved.
- `native-glass-results.txt`: earlier native attempt could not run because cargo was unavailable.
- `history/pre-decoupled/`: older logs, not acceptance evidence for the current shaders.
- Other `check_*.py` scripts: retained historical utilities; may assume old buffers, optional passes or removed multi-ray functions. Review/port before use.

Rust test sources are in `../tests/`. The batch rows live in [../docs/ledger.md](../docs/ledger.md); the menu's current behavior in [../docs/menu.md](../docs/menu.md). The original handoff notes (BATCH-LOG, HANDOFF) no longer exist in this tree.
