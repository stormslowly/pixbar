use pixbar::{Bar, Capability};
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    style::Print,
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
};
use std::io::{stdout, Write};

struct State {
    width: usize,
    p1: f64,
    p2: f64,
    cap: Capability,
}

impl State {
    fn draw(&self) -> std::io::Result<()> {
        let mut out = stdout();
        execute!(out, MoveTo(0, 0), Clear(ClearType::All))?;
        execute!(
            out,
            Print(format!(
                "width={}  primary={:.3}  secondary={:.3}  cap={:?}\r\n",
                self.width, self.p1, self.p2, self.cap
            ))
        )?;
        let bar = Bar::new(self.width)
            .primary(self.p1)
            .secondary(self.p2)
            .capability(self.cap)
            .render();
        execute!(out, Print("[ "), Print(bar), Print(" ]\r\n"))?;
        execute!(
            out,
            Print("← → adjust primary | Shift+← → adjust secondary | + - resize | t cycle cap | q quit\r\n")
        )?;
        out.flush()
    }
}

fn main() -> std::io::Result<()> {
    let mut state = State {
        width: 40,
        p1: 0.33,
        p2: 0.67,
        cap: Capability::EighthBlock,
    };

    enable_raw_mode()?;
    execute!(stdout(), Hide)?;

    let res = (|| -> std::io::Result<()> {
        loop {
            state.draw()?;
            if let Event::Key(k) = event::read()? {
                let shift = k.modifiers.contains(KeyModifiers::SHIFT);
                match (k.code, shift) {
                    (KeyCode::Char('q'), _) | (KeyCode::Esc, _) => break,
                    (KeyCode::Left,  false) => state.p1 = (state.p1 - 0.01).max(0.0),
                    (KeyCode::Right, false) => state.p1 = (state.p1 + 0.01).min(state.p2),
                    (KeyCode::Left,  true)  => state.p2 = (state.p2 - 0.01).max(state.p1),
                    (KeyCode::Right, true)  => state.p2 = (state.p2 + 0.01).min(1.0),
                    (KeyCode::Char('+'), _) | (KeyCode::Char('='), _) => {
                        state.width = (state.width + 1).min(120);
                    }
                    (KeyCode::Char('-'), _) => {
                        state.width = state.width.saturating_sub(1).max(1);
                    }
                    (KeyCode::Char('t'), _) => {
                        state.cap = match state.cap {
                            Capability::Ascii => Capability::EighthBlock,
                            Capability::EighthBlock => Capability::Ascii,
                        };
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    })();

    execute!(stdout(), Show)?;
    disable_raw_mode()?;
    res
}
