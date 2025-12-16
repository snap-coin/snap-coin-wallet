use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use std::io::{self, Write};

pub fn read_pin(prompt: &str) -> Result<String, std::io::Error> {
    print!("{}", prompt);
    io::stdout().flush()?; // show prompt immediately

    enable_raw_mode()?; // start raw mode
    let mut pin = String::new();

    while pin.len() < 6 {
        if let Event::Key(key_event) = event::read()? {
            if key_event.kind != KeyEventKind::Press {
                continue;
            }
            match key_event.code {
                KeyCode::Char(c) if c.is_ascii_digit() => {
                    pin.push(c);
                    print!("*");
                    io::stdout().flush()?;
                }
                KeyCode::Backspace => {
                    if !pin.is_empty() {
                        pin.pop();
                        print!("\x08 \x08"); // remove last *
                        io::stdout().flush()?;
                    }
                }
                _ => {} // ignore everything else
            }
        }
    }

    disable_raw_mode()?; // exit raw mode
    println!(); // move to new line
    Ok(pin)
}

pub fn read_input(prompt: &str) -> String {
    print!("{}", prompt);
    io::stdout().flush().unwrap();
    let mut buf = String::new();
    io::stdin().read_line(&mut buf).unwrap();
    buf.trim().to_string()
}