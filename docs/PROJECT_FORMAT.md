# Project format

Hexaris stores a portable `.hxproj` with an eight-byte header (`48 45 58 50 52 4A 00` and a version byte), a four-byte little-endian metadata length, a UTF-8 JSON record, and the original binary bytes. Version 1 records the display name, loader mode, byte count, SHA-256 digest, address comments, and function-name overrides. Version 2 adds bookmarks. Version 3 adds recovered argument names keyed by function address and argument number. Hexaris reads all three versions and writes the earliest version that can store the project's annotations. Analysis results are recomputed on load.

Metadata is limited to 4 MiB, and embedded binaries are limited to 256 MiB. The reader verifies the byte count, checksum, and annotation bounds. The writer stages output in the destination directory before replacing an existing project. New project creation refuses an existing destination.

The CLI `project annotate` subcommand updates a name, comment, or bookmark. An empty value removes that annotation. `project bookmarks` lists saved bookmarks in address order. Multiple independent editors of the same project are not coordinated. [Ghidra XML import](GHIDRA_XML.md)
