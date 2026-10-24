use crate::bcsv::{crc32, StringField};
use crate::icon_registry::{icon_hooks, IconSpec, IconTable};
use crate::native_bcsv::BorrowedString;

// Update 3.0.3 offsets, relative to Skyline's relocated text region.

// ItemUnitIcon
const BUILD_UNIT_ICON_CACHE: usize = 0x199c360; // ItemUnitIcon::SetupRows
const GET_UNIT_ICON: usize = 0x19f93e8;
const COLLECT_ICON_RESOURCES: usize = 0x11365e0;
const REGISTER_RESOURCE: usize = 0x657850;

static UNIT_ICONS: IconTable = IconTable::new(IconSpec {
    filename: b"ItemUnitIcon.bcsv",
    item_field: crc32(b"ItemUnitIcon.hshCstringRef"),
    model_name: Some(StringField::new(b"ModelName string32", 32)),
    res_name: StringField::new(b"ResName string64", 64),
    preload_resources: true,
});

icon_hooks!(
    UNIT_ICONS,
    build_unit_icon_cache_hook = BUILD_UNIT_ICON_CACHE,
    get_unit_icon_hook = GET_UNIT_ICON
);

// ItemMenuIcon. MenuIcon UI requests use Layout_MenuIcon_<ResName> natively.
const BUILD_MENU_ICON_CACHE: usize = 0x199a8a0; // ItemMenuIcon::SetupRows
const GET_MENU_ICON: usize = 0x19f43a0; // first function inside bcsv::ItemMenuIcon::Read_ResName

static MENU_ICONS: IconTable = IconTable::new(IconSpec {
    filename: b"ItemMenuIcon.bcsv",
    item_field: crc32(b"ItemMenu.hshCstringRef"),
    model_name: None,
    res_name: StringField::new(b"ResName string32", 32),
    preload_resources: false,
});

icon_hooks!(
    MENU_ICONS,
    build_menu_icon_cache_hook = BUILD_MENU_ICON_CACHE,
    get_menu_icon_hook = GET_MENU_ICON
);

// Native ACNH_ActorMultiResArrayLook; its private layout is not used here.
#[repr(C)]
pub struct ResourceCollection {
    _opaque: [u8; 0],
}

unsafe extern "C" fn borrowed_string_noop(_: *const BorrowedString) {}

// Only for the synchronous registrar, which calls slot +0x18 and copies text.
static RESOURCE_STRING_VTABLE: [Option<unsafe extern "C" fn(*const BorrowedString)>; 4] =
    [None, None, None, Some(borrowed_string_noop)];

#[skyline::hook(offset = COLLECT_ICON_RESOURCES)]
pub unsafe fn collect_icon_resources_hook(collection: *mut ResourceCollection, mode: i8) -> u64 {
    let result = call_original!(collection, mode);
    if mode != 0 || collection.is_null() {
        return result;
    }
    let Some(registry) = UNIT_ICONS.current() else {
        return result;
    };
    let base = skyline::hooks::getRegionAddress(skyline::hooks::Region::Text) as usize;
    let register: unsafe extern "C" fn(*mut ResourceCollection, *const BorrowedString, i32) =
        std::mem::transmute(base + REGISTER_RESOURCE);
    for name in &registry.resource_names {
        let wrapper = BorrowedString {
            vtable: RESOURCE_STRING_VTABLE.as_ptr() as usize,
            text: name.as_ptr().cast(),
        };
        register(collection, &wrapper, 1);
    }
    result
}
