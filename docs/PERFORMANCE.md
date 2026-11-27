# Performance

File parsing and automatic analysis run off the desktop UI thread, as do byte searches and ASCII scans. Large listing views render the visible rows. Automatic disassembly uses bounded function and instruction budgets. The graph layout currently renders at most 200 nodes and register output is cached by function in the active session.

The right inspector queries discovered references from the current analysis. An indexed implementation and large executable benchmarks remain open. Baseline benchmark work should measure first-open time, peak memory, search throughput, and responsiveness under slow load with an independently reproducible input.
