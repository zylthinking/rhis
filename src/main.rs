use rhis::{
    history::History,
    interface::Interface,
    settings::{Mode, Settings},
};

fn handle_addition(settings: &Settings) {
    let mut history = History::load::<false>();
    history.add(&settings.command, &settings.sid, &settings.dir, settings.exit_code);
}

fn handle_search(settings: &Settings) {
    use crossterm::terminal;
    let (width, height) = terminal::size().unwrap();
    if width < 14 {
        return;
    }

    let mut history = History::load::<true>();
    let mut ui = Interface::new(settings, &mut history, width, height);
    let Some(cmd) = ui.display() else { return; };

    for byte in cmd.as_bytes() {
        if unsafe { libc::ioctl(0, libc::TIOCSTI, byte) } < 0 {
            break;
        }
    }
}

fn main() {
    let settings = Settings::parse_args();
    match settings.mode {
        Mode::Add => {
            handle_addition(&settings);
        }
        Mode::Search => {
            handle_search(&settings);
        }
        Mode::Init => {
            let mut s: String = "".into();
            let mut script = include_str!("../rhis.bash");
            if !settings.bottom {
                let offset = script.find("--bottom").unwrap();
                s = script.into();
                s.replace_range(offset..offset + 9, "");
            }

            if !settings.lightmode {
                let offset = script.find("--light").unwrap();
                if s.is_empty() {
                    s = script.into();
                }
                s.replace_range(offset..offset + 8, "");
                script = s.as_str();
            }
            print!("{}", script);
        }
    }
}
