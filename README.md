# ACNH Additive Stuff

A Skyline plugin for Animal Crossing: New Horizons that adds custom UnitIcons and MenuIcons alongside the original icons. It reads metadata from `ItemUnitIcon.bcsv` and `ItemMenuIcon.bcsv` while preserving native selection for vanilla entries.

## Project Plan

- Keep vanilla icon behavior unchanged for existing icon IDs.
- Read the icon table and identify custom entries without reproducing unrelated game data structures.
- Share runtime cache extension and label-hash lookup between icon types.
- Preload custom UnitIcon resources and retain native named UI requests for MenuIcons.
- Verify changes with a Skyline build and in-game testing.

UnitIcon and MenuIcon rendering require in-game verification after changes; other icon categories are not supported.

## Supported Game Version

This build targets **3.0.3** only (main build ID `FF1D1C05670DB6021C85B624A710B963`). Do not use it on 2.0.6 or another update: hook addresses and native cache layouts are version-specific.

Build custom `ItemUnitIcon.bcsv` files from the **3.0.3 original**, which contains 218 vanilla rows. Preserve those rows and append custom entries. Do not reuse a 2.0.6 table or assume the first custom icon is index 217; the plugin assigns custom indices after the native cache's configured count.

## Source Layout

- `src/lib.rs`: Skyline plugin entry point and hook registration.
- `src/icon_registry.rs`: shared custom-row validation, cache extension, and item-hash lookup.
- `src/unit_icons.rs`: UnitIcon cache, lookup, and resource-collection hooks.
- `src/menu_icons.rs`: MenuIcon cache and lookup hooks.
- `src/bcsv.rs`: borrowed BCSV rows and field access, without copying the table.
- `src/native_bcsv.rs`: game cache layout and validated row publication.

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

### MenuIcons

1. Start with the 3.0.3 `ItemMenuIcon.bcsv`, preserve its 618 vanilla rows, and append a row with a unique `Label string32` and a valid `ResName string32`.
2. Set the item's `ItemParam.bcsv` field **`ItemMenu.hshCstringRef`** to the CRC32 of that label. This is not named `ItemMenuIcon.hshCstringRef`.
3. Supply the matching UI resource, following the vanilla format: `romfs/Model/Layout_MenuIcon_<ResName>.Nin_NX_NVN.zs`. Renaming an incompatible asset is not sufficient.
4. Build and test the new icon alongside vanilla inventory/menu icons and UnitIcons.

MenuIcons do not use the UnitIcon resource-preload hook. The traced native UI path reads the selected row's `ResName` and constructs `Layout_MenuIcon_<ResName>`. The new MenuIcon hooks and shared registry have host-side checks, but custom UI resource loading has not been tested in-game.

## Development Notes

Game updates may require changes to the offsets in `src/unit_icons.rs` and `src/menu_icons.rs`, or to the layout in `src/native_bcsv.rs`. Confirm changes in-game in addition to running `cargo skyline check`.

### 3.0.3 Binary References

Offsets below are relative to the main text region:

| Target | Offset |
| --- | --- |
| UnitIcon cache builder | `0x199c360` |
| Item-to-UnitIcon selector | `0x19f93e8` |
| Icon resource collector | `0x11365e0` |
| Resource-name registrar | `0x657850` |
| MenuIcon cache builder | `0x199a8a0` |
| Item-to-MenuIcon selector | `0x19f43a0` |

MenuIcon selection reaches the native row reader at `0x19f4cc0`, then the UI request builder at `0x3312d40`. These two addresses are research references, not additional plugin dependencies.

The cache fields used by `native_bcsv.rs` retain their 2.0.6 offsets. Resource registration uses the native registrar rather than reading its private pool layout: `0x657850` deduplicates names and calls `0x657c10`, which appends only when used count is below capacity. Like vanilla registration, this relies on the game maintaining a valid pool; a full pool silently drops additional requests. In-game rendering still requires runtime verification.

The validity flags, row-pointer storage, and raw-row aliases share a 16-byte backing-buffer layout (capacity at `+0`, pointer at `+8`), represented by `NativeBuffer<T>`. The cache's published `row_count` is separate from that capacity. Rust treats the resource collection as opaque, so identifying more of its fields in Ghidra does not add layout dependencies to the plugin.

### Remaining Dependencies

UnitIcons use three hooks and one native call; MenuIcons add two hooks. The 3.0.3 decompilation confirms that the UnitIcon selector at `0x19f93e8` accepts native icon indices only below `0xda`, and the collector at `0x11365e0` enumerates exactly `0xda` UnitIcon entries when its mode is zero. The MenuIcon selector similarly rejects custom indices at or above `0x26a`. Extending the caches alone does not remove these restrictions.

Replacing the collector hook with a registrar hook is not yet validated. The registrar also receives foreground-model requests in mode 1 and buoy requests in mode 0. Such a replacement would need to identify the UnitIcon batch and preserve vanilla registration priority; matching mode 1 alone is insufficient. No reliable virtual-dispatch replacement for the registrar address has been established. These are constraints of the current design, not proof that four addresses are the theoretical minimum.
