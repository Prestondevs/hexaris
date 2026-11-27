# Debugger status

Hexaris currently performs static inspection only. Native debugger backends and the debugger interface are open. An implementation must own launched fixture processes, synchronize stop and exit events with the UI, and verify breakpoint restoration, detach, process cleanup and error behavior on each supported platform.

The public reference documents a debugger workflow. Hexaris does not currently have a verified equivalent. [Feature matrix](FEATURE_MATRIX.md)
