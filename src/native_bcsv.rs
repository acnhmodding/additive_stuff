use std::ptr::{read_unaligned, write_unaligned};

use crate::bcsv::BcsvView;

pub const UNMAPPED_ALIAS: u16 = u16::MAX;

#[repr(C)]
struct LoadedFile {
    reserved_000: usize,
    header: *const u8,
}

// Backing allocation only; logical row_count lives separately in BcsvCache.
#[repr(C)]
struct NativeBuffer<T> {
    capacity: u32,
    reserved_004: u32,
    data: *mut T,
}

// Verified for Update 3.0.3 ItemUnitIcon and ItemMenuIcon. This is a
// native runtime object, not an on-disk BCSV header or a generic enum ABI.
#[repr(C)]
pub struct BcsvCache {
    reserved_000: [u8; 0xb0],
    loaded_file: *const LoadedFile, // 0x0b0
    reserved_0b8: [u8; 0x10],
    valid_flags: NativeBuffer<u8>, // 0x0c8
    reserved_0d8: [u8; 0xb8],
    configured_count: u32, // 0x190
    padding_194: u32,
    key_accessor: usize,             // 0x198
    key_adjustment_and_virtual: i64, // 0x1a0
    sort_comparator: usize,          // 0x1a8
    rows: NativeBuffer<*const u8>,   // 0x1b0
    row_count: u32,                  // 0x1c0
    padding_1c4: u32,
    raw_to_alias: NativeBuffer<u16>, // 0x1c8
}

#[repr(C)]
pub struct BorrowedString {
    pub vtable: usize,
    pub text: *const u8,
}

pub struct RowAssignment {
    raw_index: usize,
    pub alias: u16,
    row: *const u8,
}

impl BcsvCache {
    pub unsafe fn extension_table(&self) -> Result<BcsvView<'_>, &'static str> {
        if self.key_accessor == 0
            || self.key_adjustment_and_virtual != 0
            || self.sort_comparator != 0
        {
            return Err("unsupported native cache key configuration");
        }
        if self.loaded_file.is_null()
            || self.rows.data.is_null()
            || self.raw_to_alias.data.is_null()
            || self.valid_flags.data.is_null()
            || self.row_count != self.configured_count
            || self.row_count == 0
            || self.row_count >= UNMAPPED_ALIAS as u32
        {
            return Err("native cache is not ready; extension skipped");
        }
        let table = BcsvView::from_native((*self.loaded_file).header)
            .ok_or("invalid native BCSV header")?;
        if table.count > self.raw_to_alias.capacity as usize
            || table.count > self.valid_flags.capacity as usize
            || self.row_count > self.rows.capacity
        {
            return Err("native cache arrays are too small");
        }
        Ok(table)
    }

    // Call after extension_table has validated the native arrays and counts.
    pub unsafe fn is_valid_row(&self, raw_index: usize) -> bool {
        raw_index < self.valid_flags.capacity as usize && *self.valid_flags.data.add(raw_index) != 0
    }

    pub unsafe fn row_alias(&self, raw_index: usize) -> Result<u16, &'static str> {
        if raw_index >= self.raw_to_alias.capacity as usize || self.raw_to_alias.data.is_null() {
            return Err("invalid native raw row index");
        }
        let alias = read_unaligned(self.raw_to_alias.data.add(raw_index));
        if alias != UNMAPPED_ALIAS && alias as u32 >= self.row_count {
            return Err("invalid existing native alias");
        }
        Ok(alias)
    }

    pub unsafe fn plan_row(
        &self,
        raw_index: usize,
        row: *const u8,
        appended: usize,
    ) -> Result<RowAssignment, &'static str> {
        let alias = self.row_count as usize + appended;
        if alias >= UNMAPPED_ALIAS as usize || alias >= self.rows.capacity as usize {
            return Err("not enough unused native row slots");
        }
        if !read_unaligned(self.rows.data.add(alias)).is_null() {
            return Err("new row slot is already occupied");
        }
        Ok(RowAssignment {
            raw_index,
            alias: alias as u16,
            row,
        })
    }

    /// Publish assignments prepared for this unchanged cache by plan_row.
    /// Native loading/rebuild synchronization remains the game's contract.
    pub unsafe fn publish_rows(&mut self, assignments: &[RowAssignment]) {
        for assignment in assignments {
            write_unaligned(
                self.rows.data.add(assignment.alias as usize),
                assignment.row,
            );
            write_unaligned(
                self.raw_to_alias.data.add(assignment.raw_index),
                assignment.alias,
            );
        }
        // Count last; no allocation or fallible operation during publication.
        self.row_count += assignments.len() as u32;
    }
}

const _: () = {
    assert!(std::mem::size_of::<NativeBuffer<u8>>() == 0x10);
    assert!(std::mem::size_of::<NativeBuffer<*const u8>>() == 0x10);
    assert!(std::mem::size_of::<NativeBuffer<u16>>() == 0x10);
    assert!(std::mem::offset_of!(NativeBuffer<u8>, data) == 0x8);
    assert!(std::mem::size_of::<BcsvCache>() == 0x1d8);
    assert!(std::mem::offset_of!(BcsvCache, loaded_file) == 0xb0);
    assert!(std::mem::offset_of!(BcsvCache, valid_flags) == 0xc8);
    assert!(std::mem::offset_of!(BcsvCache, valid_flags.data) == 0xd0);
    assert!(std::mem::offset_of!(BcsvCache, configured_count) == 0x190);
    assert!(std::mem::offset_of!(BcsvCache, key_accessor) == 0x198);
    assert!(std::mem::offset_of!(BcsvCache, rows) == 0x1b0);
    assert!(std::mem::offset_of!(BcsvCache, rows.data) == 0x1b8);
    assert!(std::mem::offset_of!(BcsvCache, row_count) == 0x1c0);
    assert!(std::mem::offset_of!(BcsvCache, raw_to_alias) == 0x1c8);
    assert!(std::mem::offset_of!(BcsvCache, raw_to_alias.data) == 0x1d0);
    assert!(std::mem::size_of::<BorrowedString>() == 0x10);
    assert!(std::mem::offset_of!(BorrowedString, text) == 0x8);
};
