//! RQ6 §4.5.1 (M11) measurement runner — see `lib.rs`'s
//! `render_large_page_monolithic_naive`/`_optimized`/`render_widget_only_for_isr`
//! doc comments for what's actually being compared. Prints CSV to stdout:
//! `n_sections,monolithic_naive_us,monolithic_optimized_us,widget_only_us`.
//!
//! Each `n_sections` value runs 20 reps (RQ1 §8-style repetition, minus the
//! full paired-bootstrap statistical apparatus — this is a much smaller,
//! narrower measurement than RQ1's, so median-of-20 is reported instead of
//! a full CI; see roadmap/PHASES.md's honest scope note for this round).

fn median(mut xs: Vec<u128>) -> u128 {
    xs.sort_unstable();
    xs[xs.len() / 2]
}

fn main() {
    println!("n_sections,monolithic_naive_us,monolithic_optimized_us,widget_only_us");
    for n_sections in [0usize, 10, 100, 1_000, 10_000, 50_000] {
        let naive: Vec<u128> = (0..20)
            .map(|_| loom_fic::render_large_page_monolithic_naive(n_sections).1.as_micros())
            .collect();
        let optimized: Vec<u128> = (0..20)
            .map(|_| loom_fic::render_large_page_monolithic_optimized(n_sections).1.as_micros())
            .collect();
        let widget: Vec<u128> = (0..20)
            .map(|_| loom_fic::render_widget_only_for_isr(n_sections).1.as_micros())
            .collect();
        println!(
            "{},{},{},{}",
            n_sections,
            median(naive),
            median(optimized),
            median(widget)
        );
    }
}
