//! The `pagefile` command: looks inside a page file.
//!
//! ```text
//! pagefile header FILE      the header in page 0
//! pagefile pages FILE       one line for every page
//! pagefile page FILE N      everything in page N
//! pagefile check FILE       read every page and report problems
//! ```

use std::{env, path::Path, process::ExitCode};

use pagefile::{
    Header, Page, PageFile,
    PageKind::{self},
    PageNo, PageStore, Result, Slotted,
    check::check,
    freelist::FREE_NEXT,
    overflow::{DATA, NEXT, USED, capacity},
    page::CHECKSUM_LEN,
};
use vfs::Disk;

const USAGE: &str = "usage: pagefile header FILE | pages FILE | page FILE N | check FILE";

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match words.as_slice() {
        ["header", path] => open(path).map(|f| print_header(f.header())),
        ["pages", path] => open(path).and_then(|f| print_pages(&f)),
        ["page", path, n] => match n.parse::<u32>() {
            Ok(n) => open(path).and_then(|f| print_page(&f, n)),
            Err(_) => {
                eprintln!("pagefile: {n} is not a page number");
                return ExitCode::FAILURE;
            }
        },
        ["check", path] => open(path).and_then(|f| print_check(&f)),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("pagefile: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Opens the page file at `path` on the real disk, with the file's own directory
/// as the root of the file system.
fn open(path: &str) -> Result<PageFile> {
    let path = Path::new(path);
    let dir = path.parent().unwrap_or(Path::new(""));
    let name = path.file_name().map_or(path, Path::new);
    PageFile::open(&Disk::new(dir), name)
}

/// Prints the header, one field per line. Returns true: there's nothing to fail.
fn print_header(h: &Header) -> bool {
    println!("page size      {}", h.page_size);
    println!("page count     {}", h.page_count);
    println!("freelist head  {}", h.free_head.map_or(0, PageNo::get));
    println!("freelist count {}", h.free_count);
    let user: String = h.user.iter().map(|b| format!("{b:02x}")).collect();
    println!("user bytes     {user}");
    true
}

/// Prints one line for every page: its number, its kind and a summary.
fn print_pages(file: &PageFile) -> Result<bool> {
    println!("{:>5}  header", 0);
    let mut page = file.new_page();
    for number in 1..file.page_count() {
        let n = PageNo::new(number).expect("the loop starts at page 1");
        match file.read(n, &mut page) {
            Ok(()) => println!("{number:>5}  {}", summary(&page)),
            Err(e) => println!("{number:>5}  damaged: {e}"),
        }
    }
    Ok(true)
}

/// One line about a page that has already been read and checked.
fn summary(page: &Page) -> String {
    match page.kind() {
        Some(PageKind::Slotted) => match Slotted::open(page) {
            Ok(s) => {
                let items = if s.len() == 1 { "item" } else { "items" };
                format!("slotted   {} {items}, {} bytes of room", s.len(), s.room())
            }
            Err(e) => format!("slotted   {e}"),
        },
        Some(PageKind::Free) => format!("free      next {}", page.u32_at(FREE_NEXT)),
        Some(PageKind::Overflow) => {
            format!(
                "overflow  next {}, {} bytes",
                page.u32_at(NEXT),
                page.u16_at(USED)
            )
        }
        Some(PageKind::Empty) => "empty".to_string(),
        None => format!("unknown kind byte {}", page.bytes()[0]),
    }
}

/// Prints everything in page `n`: the header for page 0, the slots and items
/// for a slotted page, and the fields of the other kinds.
fn print_page(file: &PageFile, n: u32) -> Result<bool> {
    let Some(n) = PageNo::new(n) else {
        return Ok(print_header(file.header()));
    };
    let mut page = file.new_page();
    file.read(n, &mut page)?;
    println!("page {n}: {}", summary(&page));
    if page.kind() == Some(PageKind::Slotted) {
        let s = Slotted::open(&page)?;
        println!(
            "  tag {}, items start at {}, {} bytes of garbage",
            s.tag(),
            s.content_start(),
            s.garbage()
        );
        println!("  slot  offset  len  item");
        for i in 0..s.len() {
            let (offset, len) = s.slot(i);
            let item = s.get(i).expect("i is below len");
            println!("  {i:>4}  {offset:>6}  {len:>3}  {}", show(item));
        }
    }
    if page.kind() == Some(PageKind::Overflow) {
        let used = (page.u16_at(USED) as usize).min(capacity(page.size()));
        println!("  data  {}", show(&page.bytes()[DATA..DATA + used]));
    }
    println!("  checksum {:08x}", page.u32_at(page.size() - CHECKSUM_LEN));
    Ok(true)
}

/// Shows bytes as quoted text when they're all printable, and as hex
/// otherwise. Cut to 40 bytes.
fn show(bytes: &[u8]) -> String {
    let cut = bytes.len().min(40);
    let more = if bytes.len() > cut { " ..." } else { "" };
    let bytes = &bytes[..cut];
    if bytes.iter().all(|b| b.is_ascii_graphic() || *b == b' ') {
        format!("\"{}\"{more}", bytes.escape_ascii())
    } else {
        let hex: Vec<String> = bytes.iter().map(|b| format!("{b:02x}")).collect();
        format!("{}{more}", hex.join(" "))
    }
}

/// Prints the page counts and every problem `check` found.
/// Returns false is there were any.
fn print_check(file: &PageFile) -> Result<bool> {
    let report = check(file)?;
    let kinds: Vec<String> = report
        .kinds
        .iter()
        .map(|(k, n)| format!("{n} {k}"))
        .collect();
    println!("pages: {}", kinds.join(", "));
    for problem in &report.problems {
        println!("problem: {problem}");
    }
    if report.problems.is_empty() {
        println!("ok");
    }
    Ok(report.problems.is_empty())
}
