//! Crash-tests a page file. The workload creates a file, writes three
//! pages, frees one and syncs. After every crash `vfs::crash_test` makes, the
//! file must still open, and `check` must find nothing wrong.
//!
//!     cargo run --example crashes

use pagefile::check::check;
use pagefile::{PageFile, PageStore, Slotted, freelist};
use vfs::{Mem, crash_test};

/// Turns a `pagefile` result into a `vfs` one, which `crash_test` expects.
/// Only file errors are expected here; anything else is a bug in the workload.
fn file_op<T>(result: pagefile::Result<T>) -> vfs::Result<T> {
    result.map_err(|e| match e {
        pagefile::Error::Vfs(e) => e,
        other => panic!("not a file error: {other}"),
    })
}

/// Creates `p.pf`, writes three slotted pages, frees the second, and syncs.
/// `created` is set once `create` returns, because from then on the file
/// is promised to exist.
fn workload(mem: &Mem, created: &mut bool) -> vfs::Result<()> {
    let mut file = file_op(PageFile::create(mem, "p.pf", 512))?;
    *created = true;
    let mut page = file.new_page();
    let mut pages = Vec::new();
    for item in [&b"one"[..], b"two", b"three"] {
        let n = file_op(freelist::allocate(&mut file))?;
        file_op(Slotted::init(&mut page, 0).insert(0, item))?;
        file_op(file.write(n, &mut page))?;
        pages.push(n);
    }
    file_op(freelist::free(&mut file, pages[1]))?;
    file_op(file.sync())
}

/// The file must open once it was created.
fn opens(mem: &Mem, created: &bool) -> Result<(), String> {
    if *created {
        PageFile::open(mem, "p.pf").map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// The file must open, and `check` must find nothing wrong.
fn healthy(mem: &Mem, created: &bool) -> Result<(), String> {
    if !*created {
        return Ok(());
    }
    let file = PageFile::open(mem, "p.pf").map_err(|e| e.to_string())?;
    let report = check(&file).map_err(|e| e.to_string())?;
    match report.problems.first() {
        Some(problem) => Err(problem.clone()),
        None => Ok(()),
    }
}

fn main() {
    println!("it opens:            {}", crash_test(20, workload, opens));
    println!("it opens and checks: {}", crash_test(20, workload, healthy));
}
