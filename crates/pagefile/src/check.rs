//! Checking a whole file: every page readable, every slotted page consistent,
//! and the freelist holding exactly the free pages.
//!
//! It can't tell whether a slotted page or an overflow chain is still in use.
//! Only the code that stores data in the pages knows which pages it still
//! points to.

use std::collections::{BTreeMap, BTreeSet};

use crate::{Error, PageFile, PageKind, PageNo, PageStore, Result, Slotted, overflow::NEXT};

/// What `check` found.
#[derive(Debug, Default)]
pub struct Report {
    /// How many pages there are of each kind, by name. Page 0 counts as "header".
    pub kinds: BTreeMap<&'static str, u32>,
    /// Everything wrong with the file, one sentence each. Empty for a healthy file.
    pub problems: Vec<String>,
}

/// Reads every page of `file` and walks its freelist.
///
/// Damaged pages are reported, not returned as errors. Only an I/O error
/// stops the chek.
pub fn check(file: &PageFile) -> Result<Report> {
    let mut report = Report::default();
    report.kinds.insert("header", 1);
    let mut page = file.new_page();
    let mut free_pages = BTreeSet::new();

    for number in 1..file.page_count() {
        let n = PageNo::new(number).expect("the loop starts at page 1");
        match file.read(n, &mut page) {
            Ok(()) => {}
            Err(Error::Vfs(e)) => return Err(Error::Vfs(e)),
            Err(e) => {
                report.problems.push(e.to_string());
                *report.kinds.entry("damaged").or_default() += 1;
                continue;
            }
        }
        let name = match page.kind() {
            Some(PageKind::Slotted) => {
                if let Err(e) = Slotted::open(&page) {
                    report.problems.push(format!("page {n}: {e}"));
                }
                "slotted"
            }
            Some(PageKind::Free) => {
                free_pages.insert(number);
                "free"
            }
            Some(kind) => kind.name(),
            None => {
                report.problems.push(format!(
                    "page {n} has unknown kind byte {}",
                    page.bytes()[0]
                ));
                "unknown"
            }
        };
        *report.kinds.entry(name).or_default() += 1;
    }

    // Walk the freelist. Each step must reach a free page not reached before.
    let mut on_list = BTreeSet::new();
    let mut next = file.header().free_head;
    while let Some(n) = next {
        if !free_pages.contains(&n.get()) {
            report.problems.push(format!(
                "the freelist reaches page {n}, which isn't a free page"
            ));
            break;
        }
        if !on_list.insert(n.get()) {
            report
                .problems
                .push(format!("the freelist loops back to page {n}"));
            break;
        }
        file.read(n, &mut page)?;
        next = PageNo::new(page.u32_at(NEXT));
    }
    let count = file.header().free_count;
    if on_list.len() as u32 != count {
        report.problems.push(format!(
            "the header counts {count} free pages, but the freelist holds {}",
            on_list.len()
        ));
    }
    for n in free_pages.difference(&on_list) {
        report
            .problems
            .push(format!("page {n} is free but not on the freelist"));
    }

    let expected = file.page_count() as u64 * file.page_size() as u64;
    let len = file.file_len()?;
    if len < expected {
        report.problems.push(format!(
            "the file has {} bytes after its last page",
            len - expected
        ));
    }
    Ok(report)
}
