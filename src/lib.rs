mod bcsv;
mod field;
mod data_type;
mod bcsv_value;

use std::ffi::{CStr, CString};
use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use skyline::libc::c_char;
use crc32fast::hash;

use std::collections::HashMap;

use bcsv::BCSV;
use bcsv_value::BCSVValue;
use skyline::patching::Patch;

#[skyline::main(name = "acnh_additive_stuff")]
pub fn main() {
    skyline::install_hooks!(
        get_unit_icon_hook,
        load_icon_data_hook,
        get_icon_row_hook
    );
}

fn load_bcsv_bytes_from_content(file_name: &str) -> Option<Vec<u8>> {
    let path = format!("content:/Bcsv/{}.bcsv", file_name);
    std::fs::read(&path).ok()
}

static CUSTOM_ICON_BCSV_BYTES: LazyLock<Box<[u8]>> = LazyLock::new(|| {
    let Some(bytes) = load_bcsv_bytes_from_content("ItemUnitIcon") else {
        println!("[DEBUG] Failed to load custom ItemUnitIcon file");
        return Box::<[u8]>::default();
    };

    let Some(bcsv) = BCSV::from_bytes(&bytes).ok() else {
        println!("[DEBUG] Failed to parse custom ItemUnitIcon file");
        return Box::<[u8]>::default();
    };

    bytes.into_boxed_slice()
});

static CUSTOM_ICON_ROW_PTRS: LazyLock<Box<[usize]>> = LazyLock::new(|| {
    let Some(bcsv) = BCSV::from_bytes(&CUSTOM_ICON_BCSV_BYTES).ok() else {
        println!("[DEBUG] Failed to parse custom ItemUnitIcon rows");
        return Vec::<usize>::new().into_boxed_slice();
    };

    let mut row_ptrs = Vec::with_capacity(bcsv.entries.len());
    let entry_size = bcsv.entry_size as usize;
    let rows_start = bcsv.rows_start;

    for row_index in 0..bcsv.entries.len() {
        let row_offset = rows_start + row_index * entry_size;
        if row_offset + entry_size > CUSTOM_ICON_BCSV_BYTES.len() {
            break;
        }
        row_ptrs.push(unsafe { CUSTOM_ICON_BCSV_BYTES.as_ptr().add(row_offset) as usize });
    }

    row_ptrs.into_boxed_slice()
});

// ItemUnitIcon row structure from ItemUnitIcon.bcsv
#[repr(C)]
#[allow(dead_code)]
struct ItemUnitIconRow {
    file_offset: u32,                 // 0x00
    scale_offset: f32,                // 0x04
    catalog_item_unique_id: u16,      // 0x08
    unique_id: u16,                   // 0x0A
    color_index: u8,                  // 0x0C
    label: [u8; 32],                  // 0x0D
    model_name: [u8; 32],             // 0x2D
    name: [u8; 64],                   // 0x4D
    res_name: [u8; 64],               // 0x8D
}

// Not sure if this is correct either
#[repr(C)]
struct PackedItemKey {
    item_id: u16,    // 0x08..0x09
    flag_0a: u8,     // 0x0A
    flag_0b: u8,     // 0x0B
    selector: u32,   // 0x0C..0x0F, packed selector from item state
}

#[repr(C)]
struct GetUnitIconCtx {
    iface_or_vtable: *const u8, // 0x00
    packed_item: PackedItemKey, // 0x08..0x0F
    item_param_row: *const u8,  // 0x10, cached row pointer used by the icon chooser
}


// Not sure if this is correct
#[repr(C)]
struct LoadIconDataCtx {
    vtable_or_iface: *const u8, // 0x00
    text: *const c_char,        // 0x08
}


#[skyline::hook(offset = 0xfbf920)]
unsafe fn get_unit_icon_hook(param1: *const GetUnitIconCtx) -> u16 {
    //return 5;
    call_original!(param1)
}

#[skyline::hook(offset = 0xf6ccc0)]
unsafe fn load_icon_data_hook(param_1: *mut LoadIconDataCtx) {
    call_original!(param_1);

    // For some reason we need this on custom icon, but it seems to break vanilla icons
    std::thread::yield_now();
}

const TEST_ICON_INDEX: u32 = 217;

#[skyline::hook(offset = 0xf6cb70)]
unsafe fn get_icon_row_hook(param1: *const u8, icon_index: u32) -> *const u32 {
    if icon_index == TEST_ICON_INDEX {
        if let Some(row_ptr) = CUSTOM_ICON_ROW_PTRS.get(icon_index as usize).copied() {
            return row_ptr as *const u32;
        }
    }

    return call_original!(param1, icon_index);
}
