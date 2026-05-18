use pixbar::{Bar, Capability};

fn main() {
    for width in [7, 13, 25, 40] {
        println!("\n--- width = {width} ---");
        for pct in (0..=100).step_by(5) {
            let p1 = pct as f64 / 100.0;
            let p2 = (p1 + 0.10).min(1.0);
            println!(
                "{pct:3}% | {}",
                Bar::new(width).primary(p1).secondary(p2).capability(Capability::EighthBlock).render()
            );
        }
    }
}
