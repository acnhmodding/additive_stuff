use std::ffi::CStr;

/// CRC-32 (IEEE), the hash of BCSV field descriptors and string references.
pub const fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    let mut i = 0;
    while i < bytes.len() {
        crc ^= bytes[i] as u32;
        let mut bit = 0;
        while bit < 8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & (crc & 1).wrapping_neg());
            bit += 1;
        }
        i += 1;
    }
    !crc
}

/// A string field descriptor: crc32("Name stringN") and its byte width.
#[derive(Clone, Copy)]
pub struct StringField {
    hash: u32,
    width: usize,
}

impl StringField {
    pub const fn new(name: &[u8], width: usize) -> Self {
        Self {
            hash: crc32(name),
            width,
        }
    }
}

/// A string field resolved to its offset in one table.
#[derive(Clone, Copy)]
pub struct Column {
    offset: usize,
    width: usize,
}

impl Column {
    /// The non-empty NUL-terminated string stored in this column of `row`.
    pub fn read<'r>(&self, row: &'r [u8]) -> Option<&'r CStr> {
        let bytes = row.get(self.offset..self.offset.checked_add(self.width)?)?;
        let text = CStr::from_bytes_until_nul(bytes).ok()?;
        (!text.is_empty()).then_some(text)
    }
}

/// Row geometry, relative to the start of the table header.
#[derive(Clone, Copy)]
struct TableLayout {
    count: usize,
    stride: usize,
    rows_start: usize,
}

impl TableLayout {
    fn row_offset(&self, index: usize) -> Option<usize> {
        (index < self.count).then(|| self.rows_start + index * self.stride)
    }

    fn row_index(&self, offset: usize) -> Option<usize> {
        let position = offset.checked_sub(self.rows_start)?;
        let index = position / self.stride;
        (position % self.stride == 0 && index < self.count).then_some(index)
    }
}

/// A borrowed BCSV table; field types and widths come from its caller.
#[derive(Clone, Copy)]
pub struct BcsvView<'a> {
    bytes: &'a [u8],
    layout: TableLayout,
    fields_start: usize,
}

impl<'a> BcsvView<'a> {
    /// The caller must provide a live native allocation containing the entire
    /// table described by its header. Header sizes are not allocation bounds.
    pub unsafe fn from_native(header: *const u8) -> Option<Self> {
        if header.is_null() {
            return None;
        }
        let prefix = std::slice::from_raw_parts(header, 12);
        let count = u32::from_le_bytes(prefix[0..4].try_into().ok()?) as usize;
        let stride = u32::from_le_bytes(prefix[4..8].try_into().ok()?) as usize;
        let fields = u16::from_le_bytes(prefix[8..10].try_into().ok()?) as usize;
        if count == 0
            || count > u16::MAX as usize
            || !(4..=0x10000).contains(&stride)
            || !(1..=1024).contains(&fields)
        {
            return None;
        }
        let fields_start = match prefix[10] {
            0 => 12,
            1 if std::slice::from_raw_parts(header.add(12), 4) == b"VSCB" => 28,
            _ => return None,
        };
        let rows_start = fields_start + fields * 8;
        let len = rows_start + count * stride;
        if len > u32::MAX as usize || (header as usize).checked_add(len).is_none() {
            return None;
        }
        let view = Self {
            bytes: std::slice::from_raw_parts(header, len),
            layout: TableLayout {
                count,
                stride,
                rows_start,
            },
            fields_start,
        };
        if view.fields().any(|(_, offset)| offset >= stride) {
            return None;
        }
        Some(view)
    }

    pub fn count(&self) -> usize {
        self.layout.count
    }

    fn fields(&self) -> impl Iterator<Item = (u32, usize)> + '_ {
        self.bytes[self.fields_start..self.layout.rows_start]
            .chunks_exact(8)
            .map(|field| {
                (
                    u32::from_le_bytes(field[0..4].try_into().unwrap()),
                    u32::from_le_bytes(field[4..8].try_into().unwrap()) as usize,
                )
            })
    }

    fn field(&self, hash: u32, width: usize) -> Option<usize> {
        let mut matches = self.fields().filter(|(candidate, _)| *candidate == hash);
        let (_, offset) = matches.next()?;
        if matches.next().is_some() || offset < 4 || offset.checked_add(width)? > self.layout.stride
        {
            return None;
        }
        Some(offset)
    }

    pub fn column(&self, field: StringField) -> Option<Column> {
        Some(Column {
            offset: self.field(field.hash, field.width)?,
            width: field.width,
        })
    }

    pub fn row(&self, index: usize) -> Option<&'a [u8]> {
        let offset = self.layout.row_offset(index)?;
        let row = &self.bytes[offset..offset + self.layout.stride];
        (u32::from_le_bytes(row[0..4].try_into().ok()?) as usize == offset).then_some(row)
    }
}

/// A u32 field of a live native table, resolved once and rechecked per row.
#[derive(Clone, Copy)]
pub struct NativeU32Field {
    header: usize,
    layout: TableLayout,
    offset: usize,
}

impl NativeU32Field {
    /// Resolve the field from a live native row whose first word is its file
    /// offset. The same allocation validity requirements as from_native apply.
    pub unsafe fn resolve(row: *const u8, field_hash: u32) -> Option<Self> {
        let header = header_of(row)?;
        let table = BcsvView::from_native(header as *const u8)?;
        let field = Self {
            header,
            layout: table.layout,
            offset: table.field(field_hash, 4)?,
        };
        field.contains(row).then_some(field)
    }

    /// Read the field, or None if row does not belong to the resolved table.
    pub unsafe fn read(&self, row: *const u8) -> Option<u32> {
        self.contains(row)
            .then(|| std::ptr::read_unaligned(row.add(self.offset) as *const u32))
    }

    unsafe fn contains(&self, row: *const u8) -> bool {
        if header_of(row) != Some(self.header) {
            return false;
        }
        // Reject a different table reallocated at the same address.
        let header = self.header as *const u32;
        std::ptr::read_unaligned(header) as usize == self.layout.count
            && std::ptr::read_unaligned(header.add(1)) as usize == self.layout.stride
            && self.layout.row_index(row as usize - self.header).is_some()
    }
}

/// The table header of a native row, from the row's own file offset.
unsafe fn header_of(row: *const u8) -> Option<usize> {
    if row.is_null() {
        return None;
    }
    let position = std::ptr::read_unaligned(row as *const u32) as usize;
    if position == 0 || position > row as usize {
        return None;
    }
    Some(row as usize - position)
}

const _: () = assert!(crc32(b"Label string32") == 0x87bf00e8);
