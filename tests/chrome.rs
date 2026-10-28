//! Theme, window geometry and the sidebar footer (#23).
use std::fs;

use iced::keyboard::{self, Key, Modifiers, key::Named};
use iced::{Point, Size, window};
use kanvibe::app::{App, Message};
use kanvibe::chrome;
use kanvibe::keys::{self, Context};
use kanvibe::skin::Skin;
use kanvibe::storage::{self, Geometry, Settings};

/// What the iced update function does: `update`, then run any save at once.
fn send(app: &mut App, messages: impl IntoIterator<Item = Message>) {
    for message in messages {
        let _ = app.update(message);
        while let Some(job) = app.save_if_changed() {
            let _ = app.update(job.run());
        }
    }
}

fn opened(dir: &std::path::Path) -> App {
    App::open(dir.to_path_buf()).unwrap()
}

/// An app on disk with one Board, so a data change is one `NewBoard` away.
fn app_in(dir: &std::path::Path) -> App {
    let mut app = opened(dir);
    send(
        &mut app,
        [
            Message::NewBoard,
            Message::BoardNameChanged("Work".into()),
            Message::CreateBoard,
        ],
    );
    app
}

fn saved_settings(dir: &std::path::Path) -> Settings {
    storage::load(dir).unwrap().unwrap().1
}

fn geometry(x: f32, y: f32, width: f32, height: f32) -> Geometry {
    Geometry {
        x,
        y,
        width,
        height,
    }
}

/// Resizes and moves the window the way iced reports it: the events, then the maximised answer.
fn place(app: &mut App, g: Geometry, maximized: bool) {
    send(
        app,
        [
            Message::Resized(Size::new(g.width, g.height)),
            Message::Moved(Point::new(g.x, g.y)),
            Message::Maximized(maximized),
        ],
    );
}

// --- Theme ---

#[test]
fn theme_is_light_and_any_theme_can_be_picked() {
    let mut app = App::default();
    assert_eq!(Skin::of(&app.theme()), Skin::Light);
    assert!(!app.theme().extended_palette().is_dark);
    for skin in Skin::ALL {
        send(&mut app, [Message::SetTheme(skin)]);
        assert_eq!(app.skin, skin);
        assert_eq!(Skin::of(&app.theme()), skin);
    }
    send(&mut app, [Message::SetTheme(Skin::Dark)]);
    assert!(app.theme().extended_palette().is_dark);
}

#[test]
fn highlighter_theme_follows_the_app_theme() {
    use iced::highlighter::Theme as Highlight;
    let mut app = App::default();
    let light = app.highlighter_theme();
    send(&mut app, [Message::SetTheme(Skin::Dark)]);
    assert_ne!(app.highlighter_theme(), light);
    assert_eq!(app.highlighter_theme(), Highlight::Base16Ocean);
}

#[test]
fn theme_is_remembered_between_launches() {
    let tmp = tempfile::tempdir().unwrap();
    let mut app = app_in(tmp.path());
    send(&mut app, [Message::SetTheme(Skin::Dark)]);
    assert_eq!(saved_settings(tmp.path()).skin, Skin::Dark);
    assert!(opened(tmp.path()).theme().extended_palette().is_dark);
}

// --- Window geometry ---

#[test]
fn geometry_is_written_with_the_next_save_but_never_triggers_one() {
    let tmp = tempfile::tempdir().unwrap();
    let mut app = app_in(tmp.path());
    let before = fs::read_to_string(storage::file_path(tmp.path())).unwrap();

    place(&mut app, geometry(40.0, 50.0, 1000.0, 700.0), false);
    assert!(app.save_if_changed().is_none());
    assert_eq!(
        fs::read_to_string(storage::file_path(tmp.path())).unwrap(),
        before
    );

    send(&mut app, [Message::SetTheme(Skin::Dark)]); // a real change carries it along
    assert_eq!(
        saved_settings(tmp.path()).window,
        Some(geometry(40.0, 50.0, 1000.0, 700.0))
    );
    assert!(opened(tmp.path()).save_if_changed().is_none());
}

#[test]
fn maximised_flag_is_saved_apart_and_keeps_the_restored_rectangle() {
    let tmp = tempfile::tempdir().unwrap();
    let mut app = app_in(tmp.path());
    place(&mut app, geometry(40.0, 50.0, 1000.0, 700.0), false);
    // Maximising reports the screen-sized rectangle, then the flag.
    place(&mut app, geometry(-8.0, -8.0, 1936.0, 1096.0), true);
    assert!(app.save_if_changed().is_none());
    send(&mut app, [Message::SetTheme(Skin::Dark)]);

    let settings = saved_settings(tmp.path());
    assert!(settings.maximized);
    assert_eq!(settings.window, Some(geometry(40.0, 50.0, 1000.0, 700.0)));
}

#[test]
fn minimised_position_is_not_saved() {
    let tmp = tempfile::tempdir().unwrap();
    let mut app = app_in(tmp.path());
    place(&mut app, geometry(40.0, 50.0, 1000.0, 700.0), false);
    // Windows parks a minimised window at -32000,-32000 (logical: divided by the scale).
    place(&mut app, geometry(-16000.0, -16000.0, 0.0, 0.0), false);
    send(&mut app, [Message::SetTheme(Skin::Dark)]);
    assert_eq!(
        saved_settings(tmp.path()).window,
        Some(geometry(40.0, 50.0, 1000.0, 700.0))
    );
}

#[test]
fn a_move_alone_updates_the_saved_position() {
    let tmp = tempfile::tempdir().unwrap();
    let mut app = app_in(tmp.path());
    place(&mut app, geometry(40.0, 50.0, 1000.0, 700.0), false);
    send(
        &mut app,
        [
            Message::Moved(Point::new(300.0, 200.0)),
            Message::Maximized(false),
        ],
    );
    send(&mut app, [Message::SetTheme(Skin::Dark)]);
    assert_eq!(
        saved_settings(tmp.path()).window,
        Some(geometry(300.0, 200.0, 1000.0, 700.0))
    );
}

#[test]
fn saved_geometry_survives_a_launch_that_never_moves_the_window() {
    let tmp = tempfile::tempdir().unwrap();
    let mut app = app_in(tmp.path());
    place(&mut app, geometry(40.0, 50.0, 1000.0, 700.0), false);
    send(&mut app, [Message::SetTheme(Skin::Dark)]);

    let mut again = opened(tmp.path());
    send(&mut again, [Message::SetTheme(Skin::Paper)]);
    assert_eq!(
        saved_settings(tmp.path()).window,
        Some(geometry(40.0, 50.0, 1000.0, 700.0))
    );
}

// --- Opening the window ---

const MONITOR: Geometry = Geometry {
    x: 0.0,
    y: 0.0,
    width: 1920.0,
    height: 1080.0,
};

#[test]
fn closing_saves_a_window_only_change() {
    let tmp = tempfile::tempdir().unwrap();
    let mut app = app_in(tmp.path());
    place(&mut app, geometry(40.0, 50.0, 1000.0, 700.0), false);
    send(&mut app, [Message::Maximized(true)]);

    send(&mut app, [Message::CloseRequested]);

    assert_eq!(app.closing, kanvibe::app::Closing::Exiting);
    let settings = saved_settings(tmp.path());
    assert_eq!(settings.window, Some(geometry(40.0, 50.0, 1000.0, 700.0)));
    assert!(settings.maximized);
}

#[test]
fn on_screen_rectangle_is_restored() {
    let tmp = tempfile::tempdir().unwrap();
    let mut app = app_in(tmp.path());
    place(&mut app, geometry(40.0, 50.0, 1000.0, 700.0), false);
    send(&mut app, [Message::SetTheme(Skin::Dark)]);

    let mut app = opened(tmp.path());
    let settings = app.fit_window(&[MONITOR]);
    assert_eq!(settings.size, Size::new(1000.0, 700.0));
    assert!(
        matches!(settings.position, window::Position::Specific(p) if p == Point::new(40.0, 50.0))
    );
    assert!(!settings.maximized);
    assert_eq!(settings.min_size, Some(Size::new(960.0, 600.0)));
}

#[test]
fn off_screen_rectangle_opens_centred_at_1280_by_720_and_is_dropped() {
    let tmp = tempfile::tempdir().unwrap();
    let mut app = app_in(tmp.path());
    place(&mut app, geometry(5000.0, 50.0, 1000.0, 700.0), false); // the unplugged monitor
    send(&mut app, [Message::SetTheme(Skin::Dark)]);

    let mut app = opened(tmp.path());
    let settings = app.fit_window(&[MONITOR]);
    assert_eq!(settings.size, Size::new(1280.0, 720.0));
    assert!(matches!(settings.position, window::Position::Centered));
    send(&mut app, [Message::SetTheme(Skin::Paper)]);
    assert_eq!(saved_settings(tmp.path()).window, None);
}

#[test]
fn first_run_is_centred_at_1280_by_720() {
    let settings = App::default().fit_window(&[MONITOR]);
    assert_eq!(settings.size, Size::new(1280.0, 720.0));
    assert!(matches!(settings.position, window::Position::Centered));
}

#[test]
fn maximised_flag_is_restored() {
    let tmp = tempfile::tempdir().unwrap();
    let mut app = app_in(tmp.path());
    place(&mut app, geometry(40.0, 50.0, 1000.0, 700.0), true);
    send(&mut app, [Message::SetTheme(Skin::Dark)]);
    assert!(opened(tmp.path()).fit_window(&[MONITOR]).maximized);
}

#[test]
fn rectangle_is_checked_against_logical_monitor_sizes() {
    // A 4K monitor at 200%: 3840x2160 physical is 1920x1080 logical.
    let monitor = chrome::logical(0, 0, 3840, 2160, 2.0);
    assert_eq!(monitor, MONITOR);
}

// --- Footer, shortcuts overlay ---

fn press(key: Named) -> keyboard::Event {
    keyboard::Event::KeyPressed {
        modified_key: Key::Named(key),
        key: Key::Named(key),
        physical_key: keyboard::key::Physical::Code(keyboard::key::Code::F24),
        location: keyboard::Location::Standard,
        modifiers: Modifiers::empty(),
        text: None,
        repeat: false,
    }
}

#[test]
fn f1_opens_the_shortcuts_overlay_and_esc_closes_it() {
    assert_eq!(
        keys::map(&press(Named::F1), &Context::default()),
        Some(Message::ShowShortcuts)
    );
    let mut app = App::default();
    send(&mut app, [Message::ShowShortcuts]);
    assert!(app.shortcuts);
    send(&mut app, [Message::Escape]);
    assert!(!app.shortcuts);
}

#[test]
fn the_shortcuts_overlay_closes_with_its_close_button() {
    let mut app = App::default();
    send(&mut app, [Message::ShowShortcuts]);
    let mut ui = iced_test::simulator(app.view());
    assert!(ui.find("Esc closes this").is_err());
    let _ = ui.click("✕");
    assert!(ui.into_messages().any(|m| m == Message::Escape));
}

#[test]
fn shortcuts_list_names_every_key_the_map_knows() {
    let all: String = chrome::SHORTCUTS
        .iter()
        .map(|(k, _)| *k)
        .collect::<Vec<_>>()
        .join("|");
    for key in ["F1", "F2", "Esc", "Enter", "Ctrl+Z", "Ctrl+Y", "N"] {
        assert!(all.contains(key), "{key} missing from {all}");
    }
}

#[test]
fn open_data_folder_command_targets_the_folder() {
    let command = chrome::open_command(std::path::Path::new("some/dir"));
    assert!(command.get_args().any(|a| a == "some/dir"));
}

#[test]
fn minimising_a_maximised_window_keeps_the_restored_rectangle_and_the_flag() {
    let tmp = tempfile::tempdir().unwrap();
    let mut app = app_in(tmp.path());
    place(&mut app, geometry(40.0, 50.0, 1000.0, 700.0), false);
    place(&mut app, geometry(-8.0, -8.0, 1936.0, 1096.0), true);
    // Minimised: Windows reports 0x0 at -32000,-32000, and no longer says maximised.
    place(&mut app, geometry(-16000.0, -16000.0, 0.0, 0.0), false);
    send(&mut app, [Message::SetTheme(Skin::Dark)]);

    let settings = saved_settings(tmp.path());
    assert!(settings.maximized, "it comes back maximised");
    assert_eq!(settings.window, Some(geometry(40.0, 50.0, 1000.0, 700.0)));
}
