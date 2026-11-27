# Provenance

The Hexaris runtime is independently authored in Rust. Public Binary Ninja documentation, observed workflows, and results on original fixture source guide behavior comparisons. The repository includes original and properly licensed assets. [Research workflow](REA_INVESTIGATIONS.md)

`object` provides executable parsing and `iced-x86` provides instruction decoding. The flow traversal, SSA projection and structured leaf pseudocode are Hexaris implementations. Third-party interface assets are recorded in the [asset manifest](../assets/ASSET_MANIFEST.toml). Third-party Rust code is fetched from the dependency registry using the versions and checksums in Cargo.lock.
