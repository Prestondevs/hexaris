# Import Ghidra program XML

Export a program as XML from Ghidra. Open the same image in Hexaris and choose **File → Import Ghidra XML**. The import adds function names, comments and bookmarks at file-backed addresses. Existing annotations retain their values. The status bar reports added, retained and skipped items. Save the current Hexaris project to keep the imported metadata.

```sh
hexaris-cli project import-ghidra project.hxproj program.xml --dry-run
hexaris-cli project import-ghidra project.hxproj program.xml
```

The importer checks the image base, processor width and byte order when the export supplies them. XML exports may lack a file digest, so choose the export for the currently open binary. The parser accepts UTF-8 exports up to 16 MiB, ignores Ghidra’s internal DTD and rejects custom entities and external identifiers. The import reads the XML alone. External `.bytes` files, database containers, overlay address spaces, types, function signatures and data definitions are outside its current scope.

To generate a reproducible fixture export, use [the fixture instructions](../tests/fixtures/README.md).
