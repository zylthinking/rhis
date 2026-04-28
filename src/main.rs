use rhis::{
    conf,
    db,
    interface::Interface,
    settings::{Mode, Settings},
};

async fn handle_addition(settings: &Settings) {
    db::save_command(
        &settings.command,
        settings.exit_code,
    )
    .await;
}

fn handle_search(settings: &Settings) {
    use crossterm::terminal;
    let (width, height) = terminal::size().unwrap();
    if width < 14 {
        return;
    }

    let mut ui = Interface::new(settings, width, height);
    let Some(cmd) = ui.display() else { return };

    for byte in cmd.as_bytes() {
        if unsafe { libc::ioctl(0, libc::TIOCSTI, byte) } < 0 {
            break;
        }
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    rhis::set_runtime(tokio::runtime::Handle::current());
    let settings = Settings::parse_args();
    let config_path = settings
        .config_path
        .clone()
        .unwrap_or_else(|| shellexpand::tilde("~/.local/share/rhis/config.toml").into_owned());
    conf::conf_init(&config_path);

    db::warmup();

    match settings.mode {
        Mode::Add => {
            handle_addition(&settings).await;
        }
        Mode::Search => {
            tokio::task::block_in_place(|| {
                handle_search(&settings);
            });
        }
        Mode::Init => {
            let mut s: String = "".into();
            let script = include_str!("../rhis.bash");
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
            }

            if s.is_empty() {
                print!("{}", script);
            } else {
                print!("{}", s);
            }
        }
    }
}
