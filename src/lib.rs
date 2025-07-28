use skyline::libc::c_char;
use once_cell::sync::Lazy;
use crc32fast::hash;

#[skyline::main(name = "acnh_additive_stuff")]
pub fn main() {
    println!("Hello from skyline plugin");
    println!("Banana hash: {}", *BANANA_HASH);

    skyline::install_hook!(get_icon_index);
}

static BANANA_HASH: Lazy<u32> = Lazy::new(|| hash(b"Banana"));

#[skyline::hook(offset = 0xf53350)]
unsafe fn get_icon_index(crc_hash: u32, icon_name_ptr: *const c_char) -> u32 {
    
    // TODO: We need to make a BCSV rust library so we can automatically check all hashes in ItemUnitIcon.bcsv
    if (crc_hash == *BANANA_HASH)
    {
        println!("[DEBUG] Banana initialized");
        return 218;
    }

    let aa = call_original!(crc_hash, icon_name_ptr);
    println!("[DEBUG] get_icon_index: {} return {}", crc_hash, aa);
    
    return aa
}