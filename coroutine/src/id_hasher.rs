use std::hash::Hasher;

#[derive(Default)]
pub(crate) struct IdHasher {
    id: u64,
}

impl Hasher for IdHasher {
    fn write(&mut self, _bytes: &[u8]) {
        // TODO: Need to do something sensible
        panic!("Can only hash u64");
    }

    fn write_u64(&mut self, i: u64) {
        self.id = i
    }

    fn finish(&self) -> u64 {
        self.id
    }
}
