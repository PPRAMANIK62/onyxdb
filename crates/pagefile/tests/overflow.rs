//! Overflow chains: round trips, freeing, and chains that lie about their length.

use pagefile::overflow::{capacity, free_chain, read_chain, write_chain};
use pagefile::{Error, PageFile};
use vfs::{Mem, Rng};

#[test]
fn values_come_back_unchanged() {
    let mut rng = Rng::new(9);
    let mut file = PageFile::create(&Mem::new(), "chain.pf", 512).unwrap();
    let cap = capacity(512);
    // Lengths around the page boundaries are where off-by-one mistakes hide.
    for len in [1, cap - 1, cap, cap + 1, 2 * cap, 2 * cap + 1, 10_000] {
        let data = rng.bytes(len);
        let first = write_chain(&mut file, &data).unwrap();
        assert_eq!(read_chain(&file, first, len).unwrap(), data, "length {len}");
    }
}

#[test]
fn freeing_a_chain_returns_every_page() {
    let mut file = PageFile::create(&Mem::new(), "chain.pf", 512).unwrap();
    let first = write_chain(&mut file, &[7; 1200]).unwrap();
    assert_eq!(file.page_count(), 4); // the header and three overflow pages
    free_chain(&mut file, first).unwrap();
    assert_eq!(file.header().free_count, 3);
    // Writing the same value again reuses the three pages instead of growing the file.
    write_chain(&mut file, &[8; 1200]).unwrap();
    assert_eq!(file.page_count(), 4);
}

#[test]
fn a_wrong_length_is_caught() {
    let mut file = PageFile::create(&Mem::new(), "chain.pf", 512).unwrap();
    let first = write_chain(&mut file, &[1; 1200]).unwrap();
    assert!(matches!(
        read_chain(&file, first, 1300),
        Err(Error::BadChain(_))
    ));
    assert!(matches!(
        read_chain(&file, first, 1100),
        Err(Error::BadChain(_))
    ));
}
