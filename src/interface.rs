use crate::{
    command_input::{CommandInput, Move},
    fixed_length_grapheme_string::FixedLengthGraphemeString,
    history::{Command, History},
    history_cleaner,
    settings::Settings,
};
use chrono::{Duration, TimeZone, Utc};
use crossterm::{
    cursor,
    event::{read, Event, KeyCode, KeyCode::Char, KeyEvent, KeyModifiers},
    execute, queue,
    style::{Color, Print, SetBackgroundColor, SetForegroundColor},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use humantime::format_duration;
use std::{
    cmp,
    io::{stdout, Write},
    string::String,
};

pub struct Interface<'a> {
    history: &'a mut History,
    settings: &'a Settings,
    input: CommandInput,
    selection: usize,
    offset: usize,
    matches: Vec<Command>,
    menu_mode: MenuMode,
    rank: bool,
    anywhere: bool,
    width: u16,
    height: u16,
}

pub enum MoveSelection {
    Up,
    Down,
}

#[derive(PartialEq, Eq, Copy, Clone)]
pub enum MenuMode {
    Normal,
    ConfirmDelete,
}

impl MenuMode {
    fn text(&self, interface: &Interface) -> String {
        let mut menu_text = String::from("rhis");

        if *self == MenuMode::ConfirmDelete {
            return String::from("Delete selected command from the history? (Y/N)");
        }

        menu_text.push_str(" | ⏎ - Run | TAB - Edit | ");
        match interface.rank {
            true => menu_text.push_str("F1 - Rank Sort | "),
            _ => menu_text.push_str("F1 - Time Sort | "),
        }

        menu_text.push_str("F2 - Delete | ");
        match interface.anywhere {
            true => menu_text.push_str("F3 - All Directories"),
            _ => menu_text.push_str("F3 - This Directory"),
        }

        menu_text
    }

    fn bg(&self) -> Color {
        match *self {
            MenuMode::Normal => Color::Blue,
            MenuMode::ConfirmDelete => Color::Red,
        }
    }
}

impl<'a> Interface<'a> {
    pub fn new(settings: &'a Settings, history: &'a mut History, w: u16, h: u16) -> Interface<'a> {
        Interface {
            history,
            settings,
            input: CommandInput::from(settings.command.to_owned(), 2 * w - 4),
            selection: 0,
            offset: 0,
            matches: Vec::new(),
            menu_mode: MenuMode::Normal,
            rank: true,
            anywhere: true,
            width: w,
            height: h,
        }
    }

    pub fn display(&mut self) -> Option<&String> {
        self.build_cache_table();
        self.select();

        let command = &self.input.command;
        if command.chars().any(|c| !c.is_whitespace()) {
            self.history.record_selected_from_ui(command, &self.settings.sid, &self.settings.dir);
            Some(&command)
        } else {
            None
        }
    }

    fn build_cache_table(&self) { self.history.build_cache_table(&self.settings.dir, self.anywhere); }

    fn menubar<W: Write>(&self, screen: &mut W, width: u16, height: u16) {
        let indx = self.line_range::<1>(height);
        if indx.0 == -1 {
            return;
        }

        let width = width as usize - 1;
        let mut text = self.menu_mode.text(self);
        if text.len() > width {
            text.truncate(width - 3);
            text.push_str("...");
        }

        queue!(
            screen,
            SetBackgroundColor(self.menu_mode.bg()),
            SetForegroundColor(Color::White),
            cursor::MoveTo(1, indx.0 as u16),
            Print(format!("{text:width$}", width = width - 1)),
            SetBackgroundColor(Color::Reset),
            SetForegroundColor(Color::Reset)
        )
        .unwrap();
    }

    fn trans_position(width: u16, mut pos: (u16, u16)) -> (u16, u16) {
        let lines = pos.0 / width;
        pos.0 -= width * lines;
        pos.1 += lines;
        pos
    }

    fn prompt<const B: bool, W: Write>(&mut self, screen: &mut W, width: u16, height: u16) {
        let indx = self.line_range::<3>(height);
        if indx.0 == -1 {
            return;
        }

        let fg = if self.settings.lightmode {
            Color::Black
        } else {
            Color::White
        };

        let cmd = self.input.command.as_str();
        if B {
            queue!(screen, cursor::MoveTo(1, indx.0 as u16), SetForegroundColor(fg), Print(format!("$ {}", cmd)),)
                .unwrap();
        }
        let mut pos = (self.input.cursor as u16 + 3, indx.0 as u16);
        pos = Self::trans_position(width, pos);
        queue!(screen, cursor::MoveTo(pos.0, pos.1),).unwrap();
    }

    fn candidate_theme(light: bool, hi: bool) -> (Color, Color, Color, Color) {
        if light {
            if hi {
                (Color::DarkGrey, Color::White, Color::Grey, Color::DarkBlue)
            } else {
                (Color::Reset, Color::Black, Color::DarkBlue, Color::Blue)
            }
        } else {
            if hi {
                (Color::White, Color::Black, Color::DarkGreen, Color::DarkBlue)
            } else {
                (Color::Reset, Color::White, Color::DarkGreen, Color::Blue)
            }
        }
    }

    fn explain(tms: i64) -> String {
        format_duration(
            Duration::minutes(Utc::now().signed_duration_since(Utc.timestamp_opt(tms, 0).unwrap()).num_minutes())
                .to_std()
                .unwrap(),
        )
        .to_string()
        .split(' ')
        .take(2)
        .map(|s| {
            s.replace("years", "y")
                .replace("year", "y")
                .replace("months", "mo")
                .replace("month", "mo")
                .replace("days", "d")
                .replace("day", "d")
                .replace("hours", "h")
                .replace("hour", "h")
                .replace("minutes", "m")
                .replace("minute", "m")
                .replace("0s", "< 1m")
        })
        .collect::<Vec<String>>()
        .join(" ")
    }

    fn results<W: Write>(&mut self, screen: &mut W, mut idx: i32, width: u16, height: u16, resized: bool) {
        let area = self.line_range::<5>(height);
        let (min, max) = (cmp::min(area.0, area.1), cmp::max(area.0, area.1));
        if min == -1 {
            return;
        }

        let rows = (max - min) as usize;
        let (mut top, mut bottom) = (self.offset, self.offset + rows);
        if resized {
            if self.selection > bottom {
                self.offset = self.selection - rows;
            } else if bottom > self.selection {
                self.offset -= cmp::min(self.offset, bottom - self.selection);
            }
        } else {
            if self.selection == bottom + 1 {
                self.offset += 1;
                idx = -1;
            } else if top == self.selection + 1 {
                self.offset = self.selection;
                idx = -1;
            }
        }
        (top, bottom) = (self.offset, self.offset + rows);

        let input = &self.input.command;
        for index in 0..self.matches.len() {
            if index < top || index > bottom {
                continue;
            }

            if idx != -1 && idx != index as i32 && index != self.selection {
                continue;
            }

            let line = self.command_line_index((index - self.offset) as i16) + area.0;
            let command = &self.matches[index];
            let since = &Self::explain(command.last_run);
            let (bg, fg, hi, when) = Self::candidate_theme(self.settings.lightmode, index == self.selection);
            let colored = Interface::format_command_text(command, input, width, hi, fg);
            queue!(
                screen,
                cursor::MoveTo(1, line as u16),
                SetBackgroundColor(bg),
                Print(format!("{w: <w$}", w = width as usize - 10)),
                cursor::MoveTo(1, line as u16),
                SetForegroundColor(when),
                Print(format!("{index: >2} ")),
                SetForegroundColor(fg),
                Print(colored),
                cursor::MoveTo(width - 10, line as u16),
                SetForegroundColor(when),
                Print(format!("{:>9}", since)),
                SetForegroundColor(Color::Reset),
                SetBackgroundColor(Color::Reset),
            )
            .unwrap();
        }
    }

    #[allow(unused)]
    fn debug<W: Write, S: Into<String>>(&self, screen: &mut W, s: S) {
        queue!(screen, cursor::MoveTo(0, 0), Clear(ClearType::CurrentLine), Print(s.into())).unwrap();
        screen.flush().unwrap();
    }

    fn move_selection(&mut self, direction: MoveSelection) {
        let n1 = if self.settings.bottom { -1 } else { 1 };
        let n2 = match direction {
            MoveSelection::Up => -1,
            _ => 1,
        };

        let pos = self.selection as isize + n1 * n2;
        let pos = cmp::min(self.matches.len() as isize - 1, pos);
        self.selection = cmp::max(0, pos) as usize;
    }

    fn accept_selection(&mut self, run: bool) {
        if !self.matches.is_empty() {
            self.input.set(&self.matches[self.selection].cmd);
        }

        if self.input.command.is_empty() {
            return;
        }

        if run {
            self.input.command.push_str("\n");
        } else {
            self.input.command.push_str("\t");
        }
    }

    fn confirm(&mut self, confirmation: bool) {
        if confirmation {
            if let MenuMode::ConfirmDelete = self.menu_mode {
                self.delete_selection()
            }
        }
        self.menu_mode = MenuMode::Normal;
    }

    fn delete_selection(&mut self) {
        if !self.matches.is_empty() {
            let command = &self.matches[self.selection];
            history_cleaner::clean(self.history, &command.cmd);
            self.refresh_matches(false);
        }
    }

    fn refresh_matches(&mut self, reset_selection: bool) {
        if reset_selection {
            self.selection = 0;
            self.offset = 0;
        }
        self.matches = self.history.find_matches(&self.input.command, self.settings.results as i16, self.rank);
    }

    fn switch_result_filter(&mut self) {
        self.anywhere = !self.anywhere;
        self.build_cache_table();
    }

    fn key_code_handler(&mut self, key_event: KeyEvent) -> bool {
        if let KeyEvent {
            modifiers: KeyModifiers::CONTROL,
            code: Char('c'),
            ..
        } = key_event
        {
            self.input.clear();
            return true;
        }

        if self.menu_mode != MenuMode::Normal {
            match key_event {
                KeyEvent {
                    code: Char('y') | Char('Y'),
                    ..
                } => {
                    self.confirm(true);
                }
                KeyEvent {
                    code: Char('n') | Char('N'),
                    ..
                } => {
                    self.confirm(false);
                }
                _ => {}
            }
            false
        } else {
            self.key_handler(key_event)
        }
    }

    fn select(&mut self) {
        self.refresh_matches(true);

        let mut screen = stdout();
        terminal::enable_raw_mode().unwrap();
        queue!(screen, EnterAlternateScreen).unwrap();

        let mut idx = -1;
        let mut resized = true;
        loop {
            queue!(screen, cursor::Hide).unwrap();
            if idx == -1 {
                queue!(screen, Clear(ClearType::All)).unwrap();
                self.results(&mut screen, -1, self.width, self.height, resized);
                self.menubar(&mut screen, self.width, self.height);
                self.prompt::<true, _>(&mut screen, self.width, self.height);
            } else {
                self.results(&mut screen, idx, self.width, self.height, false);
                self.prompt::<false, _>(&mut screen, self.width, self.height);
            }
            queue!(screen, cursor::Show).unwrap();
            screen.flush().unwrap();
            resized = false;

            let event = read();
            if event.is_err() {
                continue;
            }

            match event.unwrap() {
                Event::Key(key_event) => {
                    let cursor = self.input.cursor;
                    let menu = self.menu_mode;
                    let rank = self.rank;
                    let anywhere = self.anywhere;
                    idx = self.selection as i32;

                    if self.key_code_handler(key_event) {
                        break;
                    }

                    if cursor != self.input.cursor
                        || menu != self.menu_mode
                        || rank != self.rank
                        || anywhere != self.anywhere
                    {
                        idx = -1;
                    }
                }
                Event::Resize(w, h) => {
                    resized = true;
                    self.width = w;
                    self.height = h;
                    self.input.update_cap(2 * w - 4);
                    idx = -1;
                }
                _ => (),
            }
        }

        queue!(screen, LeaveAlternateScreen).unwrap();
        terminal::disable_raw_mode().unwrap();
    }

    fn key_handler(&mut self, event: KeyEvent) -> bool {
        match event {
            KeyEvent {
                code: KeyCode::Enter, ..
            } => {
                self.accept_selection(true);
                return true;
            }

            KeyEvent { code: KeyCode::Tab, .. } => {
                self.accept_selection(false);
                return true;
            }

            KeyEvent {
                code: KeyCode::Left, ..
            } => self.input.move_cursor(Move::Backward),

            KeyEvent {
                code: KeyCode::Right, ..
            } => self.input.move_cursor(Move::Forward),

            KeyEvent { code: KeyCode::Up, .. } => self.move_selection(MoveSelection::Up),

            KeyEvent {
                code: KeyCode::Down, ..
            } => self.move_selection(MoveSelection::Down),

            KeyEvent {
                code: KeyCode::Backspace,
                ..
            } => {
                self.input.delete(Move::Backward);
                self.refresh_matches(true);
            }

            KeyEvent {
                code: KeyCode::Delete, ..
            } => {
                self.input.delete(Move::Forward);
                self.refresh_matches(true);
            }

            KeyEvent {
                code: KeyCode::Home, ..
            } => self.input.move_cursor(Move::BOL),

            KeyEvent { code: KeyCode::End, .. } => self.input.move_cursor(Move::EOL),

            KeyEvent { code: Char(c), .. } => {
                self.input.insert(c);
                self.refresh_matches(true);
            }

            KeyEvent {
                code: KeyCode::F(1), ..
            } => {
                self.rank = !self.rank;
                self.refresh_matches(true);
            }

            KeyEvent {
                code: KeyCode::F(2), ..
            } => {
                if !self.matches.is_empty() {
                    self.menu_mode = MenuMode::ConfirmDelete;
                }
            }

            KeyEvent {
                code: KeyCode::F(3), ..
            } => {
                self.switch_result_filter();
                self.refresh_matches(true);
            }
            _ => {}
        }

        false
    }

    fn format_command_text(command: &Command, target: &str, width: u16, hl: Color, fg: Color) -> String {
        let max_grapheme_length = cmp::max(width - 14, 0);
        let mut out1 = FixedLengthGraphemeString::empty(max_grapheme_length);
        out1.push_grapheme_str(&command.cmd[..]);
        if target.is_empty() {
            return out1.string;
        }

        let cmd = &out1.string;
        let mut out2 = FixedLengthGraphemeString::empty(0);
        let mut prev: usize = 0;
        for &(start, mut end) in &command.match_bounds {
            if start >= cmd.len() {
                break;
            }

            if end > cmd.len() {
                end = cmd.len();
            }

            if prev != start {
                out2.push_str(&cmd[prev..start]);
            }

            execute!(out2, SetForegroundColor(hl)).unwrap();
            out2.push_str(&cmd[start..end]);
            execute!(out2, SetForegroundColor(fg)).unwrap();
            prev = end;
        }

        if prev != cmd.len() {
            out2.push_str(&cmd[prev..]);
        }
        out2.string
    }

    fn line_range<const N: i32>(&self, height: u16) -> (i16, i16) {
        let height = height as i32;
        if self.settings.bottom {
            return if height >= N {
                ((height - N) as i16, 0)
            } else {
                (-1, -1)
            };
        }

        if height <= N {
            (-1, -1)
        } else {
            (N as i16, (height - 1) as i16)
        }
    }

    fn command_line_index(&self, index: i16) -> i16 { if self.settings.bottom { -index } else { index } }
}
