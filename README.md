A Skyline plugin for **Animal Crossing: New Horizons** that aim to extend the game limits so additive mods can benefit from new icons, interiors and such.

## Requirements

- Rust toolchain supported by the installed `cargo-skyline` version.
- The `cargo-skyline` command-line tool.

Check that both tools are available:

```sh
rustc --version
cargo skyline --version
```

## Build

From the project directory, run a fast compile check:

```sh
cargo skyline check
```

Build the release NRO:

```sh
cargo skyline build --release
```

The generated plugin is written under `target/aarch64-skyline-switch/release/`. Use the `.nro` artifact for installation or packaging. Ensure the Skyline title ID in `Cargo.toml` matches the intended game before distributing or installing the plugin.

## Custom Icon Workflow

1. Add the icon row and its resource files to the mod's `romfs`, following the game's expected resource naming and format.
2. Ensure `ItemUnitIcon.bcsv` contains the matching label, model name, and resource name.
3. Build the plugin and install it with the corresponding game data mod.
4. Test both vanilla icons and the custom icon in-game. A successful compile does not validate resource names or game-side loading.

The plugin's icon selection depends on the requesting item's icon label/hash matching the entry in `ItemUnitIcon.bcsv`. Keep the BCSV row and its referenced resources consistent.