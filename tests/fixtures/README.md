# Windows fixture

Run from a 64-bit Visual Studio Developer Command Prompt with MSVC and a Windows SDK.

```bat
mkdir .dev\fixtures
cl /nologo /W4 /WX /Od /GS- /TC tests\fixtures\score.c /Fo.dev\fixtures\score.obj /link /nologo /nodefaultlib /entry:entry /subsystem:console /map:.dev\fixtures\score.map /out:.dev\fixtures\score.exe
```

The source returns `value * 2` for unsigned values below 10 and `value + 3` otherwise. `entry` calls `score` with 12. Use the link map to locate the function for comparison. Compiler details can change code and addresses.

For the Ghidra comparison, create an empty local project parent. Pass the fixture and its `score` virtual address from the link map.

```bat
analyzeHeadless PATH_TO_PROJECT_PARENT HexarisFixture -import .dev\fixtures\score.exe -deleteProject -scriptPath tests\fixtures -postScript HexarisProbe.java 140001000
```

The probe emits the decoded instructions, basic blocks and edges, references to `score`, and decompiled output for the source in this directory. Keep generated images and research outputs local.

From a Visual Studio developer shell, `pwsh -File scripts/verify-windows-fixture.ps1` compiles this fixture, validates key CLI operations, recompiles Hexaris's C-like result, and compares it with the original function over boundary cases and a deterministic input sequence.

`unwind.c` tests an uncalled function discoverable through the PE exception directory and a separate named executable export. Build and verify it with `pwsh -File scripts/verify-workstation.ps1`. Pass `-GhidraXml PATH` to check a real Ghidra metadata merge. The following headless command creates that export from the original score fixture.

```bat
analyzeHeadless PATH_TO_PROJECT_PARENT HexarisXmlFixture -import .dev\fixtures\score.exe -deleteProject -scriptPath tests\fixtures -postScript ExportFixtureXml.java 140001000 PATH_TO_OUTPUT_XML
```

Use a local scratch path for XML and its accompanying `.bytes` file. The import check reads only the XML.

`workbench.c` is the screenshot fixture. Build it with the same compiler options as `score.c`, using `workbench.c`, `workbench.obj`, `workbench.map`, and `workbench.exe` for the input, object, map, and image names. Open `score_window` to see its branching logic and read its callers and callees.
