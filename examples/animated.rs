use pixbar::{Bar, Capability};
use std::{thread::sleep, time::Duration};

fn main() {
    let cap = Capability::EighthBlock;
    print!("\x1b[?25l"); // hide cursor
    for tick in 0..=100u32 {
        let p1 = tick as f64 / 100.0;
        let p2 = (p1 + 0.12).min(1.0);
        print!("\r{}", Bar::new(40).primary(p1).secondary(p2).capability(cap).render());
        std::io::Write::flush(&mut std::io::stdout()).ok();
        sleep(Duration::from_millis(40));
    }
    println!("\x1b[?25h");
}
