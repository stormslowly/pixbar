use pixbar::{Bar, Capability};

fn main() {
    let p1 = 0.33;
    let p2 = 0.67;
    for width in [13, 25, 50, 80] {
        println!("\nwidth = {width}");
        for cap in [Capability::Ascii, Capability::EighthBlock] {
            println!(
                "  {:>14?}  {}",
                cap,
                Bar::new(width).primary(p1).secondary(p2).capability(cap).render()
            );
        }
    }
}
