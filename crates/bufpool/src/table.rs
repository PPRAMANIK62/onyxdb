//! `PageTable`: a hash table from page number to a small value, such as the
//! frame that holds the page.
//!
//! Every entry lives in one array of slots. A key's hash picks its home slot.
//! If that slot is taken, the key goes in the next free slot after it, so a
//! search starts at the home slot and walks forward until it finds the key or an
//! empty slot. This is `open addressing` with `linear probing`.
//!
//! The table never grows. It has twice as many slots as the most entries it
//! will hold (load factor 0.5), so it's never more than half full, and the runs
//! of taken slots stay short.

use pagefile::PageNo;

/// A hash table from page number to a value of type `V`, with a fixed number of
/// slots.
pub struct PageTable<V> {
    /// Each slot is empty or holds one entry.
    slots: Vec<Option<(PageNo, V)>>,
    /// The number of slots is 2 to the power of `bits`.
    bits: u32,
    /// The most entries the table may hold: half the slots or fewer.
    capacity: usize,
    /// The number of entries.
    len: usize,
}

/// 2^64 divided by the golden ratio, rounded to an odd number. Multiplying a page
/// number by it spreads neighbouring numbers far apart in the top bits.
/// Knuth calls this Fibonacci hashing.
const GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;

impl<V: Copy> PageTable<V> {
    /// A table for up to `capacity` entries. It has twice that many slots, rounded up
    /// to a power of two.
    pub fn new(capacity: usize) -> PageTable<V> {
        let slots = (2 * capacity).max(2).next_power_of_two();
        PageTable {
            slots: vec![None; slots],
            bits: slots.trailing_zeros(),
            capacity,
            len: 0,
        }
    }

    /// The most entries the table may hold.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// The number of entries.
    pub fn len(&self) -> usize {
        self.len
    }

    /// True if the table has no entries.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The number of slots.
    pub fn slots(&self) -> usize {
        self.slots.len()
    }

    /// The entry in slot `i`, if there is one. For tests and tools that show
    /// how the table is laid out.
    pub fn slot(&self, i: usize) -> Option<(PageNo, V)> {
        self.slots[i]
    }

    /// The home slot of `key`: the top `bits` bits of the key times `GOLDEN`.
    pub fn home(&self, key: PageNo) -> usize {
        ((key.get() as u64).wrapping_mul(GOLDEN) >> (64 - self.bits)) as usize
    }

    /// The slot after slot `i`, wrapping around from the last to the first.
    fn next(&self, i: usize) -> usize {
        (i + 1) & (self.slots.len() - 1)
    }

    /// How many steps forward it is from slot `from` to slot `to`, wrapping around.
    fn distance(&self, from: usize, to: usize) -> usize {
        to.wrapping_sub(from) & (self.slots.len() - 1)
    }

    /// The slot that holds `key`, or the empty slot where its search ends.
    /// There's always an empty slot, because the table is never more than half full.
    fn find(&self, key: PageNo) -> usize {
        let mut i = self.home(key);
        while let Some((k, _)) = self.slots[i] {
            if k == key {
                break;
            }
            i = self.next(i);
        }
        i
    }

    /// The value for `key`, if the table has it.
    pub fn get(&self, key: PageNo) -> Option<V> {
        self.slots[self.find(key)].map(|(_, v)| v)
    }

    /// Sets the value for `key` and returns the old one, if there was one.
    /// Panics if the key is new and the table already holds `capacity` entries.
    pub fn insert(&mut self, key: PageNo, value: V) -> Option<V> {
        let i = self.find(key);
        let old = self.slots[i].map(|(_, v)| v);
        if old.is_none() {
            assert!(
                self.len < self.capacity,
                "the table is full: it holds {} entries",
                self.capacity
            );
            self.len += 1;
        }
        self.slots[i] = Some((key, value));
        old
    }

    /// Every entry, in slot order.
    // `impl Iterator<Item = (PageNo, V)> + '_` means "some iterator that yields (PageNo, V) pairs".
    // `+ '_` means the iterator borrows self. It reads straight from self.slots, so it can't outlive the table. While you hold it, you can't change the table.
    pub fn iter(&self) -> impl Iterator<Item = (PageNo, V)> + '_ {
        self.slots.iter().flatten().copied()
    }

    /// Removes `key` and returns its value, if the table had it.
    pub fn remove(&mut self, key: PageNo) -> Option<V> {
        let mut hole = self.find(key);
        let (_, value) = self.slots[hole].take()?;
        self.len -= 1;
        // Emptying the slot could cut a run in two, and a search that starts
        // before the hole would stop there and miss the keys after it. So walk
        // the rest of the run, and move back each entry whose search passes the
        // hole: one whose home is at the hole or before it.
        let mut i = self.next(hole);
        while let Some((k, _)) = self.slots[i] {
            if self.distance(self.home(k), i) >= self.distance(hole, i) {
                self.slots[hole] = self.slots[i].take();
                hole = i
            }
            i = self.next(i);
        }
        Some(value)
    }
}
