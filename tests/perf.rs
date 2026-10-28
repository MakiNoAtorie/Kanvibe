//! Performance harness, not a check: `cargo test --release --test perf -- --ignored --nocapture`.
//! Builds an App at three sizes through the public API and prints the mean cost of the hot
//! paths (one message plus `save_if_changed`, a view, storage, history, lookups).
use std::path::Path;
use std::time::{Duration, Instant};

use iced::keyboard::{self, key::Named};
use iced::{Font, Pixels, Point, Size};
use iced_test::core::renderer::Headless;
use iced_test::runtime::UserInterface;
use iced_test::runtime::user_interface::Cache;
use kanvibe::app::{App, Field, Message, Selection};
use kanvibe::domain::{Board, Card, Column, Data, Item, Label};
use kanvibe::drag::{self, Target};
use kanvibe::history::History;
use kanvibe::storage::{self, Rotation, Settings};

/// (name, Boards, Cards).
const SIZES: [(&str, usize, usize); 3] = [
    ("realistic", 5, 300),
    ("large", 10, 5_000),
    ("stress", 20, 20_000),
];

const STAMP: &str = "2024-05-01T10:00:00Z";
const WORDS: [&str; 12] = [
    "fix", "login", "page", "update", "docs", "review", "deploy", "bug", "cache", "report",
    "design", "meeting",
];

/// `boards` Boards of three Columns with 5 Labels each, `cards` Cards spread evenly; every
/// other Card has a description, every third a 3-Item Checklist, most have 1-2 Labels.
fn data(boards: usize, cards: usize) -> Data {
    let mut next = 0;
    let mut id = || {
        next += 1;
        next
    };
    let per_board = cards / boards;
    let boards = (0..boards)
        .map(|b| {
            let board = id();
            let labels: Vec<Label> = ["Urgent", "Bug", "Feature", "Chore", "Idea"]
                .iter()
                .zip(["red", "orange", "gray", "amber", "red"])
                .map(|(name, color)| Label {
                    id: id(),
                    name: (*name).into(),
                    color: color.into(),
                })
                .collect();
            let mut columns: Vec<Column> = ["To do", "Doing", "Done"]
                .map(|name| Column {
                    id: id(),
                    name: name.into(),
                    limit: Some(50),
                    collapsed: false,
                    cards: Vec::new(),
                })
                .into();
            for i in 0..per_board {
                let w = |k: usize| WORDS[(i * 7 + k * 5 + b) % WORDS.len()];
                let card = Card {
                    id: id(),
                    title: format!("{} the {} {} #{i}", w(0), w(1), w(2)),
                    created: STAMP.into(),
                    updated: STAMP.into(),
                    checklist: match i % 3 {
                        0 => (0..3)
                            .map(|k| Item {
                                id: id(),
                                text: format!("step {k} {}", w(k)),
                                done: k == 0,
                            })
                            .collect(),
                        _ => Vec::new(),
                    },
                    labels: (0..i % 3).map(|k| labels[(i + k) % 5].id).collect(),
                    description: match i % 2 {
                        0 => format!(
                            "Some **notes** about {} and {}.\n\n- one\n- two [link](https://example.com)\n\n{}",
                            w(3),
                            w(4),
                            "More text to make the description a realistic length. ".repeat(3)
                        ),
                        _ => String::new(),
                    },
                    files: Vec::new(),
                    due: None,
                };
                columns[i % 3].cards.push(card);
            }
            Board {
                id: board,
                name: format!("Board {b}"),
                archived: false,
                archived_at: None,
                columns,
                archived_cards: Vec::new(),
                labels,
            }
        })
        .collect();
    Data {
        next_id: next,
        boards,
    }
}

/// Median time of `f`, run for about 300 ms (at least 3, at most `max` times). The median,
/// so a stray antivirus scan or page fault doesn't skew a row.
fn mean(max: u32, mut f: impl FnMut() -> Duration) -> Duration {
    let (mut total, mut times) = (Duration::ZERO, Vec::new());
    while times.len() < 3 || (times.len() < max as usize && total < Duration::from_millis(300)) {
        let t = f();
        total += t;
        times.push(t);
    }
    times.sort();
    times[times.len() / 2]
}

/// The time `f` takes.
fn timed(f: impl FnOnce()) -> Duration {
    let start = Instant::now();
    f();
    start.elapsed()
}

/// `update` then `save_if_changed`, timed; a resulting save is written untimed (it runs on
/// the background writer in the app) and its `Saved` goes back through `update`.
fn send(app: &mut App, message: Message) -> Duration {
    let mut job = None;
    let t = timed(|| {
        let _ = app.update(message);
        job = app.save_if_changed();
    });
    while let Some(j) = job {
        let _ = app.update(j.run());
        job = app.save_if_changed();
    }
    t
}

fn key(named: Named) -> Message {
    let event = keyboard::Event::KeyPressed {
        key: keyboard::Key::Named(named),
        modified_key: keyboard::Key::Named(named),
        physical_key: keyboard::key::Physical::Code(keyboard::key::Code::F24),
        location: keyboard::Location::Standard,
        modifiers: keyboard::Modifiers::empty(),
        text: None,
        repeat: false,
    };
    Message::KeyWithFocus(event, None)
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::copy(from.join("kanvibe.json"), to.join("kanvibe.json")).unwrap();
}

/// Results: one row per hot path, one column per size.
#[derive(Default)]
struct Table(Vec<(String, Vec<Duration>)>);

impl Table {
    fn add(&mut self, name: &str, t: Duration) {
        match self.0.iter_mut().find(|(n, _)| n == name) {
            Some((_, times)) => times.push(t),
            None => self.0.push((name.into(), vec![t])),
        }
    }

    fn print(&self) {
        let fmt = |d: &Duration| match d.as_secs_f64() * 1e3 {
            ms if ms < 1.0 => format!("{:.1} µs", ms * 1e3),
            ms => format!("{ms:.2} ms"),
        };
        let build = if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        };
        println!();
        print!("{:<44}", format!("hot path (median, {build})"));
        SIZES
            .iter()
            .for_each(|(n, b, c)| print!("{:>22}", format!("{n} {b}B/{c}C")));
        println!();
        for (name, times) in &self.0 {
            print!("{name:<44}");
            times.iter().for_each(|t| print!("{:>22}", fmt(t)));
            println!();
        }
    }
}

fn measure(table: &mut Table, boards: usize, cards: usize) {
    let data = data(boards, cards);
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().to_path_buf();
    let settings = Settings {
        last_board: Some(data.boards[0].id),
        ..Settings::default()
    };

    // Storage.
    table.add(
        "storage::save",
        mean(10, || {
            timed(|| storage::save(&dir, &data, &settings).unwrap())
        }),
    );
    table.add(
        "storage::open (load + repair)",
        mean(10, || timed(|| drop(storage::open(&dir).unwrap()))),
    );
    table.add(
        "startup: App::open",
        mean(10, || timed(|| drop(App::open(dir.clone()).unwrap()))),
    );
    let rotate_dir = tempfile::tempdir().unwrap();
    copy_dir(&dir, rotate_dir.path());
    table.add(
        "storage::rotate (first save, no .bak1)",
        timed(|| storage::rotate(rotate_dir.path(), &mut Rotation::Due, true).unwrap()),
    );
    // The usual first save of a session: `.bak1` holds an older, different file.
    let older = tempfile::tempdir().unwrap();
    let mut old = data.clone();
    old.boards[0].name = "Older".into();
    storage::save(older.path(), &old, &settings).unwrap();
    table.add(
        "storage::rotate (first save, older .bak1)",
        mean(5, || {
            std::fs::copy(
                older.path().join("kanvibe.json"),
                rotate_dir.path().join("kanvibe.json.bak1"),
            )
            .unwrap();
            timed(|| storage::rotate(rotate_dir.path(), &mut Rotation::Due, true).unwrap())
        }),
    );
    // The same in the app: the session's first saving message, timed on the UI thread.
    table.add(
        "first SubmitCapture of a session (rotation due)",
        mean(5, || {
            let session = tempfile::tempdir().unwrap();
            copy_dir(&dir, session.path());
            std::fs::copy(
                older.path().join("kanvibe.json"),
                session.path().join("kanvibe.json.bak1"),
            )
            .unwrap();
            let mut app = App::open(session.path().to_path_buf()).unwrap();
            let column = app.data.boards[0].columns[0].id;
            let _ = send(&mut app, Message::OpenCapture(column));
            let _ = send(&mut app, Message::CaptureChanged("New card".into()));
            let t = send(&mut app, Message::SubmitCapture);
            assert!(session.path().join("kanvibe.json.bak2").exists(), "rotated");
            t
        }),
    );
    table.add(
        "storage::rotate (.bak1 fresh, Done)",
        mean(100, || {
            timed(|| storage::rotate(rotate_dir.path(), &mut Rotation::Done, true).unwrap())
        }),
    );

    // History and lookups.
    let last = data.boards.last().unwrap().columns[2]
        .cards
        .last()
        .unwrap()
        .id;
    let mut history = History::default();
    let mut boards = data.boards.clone();
    history.sync(&boards);
    let mut flip = false;
    table.add(
        "History::record after a one-Card change",
        mean(200, || {
            flip = !flip;
            boards[0].columns[0].cards[0].title = if flip { "A" } else { "B" }.into();
            timed(|| assert!(history.record(&boards, None, None)))
        }),
    );
    table.add(
        "History::record, nothing changed",
        mean(200, || {
            timed(|| assert!(!history.record(&boards, None, None)))
        }),
    );
    table.add(
        "Data clone",
        mean(200, || timed(|| drop(std::hint::black_box(data.clone())))),
    );
    let copy = data.clone();
    table.add(
        "Data == Data (equal)",
        mean(200, || {
            timed(|| assert!(std::hint::black_box(&copy) == &data))
        }),
    );
    table.add(
        "Data::card(last)",
        mean(1000, || timed(|| assert!(data.card(last).is_some()))),
    );
    table.add(
        "Data::board_of_card(last)",
        mean(1000, || {
            timed(|| assert!(data.board_of_card(last).is_some()))
        }),
    );

    // The App, saving into `dir`.
    let mut app = App::open(dir.clone()).unwrap();
    let _ = send(&mut app, Message::Resized(Size::new(1600.0, 900.0)));
    let board = &app.data.boards[0];
    let (todo, doing) = (board.columns[0].id, board.columns[1].id);
    let first = board.columns[0].cards[0].id;
    let mid = board.columns[0].cards[board.columns[0].cards.len() / 2].id;

    let view = |app: &App| timed(|| drop(app.view()));
    table.add("view: Card closed", mean(200, || view(&app)));

    let mut w = 1600.0;
    table.add(
        "Resized",
        mean(1000, || {
            w = if w == 1600.0 { 1601.0 } else { 1600.0 };
            send(&mut app, Message::Resized(Size::new(w, 900.0)))
        }),
    );

    // Selection moves.
    let _ = send(
        &mut app,
        Message::Select(Selection {
            column: todo,
            card: Some(mid),
        }),
    );
    let _ = send(&mut app, Message::CloseCard);
    let mut down = false;
    table.add(
        "KeyWithFocus(arrow) selection move",
        mean(1000, || {
            down = !down;
            send(
                &mut app,
                key(if down {
                    Named::ArrowDown
                } else {
                    Named::ArrowUp
                }),
            )
        }),
    );

    // Drag.
    let _ = send(&mut app, Message::DragPress(drag::Item::Card(first)));
    let _ = send(&mut app, Message::DragMoved(Point::new(100.0, 100.0)));
    let _ = send(&mut app, Message::DragMoved(Point::new(120.0, 120.0)));
    let target = Target::Card {
        column: doing,
        index: 3,
    };
    let _ = send(&mut app, Message::DragHover(target));
    let mut x = 120.0;
    table.add(
        "DragMoved",
        mean(1000, || {
            x += 1.0;
            send(&mut app, Message::DragMoved(Point::new(x, 120.0)))
        }),
    );
    table.add("DragTick", mean(1000, || send(&mut app, Message::DragTick)));
    table.add("view: dragging a Card", mean(200, || view(&app)));
    let _ = send(&mut app, Message::DragCancel);

    // Typing a rename.
    let _ = send(&mut app, Message::StartEdit(Field::CardTitle(mid)));
    let mut n = 0;
    table.add(
        "EditChanged (rename keystroke)",
        mean(1000, || {
            n += 1;
            send(&mut app, Message::EditChanged(format!("Renamed {n}")))
        }),
    );
    table.add(
        "EditTick",
        mean(1000, || send(&mut app, Message::EditTick(Instant::now()))),
    );
    let _ = send(&mut app, Message::CommitEdit);

    // Capture.
    let _ = send(&mut app, Message::OpenCapture(doing));
    table.add(
        "CaptureChanged",
        mean(1000, || {
            send(&mut app, Message::CaptureChanged("New card".into()))
        }),
    );
    table.add(
        "SubmitCapture (adds a Card, saves)",
        mean(100, || {
            let _ = send(&mut app, Message::CaptureChanged("New card".into()));
            send(&mut app, Message::SubmitCapture)
        }),
    );
    let _ = send(&mut app, Message::Escape);
    table.add("Undo (saves)", mean(50, || send(&mut app, Message::Undo)));
    table.add("Redo (saves)", mean(50, || send(&mut app, Message::Redo)));

    // Views.
    let _ = send(
        &mut app,
        Message::Select(Selection {
            column: todo,
            card: Some(first),
        }),
    );
    table.add("view: Card open", mean(200, || view(&app)));
    let _ = send(&mut app, Message::CloseCard);
    let _ = send(&mut app, Message::FilterChanged("fix".into()));
    table.add("view: text Filter on", mean(200, || view(&app)));
    let _ = send(&mut app, Message::ClearFilter);
    // What the app does per message: rebuild the view and lay it out against the last cache.
    let size = Size::new(1600.0, 900.0);
    let mut renderer = iced_test::futures::futures::executor::block_on(
        <iced::Renderer as Headless>::new(Font::with_name("Fira Sans"), Pixels(16.0), None),
    )
    .expect("a headless renderer");
    let mut cache =
        Some(UserInterface::build(app.view(), size, Cache::default(), &mut renderer).into_cache());
    table.add(
        "view + warm layout (UserInterface::build)",
        mean(100, || {
            timed(|| {
                let ui =
                    UserInterface::build(app.view(), size, cache.take().unwrap(), &mut renderer);
                cache = Some(ui.into_cache());
            })
        }),
    );

    // The simulator creates a renderer each time: subtract an empty view's cost.
    let empty = || {
        timed(|| {
            drop(iced_test::simulator(iced::Element::<Message>::from(
                iced::widget::text(""),
            )))
        })
    };
    let _ = empty(); // warm up the renderer
    let base = mean(5, empty);
    let layout = mean(5, || timed(|| drop(iced_test::simulator(app.view()))));
    table.add(
        "simulator(view) layout, less empty view",
        layout.saturating_sub(base),
    );
}

#[test]
#[ignore = "performance harness; run with --release --ignored --nocapture"]
fn hot_paths() {
    let mut table = Table::default();
    for (_, boards, cards) in SIZES {
        measure(&mut table, boards, cards);
    }
    table.print();
}
