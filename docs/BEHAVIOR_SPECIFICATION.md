# Behavior specification

Opening an image reads bounded file data and resolves loader metadata. If metadata loads and the architecture supports decoding, Hexaris schedules static analysis and selects a discovered entry. A failed open leaves the current session available. Raw images use caller-provided mapping settings, and mapped images obtain addresses from their file format.

An address selection activates the disassembly if the address is a decoded instruction. If it maps only to file bytes, Hexaris jumps to the corresponding hex row. Use **Analyze here** to start bounded function discovery from a selected mapped code address, or **Linear** to decode positions sequentially. A flow block opens its first instruction. Byte search and string scans operate on disk offsets.

Annotations are local to the project. Applying a comment or name marks it unsaved. Saving writes the portable project as a separate file and reopens it using its embedded bytes. Unsupported analysis operations surface a reason where relevant. [Project format](PROJECT_FORMAT.md)
