mod bcsv;
mod field;
mod data_type;
mod bcsv_value;

use std::sync::LazyLock;

use skyline::libc::c_char;
use crc32fast::hash;

use std::collections::HashMap;

use bcsv::BCSV;
use bcsv_value::BCSVValue;

#[skyline::main(name = "acnh_additive_stuff")]
pub fn main() {
    println!("Hello from skyline plugin");

    skyline::install_hook!(get_icon_index);

    for (name, hash) in HASHES.iter() {
        println!("[HASH] {} => {}", name, hash);
    }
}

static HASHES: LazyLock<HashMap<String, u32>> = LazyLock::new(|| {
    let path = "content:/Bcsv/ItemUnitIcon.bcsv";
    let Some(file) = std::fs::read(&path).ok() else {
        println!("Failed to read BCSV file.");
        return HashMap::new();
    };

    let Some(bcsv) = BCSV::from_bytes(&file).ok() else {
        println!("Failed to parse BCSV file.");
        return HashMap::new();
    };

    println!("BCSV loaded! Hashing entries...");

    bcsv.entries.iter()
        .filter_map(|entry| entry.get(4))
        .filter_map(|value| {
            if let BCSVValue::String(s) = value {
                Some((s.clone(), crc32fast::hash(s.as_bytes())))
            } else {
                None
            }
        })
        .collect()
});

#[skyline::hook(offset = 0xf53350)]
unsafe fn get_icon_index(crc_hash: u32, icon_name_ptr: *const c_char) -> u32 {
    // Search for the index of the value in the HASHES map
    if let Some((index, (key, _))) = HASHES.iter().enumerate().find(|(_, (_, &v))| v == crc_hash) {
        //println!("[DEBUG] Matched hash {} for string \"{}\" at index {}", crc_hash, key, index);
        return index as u32;
    }

    let original_result = call_original!(crc_hash, icon_name_ptr);
    //println!("[DEBUG] get_icon_index: {} return {}", crc_hash, original_result);
    
    return original_result
}