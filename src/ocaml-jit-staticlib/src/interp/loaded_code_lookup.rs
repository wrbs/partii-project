pub struct LoadedCodeLookup {
    entries: Vec<Entry>,
}

struct Entry {
    base_ptr: usize,
    size: usize,
    base_mapped: usize,
    lookup: Vec<Option<(usize, usize)>>,
}

impl LoadedCodeLookup {
    pub fn new() -> LoadedCodeLookup {
        LoadedCodeLookup {
            entries: Vec::new(),
        }
    }

    pub fn add_entry(
        &mut self,
        code: &[i32],
        base_mapped: usize,
        lookup: Vec<Option<(usize, usize)>>,
    ) {
        self.entries.push(Entry {
            base_ptr: code.as_ptr() as usize,
            size: code.len(),
            base_mapped,
            lookup,
        })
    }

    pub fn lookup(&self, ptr: *const i32) -> Option<(usize, usize)> {
        let ptr = ptr as usize;

        for entry in self.entries.iter() {
            if entry.base_ptr <= ptr && ptr < entry.base_ptr + entry.size * 4 {
                // Hit
                return entry
                    .lookup
                    .get((ptr - entry.base_ptr) / 4)
                    .and_then(|x| *x)
                    .map(|(start, num_instructions)| {
                        (start + entry.base_mapped, num_instructions)
                    });
            }
        }

        None
    }
}
