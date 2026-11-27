# Hexaris user guide

Hexaris is a native Rust workbench for static binary analysis. The desktop and CLI share the same engine, so you can investigate a file interactively and repeat the same analysis from a terminal.

- [Run Hexaris](#run-hexaris)
- [Try the sample](#try-the-sample)
- [Explore an image](#explore-an-image)
- [Follow code and references](#follow-code-and-references)
- [Keep an investigation](#keep-an-investigation)
- [Export a byte patch](#export-a-byte-patch)
- [Work with an assistant](#work-with-an-assistant)
- [Compare two builds](#compare-two-builds)
- [Command reference](#command-reference)

## Run Hexaris

On Windows x64, download the Windows ZIP from [Releases](https://github.com/Prestondevs/hexaris/releases/latest), extract the archive, and double-click `hexaris.exe`. The same folder contains `hexaris-cli.exe`.

The guide covers the current source tree. If your downloaded build lacks the **Tools** menu or the `context`, `compare`, and `mcp` subcommands, build the current checkout. On Windows, use a Visual Studio Developer shell with MSVC and the Windows SDK.

```powershell
cargo build --workspace --release --locked
.\target\release\hexaris.exe
.\target\release\hexaris-cli.exe --help
```

Windows x64 is the validated desktop platform. See the [README build instructions](../README.md#build-from-source) for platform dependencies.

![Welcome screen with Choose file and Open raw image actions](images/welcome.png)

## Try the sample

From a Windows x64 Visual Studio Developer PowerShell, run these commands at the repository root to compile the small [score fixture](../tests/fixtures/score.c). Replace the executable paths in later examples with the extracted release binaries if you are using a downloaded build.

```powershell
New-Item -ItemType Directory -Force .dev\fixtures | Out-Null
cl /nologo /W4 /WX /Od /GS- /TC tests\fixtures\score.c /Fo.dev\fixtures\score.obj /link /nologo /nodefaultlib /entry:entry /subsystem:console /map:.dev\fixtures\score.map /out:.dev\fixtures\score.exe
.\target\release\hexaris.exe .dev\fixtures\score.exe
```

The fixture doubles an unsigned input below 10 and adds 3 otherwise. The commands below use `0x140001000`, the address of `score` in the pictured build. Check `score.map` for the actual virtual address on your machine. Without a name override, the browser may show the function as `sub_140001000`. The [workbench fixture](../tests/fixtures/README.md) adds exported names and a larger call graph.

## Explore an image

| Task | Desktop workflow |
| --- | --- |
| Open a binary or project | Choose **Open**, drag in files, or pass file paths on the command line. Each file opens in a tab with its own view, selection, search, and navigation history. |
| Load raw bytes | Choose **File → Open raw image**, select x86 or x86-64, and enter a virtual base such as `0x1000`. Open the raw file to inspect it as mapped code and data. |
| Identify the format | Open **Overview** for architecture, endianness, entry point, image base, file size, and analysis notices. The loader reads PE, ELF, thin Mach-O, and COFF metadata and supports explicit raw x86 mappings. Code analysis requires supported, mapped x86 or x86-64 code. |
| Browse metadata | Use **Regions**, **Symbols**, **Imports**, or **Exports** in the image browser. Click an address or file offset to navigate. Availability depends on the format and metadata in the image. |
| Check a file digest | In **Overview**, use **Copy** beside SHA-256. Click the entropy chart to open the corresponding offset in **Hex**. The chart summarizes byte distribution across file ranges. |
| Read bytes | Open **Hex** for the file offset, bytes, and ASCII representation of each row. Pick a row to select its start offset, or jump to a specific offset from the entropy chart or a search result. |
| Find byte patterns | Open **Byte search** or **Find bytes**. Enter a pattern such as `83 F8 ??`, where `??` matches one byte. Pick a result to open its offset. |
| Find strings | Open **Strings**, set a minimum length, and run the scan. Select a printable ASCII result to see where it occurs in the file. |
| Change the appearance | Open **Themes** and choose Graphite, Slate, Ash Light, Foundry, or High Contrast. The menu previews each palette and Hexaris remembers the choice. |

![Overview in Ash Light with executable metadata, checksum, and file entropy](images/triage.png)

## Follow code and references

1. Select an entry in **Functions** or type a virtual address and click **Go**. Addresses accept decimal or a `0x` hexadecimal prefix. **Analyze here** explicitly decodes a function at a mapped code address.
2. Use **Disassembly** for the selected function or **Linear** to decode successive instructions from an address. Right-click a disassembly instruction to copy its address or text. **Back** and **Forward** revisit navigation history.
3. Open **Control flow** to see basic blocks and labeled branches. Click a block to jump to its first instruction. In the score fixture, follow the comparison against 10 to the two return paths.
4. Open **Calls** to see indexed direct callers and callees. The **Inspector** gives incoming and outgoing references for the selected address. Click an address to follow it.
5. Use **Register IR** to inspect projected 32-bit register values and incoming values at block boundaries. **Pseudo-C** shows the supported Windows x64 leaf-function projection and explains why a projection is unavailable when the engine cannot produce one.

![Control-flow blocks for the score fixture](images/control-flow.png)

![Direct callers, callees, and selectable call-site addresses](images/calls.png)

![Pseudo-C for the unsigned score branch](images/pseudo-c.png)

## Keep an investigation

- **Rename a function.** Select it and enter a name under **Function name** in the Inspector. Click **Apply name**.
- **Comment on an address.** Select an instruction or mapped location, enter the note in **Address comment**, and click **Apply comment**. Clearing the field and applying removes the comment.
- **Bookmark a location.** Press **Ctrl+D** to toggle the bookmark at the selected address. Use the Inspector to name it, then revisit it through **Bookmarks** in the image browser.
- **Save and reopen.** Press **Ctrl+S** or choose **File → Save project as…**. A `.hxproj` embeds the original binary bytes along with names, comments, and bookmarks. Analysis runs again on reopen. [Project format](PROJECT_FORMAT.md)
- **Import notes from Ghidra.** Export the program XML for the same binary, then choose **File → Import Ghidra XML**. Hexaris adds mapped names, comments, and bookmarks while retaining your existing values. [Import instructions](GHIDRA_XML.md)

![Disassembly and Inspector with the selected instruction and its outgoing reference](images/workbench.png)

## Export a byte patch

On a sample you own, select the target bytes in **Hex**. Choose **File → Export byte patch…**, enter the file offset and the exact original bytes, then enter a replacement of the same length. Save to a new output path. Hexaris checks the expected bytes before writing the copy. Open the copy separately to inspect the result.

## Work with an assistant

### Copy function context

Right-click a function in the browser and choose **Function context…**, or select a function and open **Tools → Function context…**. Review the contents, then choose **Copy Markdown** to paste the evidence and review instructions into an assistant or **Copy JSON** for a script.

The export includes the image identity, virtual addresses and file offsets, decoded bytes and instructions, basic blocks and their successors, direct calls and references, register IR for small functions, Pseudo-C when supported, and coverage notices. The desktop shows 128 instructions per page. The CLI accepts `--limit` from 1 to 256 and `--offset` as a decoded-instruction index. Follow `next_instruction_offset` for subsequent pages.

```powershell
$cli = '.\target\release\hexaris-cli.exe'
$sample = '.dev\fixtures\score.exe'
& $cli context $sample 0x140001000 --format markdown
& $cli context $sample 0x140001000 --limit 4 --offset 4
```

An example question for the score fixture is “Which inputs make this function return the input plus 3? Give the addresses of the comparison, branch, and addition.”

![Function Context window with a preview and Markdown and JSON copy actions](images/function-context.png)

### Follow evidence and review annotations

Open the matching image in Hexaris and choose **Tools → Review assistant response…**. Paste the returned JSON, then click **Load response**. Click a finding’s address to open its instruction or mapped bytes. Under **Proposed annotations**, compare **Current** and **Proposed**, then choose **Apply this annotation** for each change you want to keep. Save the project to persist your choices.

You can try the workflow without an assistant. Run the following in PowerShell with the variables from the previous example. Paste the JSON printed by the last line into the review window.

```powershell
$context = & $cli context $sample 0x140001000 | ConvertFrom-Json
$response = @{
    schema_version = 1
    image = $context.image
    findings = @(@{
        text = 'This is the entry to the scoring function.'
        addresses = @($context.address)
    })
    suggestions = @(@{
        kind = 'function_name'
        address = $context.address
        value = 'score_by_threshold'
        rationale = 'The comparison selects one of two scoring rules.'
    })
}
$response | ConvertTo-Json -Depth 8
```

The response must use the `image` identity from the export, which includes the file digest, format, architecture, and base address. Use address strings such as `"0x140001000"`. The available suggestion kinds are `function_name`, `comment`, and `bookmark`. A function name must target a discovered function entry. Comments and bookmarks need mapped addresses.

![Assistant response review showing clickable evidence and the current and proposed function name](images/assistant-review.png)

### Connect a local MCP client

Configure a client that supports local stdio MCP servers. Point it at the CLI executable and pass one user-selected binary or project. The server supports protocol revisions `2025-06-18` and `2025-11-25`. [MCP stdio specification](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports)

Here is a typical client configuration. Replace both paths and use your client’s local MCP settings format.

```json
{
  "mcpServers": {
    "hexaris": {
      "command": "C:/tools/hexaris/hexaris-cli.exe",
      "args": ["mcp", "C:/samples/score.hxproj"]
    }
  }
}
```

| Read-only tool | Example arguments |
| --- | --- |
| `inspect_image` | `{}` |
| `list_functions` | `{"limit": 10}` |
| `function_context` | `{"address": "0x140001000", "limit": 32}` |
| `read_references` | `{"address": "0x140001000"}` |

The server reads one supplied image for the lifetime of the process. Restart it with a different path to inspect another file or reload a saved project. The client controls what it sends to its model provider. Hexaris itself uses stdio and opens no network listener.

## Compare two builds

Open both versions as separate tabs. Choose **Tools → Compare builds…**, select **Before** and **After**, and click **Compare**. Click a function address to open that build at the matching location. Hover over **Changed** to see the first differing instruction, or use **Copy Markdown** to include it in an assistant prompt.

To make a second sample, change the `+ 3u` expression to `+ 5u` in a local copy of the score fixture and compile it from the same Developer PowerShell. Then run the CLI or open both files in Hexaris.

```powershell
(Get-Content tests\fixtures\score.c -Raw).Replace('value + 3u', 'value + 5u') |
    Set-Content .dev\fixtures\score-next.c
cl /nologo /W4 /WX /Od /GS- /TC .dev\fixtures\score-next.c /Fo.dev\fixtures\score-next.obj /link /nologo /nodefaultlib /entry:entry /subsystem:console /out:.dev\fixtures\score-next.exe
& $cli compare .dev\fixtures\score.exe .dev\fixtures\score-next.exe --format markdown
```

Compare builds with the same architecture and format. Hexaris names the basis of each match, including relative address, unique name, or identical decoded bytes. A match by relative address alone is a hint. Reports cover discovered code, so use the analysis notices and inspect the functions before treating identical bytes as equivalent behavior.

![Build comparison between two score fixtures, with links to the changed function in each build](images/compare-builds.png)

## Command reference

Run `hexaris-cli <command> --help` for the complete arguments. These examples use paths and virtual addresses from your own image unless they explicitly take a file offset.

| Command | Example arguments | Result |
| --- | --- | --- |
| `inspect` | `sample.exe` | Image format, regions, imports, exports, and symbols |
| `triage` | `first.exe second.exe` | Ordered JSON reports with metadata, SHA-256, entropy, and per-input failures |
| `disassemble` | `sample.exe 0x140001000 --count 16` | Sequential decode at a virtual address |
| `analyze` | `sample.exe --address 0x140001000 --lift` | Function instructions, blocks, references, and optional register IR |
| `references` | `sample.exe 0x140001000` | Indexed incoming and outgoing references and direct calls |
| `decompile` | `sample.exe 0x140001000` | Supported Pseudo-C projection or the engine’s reason for withholding it |
| `hex` | `sample.exe --offset 0x400 --length 32` | Text hex dump at a file offset |
| `strings` | `sample.exe --min-length 6 --limit 20` | Printable ASCII runs |
| `find` | `sample.exe "48 8B ?? ??" --limit 20` | Byte-pattern hits with file offsets and mapped addresses |
| `patch` | `sample.exe copy.exe 0x400 --expect B8 --replace 90` | Guarded fixed-length replacement in a new copy, if that offset contains the expected byte |
| `project create` | `sample.exe sample.hxproj` | Portable project containing the image |
| `project annotate` | `sample.hxproj 0x140001000 --comment "Entry" --bookmark "Start"` | Update annotations in the project |
| `project annotation` | `sample.hxproj 0x140001000` | Read the annotations at that address |
| `project bookmarks` | `sample.hxproj` | List bookmarks |
| `project import-ghidra` | `sample.hxproj program.xml --dry-run` | Preview XML annotation merges. Omit `--dry-run` to save. |
| `context` | `sample.exe 0x140001000 --format markdown` | Bounded function evidence suitable for sharing |
| `compare` | `before.exe after.exe` | Build comparison in JSON by default |
| `mcp` | `sample.hxproj` | Read-only stdio server scoped to that project |

For raw files, commands that open one image accept `--raw x86` or `--raw x86-64` and an optional `--base 0x1000`. See the [feature matrix](FEATURE_MATRIX.md) for platform and analysis limits.
