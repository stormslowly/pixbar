use almost_perfect_progressbar::{Bar, Capability};

fn main() {
    for cap in [Capability::Ascii, Capability::EighthBlock, Capability::PatchedSixteenth] {
        println!("{:>20?}  {}", cap, Bar::new(40).primary(0.33).secondary(0.67).capability(cap).render());
    }
}
