# Development environment

Windows 11 25H2 x64, build 26200.9974, was used for the initial build and validation. The host exposes an AMD Ryzen 7 9700X with 16 logical processors and an NVIDIA GeForce RTX 5070 Ti. C: had approximately 706 GB free at discovery. The sandbox blocked the initial WMI queries for RAM and driver details.

The workspace began empty without Git metadata. The Windows toolchain used Rust 1.100.0, Cargo, rustfmt and Clippy, MSVC 14.50.35717 from Visual Studio 2026, and Windows SDK 10.0.26100.0. Rust was installed locally with rustup. The GitHub remote originally returned no branch refs. Dependency downloads used the crates.io registry and were followed by locked offline builds. The default sandbox blocked direct network access and persistent writes outside the workspace.

The optional research setup used Node 26.13.1, REA 6.10.1, Ghidra 12.2, and JDK 26 x64. Research tools and local reference installations are independently provisioned. [Research workflow](REA_INVESTIGATIONS.md)
