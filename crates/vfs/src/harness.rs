//! `crash_test`: crashes a workload at every point and checks what's left.
//!
//! The workload cuts the power after its first change, then after its second,
//! and so on, and each cut is crashed with many seeds. As it runs, the
//! workload records what it was told is safe (its acknowledgements) in a
//! value of type `A`. The check gets that value with the crashed file system,
//! so it can tell data that was promised and lost from data that was never
//! promised.

use std::fmt;

use crate::{Error, Mem, Result};

/// One crash after which the check failed.
#[derive(Debug)]
pub struct Failure {
    /// How many changes the workload made before the power went off.
    pub changes: u64,
    /// The seed the crash was chosen with.
    pub seed: u64,
    /// What the check said was wrong.
    pub message: String,
}

/// The result of a `crash_test`.
#[derive(Debug)]
pub struct Report {
    /// How many crashes were checked.
    pub runs: u64,
    /// The crashes after which the check failed.
    pub failures: Vec<Failure>,
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} crashes, {} failed", self.runs, self.failures.len())?;
        if let Some(first) = self.failures.first() {
            write!(
                f,
                "\nfirst: power off after {} changes, seed {}: {}",
                first.changes, first.seed, first.message
            )?;
        }
        Ok(())
    }
}

/// Crash-tests `workload`. First it runs once with no crash, to count its
/// changes, and `check` must pass. Then, for each `n` from 0 to that count
/// and each seed below `seeds`: run on a fresh `Mem` that loses power after
/// `n` changes, crash it with the seed, and run `check`.
pub fn crash_test<A: Default>(
    seeds: u64,
    workload: impl Fn(&Mem, &mut A) -> Result<()>,
    check: impl Fn(&Mem, &A) -> std::result::Result<(), String>,
) -> Report {
    let full = Mem::new();
    let mut acks = A::default();
    if let Err(e) = workload(&full, &mut acks) {
        panic!("the workload fails with no crash at all: {e}");
    }
    if let Err(message) = check(&full, &acks) {
        panic!("the check fails with no crash at all: {message}");
    }
    let total = full.changes();

    let mut report = Report {
        runs: 0,
        failures: Vec::new(),
    };
    for changes in 0..=total {
        for seed in 0..seeds {
            let mem = Mem::new();
            mem.power_off_after(changes);
            let mut acks = A::default();
            let result = match workload(&mem, &mut acks) {
                // The workload stopped at the power cut, as it should.
                Ok(()) | Err(Error::PowerOff) => {
                    mem.crash(seed);
                    check(&mem, &acks)
                }
                Err(e) => Err(format!(
                    "the workload failed with something other than the power cut: {e}"
                )),
            };
            report.runs += 1;
            if let Err(message) = result {
                report.failures.push(Failure {
                    changes,
                    seed,
                    message,
                });
            }
        }
    }
    report
}
