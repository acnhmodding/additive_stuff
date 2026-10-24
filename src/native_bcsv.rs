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

/// The cache arrays an extension writes, borrowed once validated.
pub struct CacheExtension<'a> {
    pub table: BcsvView<'a>,
    valid_flags: &'a [u8],
    raw_to_alias: &'a mut [u16],
    rows: &'a mut [*const u8],
    row_count: &'a mut u32,
}

impl BcsvCache {
    pub unsafe fn extension(&mut self) -> Result<CacheExtension<'_>, &'static str> {
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
            || !self.rows.data.is_aligned()
            || !self.raw_to_alias.data.is_aligned()
            || self.row_count != self.configured_count
            || self.row_count == 0
            || self.row_count >= UNMAPPED_ALIAS as u32
        {
            return Err("native cache is not ready; extension skipped");
        }
        let table = BcsvView::from_native((*self.loaded_file).header)
            .ok_or("invalid native BCSV header")?;
        if table.count() > self.raw_to_alias.capacity as usize
            || table.count() > self.valid_flags.capacity as usize
            || self.row_count > self.rows.capacity
        {
            return Err("native cache arrays are too small");
        }
        // Aliases must stay below UNMAPPED_ALIAS, so later slots are unusable.
        let slots = (self.rows.capacity as usize).min(UNMAPPED_ALIAS as usize);
        Ok(CacheExtension {
            table,
            valid_flags: std::slice::from_raw_parts(self.valid_flags.data, table.count()),
            raw_to_alias: std::slice::from_raw_parts_mut(self.raw_to_alias.data, table.count()),
            rows: std::slice::from_raw_parts_mut(self.rows.data, slots),
            row_count: &mut self.row_count,
        })
    }
}

impl CacheExtension<'_> {
    pub fn is_valid(&self, raw_index: usize) -> bool {
        self.valid_flags[raw_index] != 0
    }

    /// Whether the native cache already gave this raw row an alias.
    pub fn is_mapped(&self, raw_index: usize) -> Result<bool, &'static str> {
        match self.raw_to_alias[raw_index] {
            UNMAPPED_ALIAS => Ok(false),
            alias if (alias as u32) < *self.row_count => Ok(true),
            _ => Err("invalid existing native alias"),
        }
    }

    /// The alias append gives the row after `appended` earlier ones.
    pub fn next_alias(&self, appended: usize) -> u16 {
        (*self.row_count as usize + appended) as u16
    }

    /// Map (raw index, row) pairs to the next free aliases, all or nothing.
    /// Native loading/rebuild synchronization remains the game's contract.
    pub fn append(&mut self, new_rows: &[(usize, *const u8)]) -> Result<(), &'static str> {
        let start = *self.row_count as usize;
        let slots = self
            .rows
            .get_mut(start..start + new_rows.len())
            .ok_or("not enough unused native row slots")?;
        if slots.iter().any(|slot| !slot.is_null()) {
            return Err("new row slot is already occupied");
        }
        for (alias, (slot, &(raw_index, row))) in (start..).zip(slots.iter_mut().zip(new_rows)) {
            *slot = row;
            self.raw_to_alias[raw_index] = alias as u16;
        }
        // Count last; no allocation or fallible operation during publication.
        *self.row_count += new_rows.len() as u32;
        Ok(())
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
