use std::sync::LazyLock;

use crate::icon_registry::{IconKind, IconTable, ItemData};
use crate::native_bcsv::{BcsvCache, BorrowedString};

// Update 3.0.3 offsets, relative to Skyline's relocated text region.
const BUILD_ICON_CACHE: usize = 0x199c360;
const GET_UNIT_ICON: usize = 0x19f93e8;
const COLLECT_ICON_RESOURCES: usize = 0x11365e0;
const REGISTER_RESOURCE: usize = 0x657850;

static ICONS: LazyLock<IconTable> = LazyLock::new(|| IconTable::new(IconKind::Unit));

// Native ACNH_ActorMultiResArrayLook; its private layout is not used here.
#[repr(C)]
pub struct ResourceCollection {
    _opaque: [u8; 0],
}

unsafe extern "C" fn borrowed_string_noop(_: *const BorrowedString) {}

// Only for the synchronous registrar, which calls slot +0x18 and copies text.
static RESOURCE_STRING_VTABLE: [Option<unsafe extern "C" fn(*const BorrowedString)>; 4] =
    [None, None, None, Some(borrowed_string_noop)];

#[skyline::hook(offset = BUILD_ICON_CACHE)]
pub unsafe fn build_icon_cache_hook(
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

#[skyline::hook(offset = GET_UNIT_ICON)]
pub unsafe fn get_unit_icon_hook(item: *const ItemData) -> u16 {
    ICONS
        .index_for(item)
        .unwrap_or_else(|| call_original!(item))
}

unsafe fn queue_resource_names(collection: *mut ResourceCollection, names: &[Vec<u8>]) {
    let base = skyline::hooks::getRegionAddress(skyline::hooks::Region::Text) as usize;
    let register: unsafe extern "C" fn(*mut ResourceCollection, *const BorrowedString, i32) =
        std::mem::transmute(base + REGISTER_RESOURCE);
    for name in names {
        let wrapper = BorrowedString {
            vtable: RESOURCE_STRING_VTABLE.as_ptr() as usize,
            text: name.as_ptr(),
        };
        register(collection, &wrapper, 1);
    }
}

#[skyline::hook(offset = COLLECT_ICON_RESOURCES)]
pub unsafe fn collect_icon_resources_hook(collection: *mut ResourceCollection, mode: i8) -> u64 {
    let result = call_original!(collection, mode);
    if mode == 0 && !collection.is_null() {
        if let Some(registry) = ICONS.current() {
            queue_resource_names(collection, &registry.resource_names);
        }
    }
    result
}
