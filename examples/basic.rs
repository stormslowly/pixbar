use pixbar::{Bar, Capability};

fn main() {
    for cap in [Capability::Ascii, Capability::EighthBlock] {
        println!("{:>14?}  {}", cap, Bar::new(40).primary(0.33).secondary(0.67).capability(cap).render());
    }
}
