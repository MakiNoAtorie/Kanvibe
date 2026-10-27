//! "Save as image…": off-screen rendering of the whole Board to a PNG.
use iced::Theme;
use kanvibe::app::{App, Message};
use kanvibe::domain::{Board, Id};
use kanvibe::filter::Filter;
use kanvibe::image::render;

/// A current Board "Work" with `columns` Columns; the first holds `cards`.
fn app_with(columns: usize, cards: &[&str]) -> App {
    let mut app = App::default();
    let work = app.data.create_board("Work").unwrap();
    while app.data.boards[0].columns.len() < columns {
        app.data.add_column(work);
    }
    app.data.boards[0].columns.truncate(columns);
    let first = app.data.boards[0].columns[0].id;
    for title in cards {
        app.data.add_card(first, title);
    }
    app.current_board = Some(work);
    app
}

fn board(app: &App) -> &Board {
    &app.data.boards[0]
}

/// (width, height, RGBA rows top to bottom) of a PNG.
fn decode(png: &[u8]) -> (u32, u32, Vec<Vec<u8>>) {
    let mut reader = png::Decoder::new(std::io::Cursor::new(png))
        .read_info()
        .unwrap();
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).unwrap();
    assert_eq!(info.color_type, png::ColorType::Rgba);
    let rows = buf[..info.buffer_size()]
        .chunks_exact(info.line_size)
        .map(<[u8]>::to_vec)
        .collect();
    (info.width, info.height, rows)
}

const WHITE: [u8; 4] = [255, 255, 255, 255];

fn all_white(row: &[u8]) -> bool {
    row.as_chunks::<4>().0.iter().all(|p| *p == WHITE)
}

#[test]
fn a_small_board_renders_to_a_png_of_the_column_width_with_the_background_cropped() {
    let app = app_with(3, &["A", "B"]);
    let png = render(board(&app), &Filter::default(), &Theme::Light).unwrap();
    let (width, height, rows) = decode(&png);
    // 12 px padding each side, three 280 px Columns 12 px apart.
    assert_eq!(width, 888);
    assert!(height > 50 && height < 400, "height {height}");
    // Cropped to the content plus the 12 px bottom padding.
    assert!(rows[height as usize - 12..].iter().all(|r| all_white(r)));
    assert!(!all_white(&rows[height as usize - 13]));
}

fn too_large(size: &str) -> String {
    format!(
        "Board too large for one image ({size} px; limit 8192). \
         Use the filter or archive old Cards, then try again."
    )
}

#[test]
fn a_board_wider_than_the_cap_is_refused_before_rendering() {
    let mut app = app_with(29, &["A"]);
    let id: Id = board(&app).id;
    let _ = app.update(Message::SaveImage(id));
    assert_eq!(app.notice, Some(too_large("8480×…")));
    assert!(!app.saving_image);
    // The renderer refuses too, without touching wgpu (which panics over 8192 px).
    let refused = render(board(&app), &Filter::default(), &Theme::Light);
    assert_eq!(refused, Err(too_large("8480×…")));
    // 28 Columns (8188 px) fit.
    let mut app = app_with(28, &["A"]);
    let _ = app.update(Message::SaveImage(board(&app).id));
    assert_eq!(app.notice, None);
}

#[test]
fn a_board_taller_than_the_cap_is_refused_after_rendering() {
    let titles: Vec<String> = (0..400).map(|i| format!("Card {i}")).collect();
    let titles: Vec<&str> = titles.iter().map(String::as_str).collect();
    let app = app_with(1, &titles);
    let refused = render(board(&app), &Filter::default(), &Theme::Light);
    assert_eq!(refused, Err(too_large("304×>8192")));
}

#[test]
fn the_image_applies_the_filter_strictly() {
    let filter = Filter {
        text: "apple".into(),
        labels: vec![],
    };
    let height =
        |app: &App, filter: &Filter| decode(&render(board(app), filter, &Theme::Light).unwrap()).1;
    let all = app_with(3, &["apple", "banana", "cherry"]);
    let one = app_with(3, &["apple"]);
    assert!(height(&all, &filter) < height(&all, &Filter::default()));
    // The hidden Cards leave no trace: as tall as a Board without them.
    assert_eq!(height(&all, &filter), height(&one, &filter));
}

#[test]
fn the_menu_item_reads_saving_image_while_it_runs() {
    let mut app = app_with(3, &["A"]);
    let id = board(&app).id;
    let _ = app.update(Message::ToggleBoardMenu);
    let _ = app.update(Message::SaveImage(id));
    assert!(app.saving_image);
    let mut ui = iced_test::simulator(app.view());
    assert!(ui.find("Saving image…").is_ok());
    assert!(ui.find("Save as image…").is_err());
    drop(ui);
    // A second click while saving does nothing; a cancelled dialog ends the save.
    let _ = app.update(Message::SaveImage(id));
    let _ = app.update(Message::ImageSaved(None));
    assert!(!app.saving_image);
    assert!(
        iced_test::simulator(app.view())
            .find("Save as image…")
            .is_ok()
    );
    // A failure is shown as a notice.
    let _ = app.update(Message::SaveImage(id));
    let _ = app.update(Message::ImageSaved(Some(Err("Image not saved".into()))));
    assert_eq!(app.notice.as_deref(), Some("Image not saved"));
    assert!(!app.saving_image);
}
