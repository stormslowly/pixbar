use almost_perfect_progressbar::{Bar, Capability};

fn main() {
    let p1 = 0.33;
    let p2 = 0.67;
    for width in [7, 13, 25, 50] {
        println!("\nwidth = {width}");
        for cap in [Capability::Ascii, Capability::EighthBlock, Capability::PatchedSixteenth] {
            println!(
                "  {:>18?}  {}",
                cap,
                Bar::new(width).primary(p1).secondary(p2).capability(cap).render()
            );
        }
    }
}
