use std::sync::LazyLock;

use crate::icon_registry::{IconKind, IconTable, ItemData};
use crate::native_bcsv::{BcsvCache, BorrowedString};

// Update 3.0.3. MenuIcon UI requests use Layout_MenuIcon_<ResName> natively.
const BUILD_ICON_CACHE: usize = 0x199a8a0; // ItemMenuIcon::SetupRows
const GET_MENU_ICON: usize = 0x19f43a0; // first function inside bcsv::ItemMenuIcon::Read_ResName

static ICONS: LazyLock<IconTable> = LazyLock::new(|| IconTable::new(IconKind::Menu));

#[skyline::hook(offset = BUILD_ICON_CACHE)]
pub unsafe fn build_menu_icon_cache_hook(
    cache: *mut BcsvCache,
    path: *const BorrowedString,
    allocator: *mut u8,
) {
    let relevant = ICONS.begin_build(cache, path);
    call_original!(cache, path, allocator);
    if relevant {
        ICONS.finish_build(cache);
    }
}

#[skyline::hook(offset = GET_MENU_ICON)]
pub unsafe fn get_menu_icon_hook(item: *const ItemData) -> u16 {
    ICONS
        .index_for(item)
        .unwrap_or_else(|| call_original!(item))
}
