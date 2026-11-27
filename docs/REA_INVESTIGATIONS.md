# Research workflow

Hexaris uses REA and Ghidra as research and differential validation tools. Register REA through its documented MCP setup and configure the Ghidra provider with the path to a verified distribution. Confirm availability with a real tool call on an owned fixture. Record tool calls and keep full evidence and local observations in the ignored research directory. [REA documentation](https://github.com/morluto/rea) · [Ghidra provider](https://github.com/morluto/rea/blob/main/docs/providers/ghidra-provider.md)

The source fixture under `tests/fixtures` and the available probe provide an independently reproducible comparison. [Reproduction commands](../tests/fixtures/README.md)
