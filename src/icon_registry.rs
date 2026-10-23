use std::collections::{HashMap, HashSet};
use std::ffi::CStr;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

use crc32fast::hash as crc32;

use crate::bcsv::{string_field, BcsvView};
use crate::native_bcsv::{BcsvCache, BorrowedString, UNMAPPED_ALIAS};

#[repr(C)]
pub struct ItemData {
    reserved_000: [u8; 0x10],
    item_param_row: *const u8,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum IconKind {
    Unit,
    Menu,
}

impl IconKind {
    fn filename(self) -> &'static [u8] {
        match self {
            Self::Unit => b"ItemUnitIcon.bcsv",
            Self::Menu => b"ItemMenuIcon.bcsv",
        }
    }

    fn item_field(self) -> &'static [u8] {
        match self {
            Self::Unit => b"ItemUnitIcon.hshCstringRef",
            Self::Menu => b"ItemMenu.hshCstringRef",
        }
    }
}

#[derive(PartialEq, Eq)]
pub struct IconRegistry {
    indices_by_label_hash: HashMap<u32, u16>,
    pub resource_names: Vec<Vec<u8>>,
}

pub struct IconTable {
    kind: IconKind,
    item_field_hash: u32,
    // None blocks custom lookups during rebuilds or after extension failure.
    registries: Mutex<HashMap<usize, Option<Arc<IconRegistry>>>>,
    warned: AtomicBool,
}

impl IconTable {
    pub fn new(kind: IconKind) -> Self {
        Self {
            kind,
            item_field_hash: crc32(kind.item_field()),
            registries: Mutex::new(HashMap::new()),
            warned: AtomicBool::new(false),
        }
    }

    pub unsafe fn begin_build(&self, cache: *mut BcsvCache, path: *const BorrowedString) -> bool {
        let Some(path) = path.as_ref().filter(|path| !path.text.is_null()) else {
            return false;
        };
        let relevant = CStr::from_ptr(path.text.cast())
            .to_bytes()
            .rsplit(|&byte| byte == b'/')
            .next()
            == Some(self.kind.filename());
        if relevant {
            if let Ok(mut registries) = self.registries.lock() {
                registries.insert(cache as usize, None);
            }
        }
        relevant
    }

    pub unsafe fn finish_build(&self, cache: *mut BcsvCache) {
        let Ok(mut registries) = self.registries.lock() else {
            return;
        };
        let Some(native) = cache.as_mut() else { return };
        match self.extend_cache(native) {
            Ok(registry) => {
                registries.insert(cache as usize, Some(registry));
            }
            Err(message) => {
                if !self.warned.swap(true, Ordering::Relaxed) {
                    println!(
                        "[additive icons] {}: {}",
                        String::from_utf8_lossy(self.kind.filename()),
                        message
                    );
                }
            }
        }
    }

    pub fn current(&self) -> Option<Arc<IconRegistry>> {
        let registries = self.registries.lock().ok()?;
        let mut entries = registries.values();
        let registry = entries.next()?.as_ref()?;
        // Without a selected-cache global, every observed cache must agree.
        for entry in entries {
            if entry.as_ref()? != registry {
                return None;
            }
        }
        Some(Arc::clone(registry))
    }

    pub unsafe fn index_for(&self, item: *const ItemData) -> Option<u16> {
        let registry = self.current()?;
        if registry.indices_by_label_hash.is_empty() {
            return None;
        }
        let item = item.as_ref()?;
        let hash = BcsvView::read_native_u32(item.item_param_row, self.item_field_hash)?;
        registry.indices_by_label_hash.get(&hash).copied()
    }

    unsafe fn extend_cache(
        &self,
        cache: &mut BcsvCache,
    ) -> Result<Arc<IconRegistry>, &'static str> {
        let table = cache.extension_table()?;
        let label_offset = table
            .field(crc32(b"Label string32"), 32)
            .ok_or("missing Label descriptor")?;
        let model_offset = if self.kind == IconKind::Unit {
            Some(
                table
                    .field(crc32(b"ModelName string32"), 32)
                    .ok_or("missing ModelName descriptor")?,
            )
        } else {
            None
        };
        let (resource_hash, resource_width) = match self.kind {
            IconKind::Unit => (crc32(b"ResName string64"), 64),
            IconKind::Menu => (crc32(b"ResName string32"), 32),
        };
        let resource_offset = table
            .field(resource_hash, resource_width)
            .ok_or("missing ResName descriptor")?;
        let mut indices_by_label_hash = HashMap::new();
        let mut resource_names = Vec::new();
        let mut assignments = Vec::new();
        let mut labels = HashMap::new();
        let mut resources = HashSet::new();

        for raw_index in 0..table.count {
            if !cache.is_valid_row(raw_index) {
                continue;
            }
            let row = table.row(raw_index).ok_or("invalid native row position")?;
            let label = string_field(row, label_offset, 32).ok_or("invalid icon Label")?;
            let hash = crc32(label);
            let vanilla = cache.row_alias(raw_index)? != UNMAPPED_ALIAS;
            let (existing_label, existing_is_vanilla) =
                labels.entry(hash).or_insert((label, vanilla));
            if *existing_label != label {
                return Err("ambiguous icon Label CRC collision");
            }
            if *existing_is_vanilla != vanilla {
                return Err("custom Label would override an existing native icon");
            }
            if vanilla {
                continue;
            }

            if let Some(offset) = model_offset {
                string_field(row, offset, 32).ok_or("invalid custom ModelName")?;
            }
            let resource = string_field(row, resource_offset, resource_width)
                .ok_or("invalid custom ResName")?;
            if resource.len() > 31 {
                return Err("custom ResName exceeds the renderer's 31-byte limit");
            }
            let assignment = cache.plan_row(raw_index, row.as_ptr(), assignments.len())?;
            // Identical duplicate Labels use their first assigned icon index.
            indices_by_label_hash
                .entry(hash)
                .or_insert(assignment.alias);
            assignments.push(assignment);
            // MenuIcons are named UI requests; only UnitIcons use this preload list.
            if self.kind == IconKind::Unit && resources.insert(resource) {
                let mut name = resource.to_vec();
                name.push(0);
                resource_names.push(name);
            }
        }
        let registry = Arc::new(IconRegistry {
            indices_by_label_hash,
            resource_names,
        });
        // Validate everything before writing native arrays; publish count last.
        cache.publish_rows(&assignments);
        Ok(registry)
    }
}

const _: () = {
    assert!(std::mem::size_of::<ItemData>() == 0x18);
    assert!(std::mem::offset_of!(ItemData, item_param_row) == 0x10);
};
