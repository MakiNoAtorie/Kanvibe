//! Load repairs and migrations (#22): hand-edited files with bad references load repaired,
//! never refused, and never reach a problem screen.
use std::fs;
use std::path::PathBuf;

use kanvibe::app::App;
use kanvibe::domain::Data;
use kanvibe::startup::Screen;
use kanvibe::storage::{self, Rotation, Settings};

fn write(json: &str) -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().to_path_buf();
    fs::write(dir.join("kanvibe.json"), json).unwrap();
    (tmp, dir)
}

/// Loads a hand-crafted file and checks it opens straight into the app with rotation off.
fn load_repaired(json: &str) -> (Data, Settings) {
    let (_tmp, dir) = write(json);
    let loaded = storage::load(&dir).unwrap().unwrap();
    let app = opened(Screen::open(dir));
    assert_eq!(
        app.rotation,
        Rotation::Off,
        "a repaired file must not rotate"
    );
    loaded
}

fn opened(screen: Screen) -> App {
    match screen {
        Screen::App(app) => app,
        Screen::Problem(problem) => panic!("expected the app, got {problem:?}"),
    }
}

#[test]
fn label_ids_the_cards_board_does_not_have_are_dropped() {
    let (data, _) = load_repaired(
        r#"{ "version": 1, "next_id": 9, "boards": [
            { "id": 1, "name": "Work", "labels": [ { "id": 2, "name": "Bug", "color": "red" } ],
              "columns": [ { "id": 3, "name": "To do", "cards": [
                { "id": 4, "title": "A", "labels": [ 2, 7, 99 ] } ] } ],
              "archived_cards": [ { "card": { "id": 5, "title": "B", "labels": [ 99, 2 ] },
                                    "column_id": 3, "archived_at": "2020-01-01T00:00:00Z" } ] },
            { "id": 6, "name": "Home", "labels": [ { "id": 7, "name": "Bug", "color": "red" } ] } ] }"#,
    );
    assert_eq!(data.boards[0].columns[0].cards[0].labels, [2]);
    assert_eq!(data.boards[0].archived_cards[0].card.labels, [2]);
}

#[test]
fn an_unknown_colour_becomes_the_first_palette_colour() {
    let (data, _) = load_repaired(
        r#"{ "version": 1, "next_id": 3, "boards": [ { "id": 1, "name": "Work", "labels": [
            { "id": 2, "name": "Bug", "color": "chartreuse" },
            { "id": 3, "name": "Idea", "color": "teal" } ] } ] }"#,
    );
    let colours: Vec<_> = data.boards[0].labels.iter().map(|l| &l.color).collect();
    assert_eq!(colours, ["gray", "teal"]);
}

#[test]
fn a_column_limit_below_1_is_none() {
    for limit in ["0", "-2"] {
        let (data, _) = load_repaired(&format!(
            r#"{{ "version": 1, "next_id": 3, "boards": [ {{ "id": 1, "name": "Work", "columns": [
                {{ "id": 2, "name": "Doing", "limit": {limit} }}, {{ "id": 3, "name": "Done", "limit": 4 }} ] }} ] }}"#
        ));
        let limits: Vec<_> = data.boards[0].columns.iter().map(|c| c.limit).collect();
        assert_eq!(limits, [None, Some(4)], "limit {limit}");
    }
}

#[test]
fn a_next_id_below_the_highest_id_is_raised_to_it() {
    let (mut data, _) = load_repaired(
        r#"{ "version": 1, "next_id": 1, "boards": [ { "id": 1, "name": "Work", "columns": [
            { "id": 7, "name": "To do", "cards": [ { "id": 4, "title": "A" } ] } ] } ] }"#,
    );
    assert_eq!(data.next_id, 7);
    assert_eq!(data.create_board("Home"), Some(8));
}

#[test]
fn duplicate_ids_get_fresh_ones_and_the_first_keeps_its_id() {
    // Walk order: Board, its Labels, Columns with their Cards and Items, then archived Cards.
    let (data, _) = load_repaired(
        r#"{ "version": 1, "next_id": 2, "boards": [ { "id": 1, "name": "Work",
            "labels": [ { "id": 3, "name": "Bug", "color": "red" } ],
            "columns": [ { "id": 2, "name": "To do", "cards": [
                { "id": 2, "title": "A", "labels": [ 3 ],
                  "checklist": [ { "id": 3, "text": "step" } ] } ] } ],
            "archived_cards": [ { "card": { "id": 1, "title": "B" }, "column_id": 2,
                                  "archived_at": "2020-01-01T00:00:00Z" } ] } ] }"#,
    );
    let board = &data.boards[0];
    let card = &board.columns[0].cards[0];
    let ids = [
        board.id,
        board.labels[0].id,
        board.columns[0].id,
        card.id,
        card.checklist[0].id,
        board.archived_cards[0].card.id,
    ];
    assert_eq!(ids, [1, 3, 2, 4, 5, 6]);
    assert_eq!(data.next_id, 6);
    assert_eq!(card.labels, [3]);
    assert_eq!(board.archived_cards[0].column_id, 2);
}

#[test]
fn names_are_trimmed_and_blank_ones_get_a_default() {
    let (data, _) = load_repaired(
        r#"{ "version": 1, "next_id": 6, "boards": [ { "id": 1, "name": "  ",
            "labels": [ { "id": 2, "name": "", "color": "red" }, { "id": 3, "name": " Bug ", "color": "red" } ],
            "columns": [ { "id": 4, "name": "\t", "cards": [ { "id": 5, "title": " " } ] } ],
            "archived_cards": [ { "card": { "id": 6, "title": " Old " }, "column_id": 4,
                                  "archived_at": "2020-01-01T00:00:00Z" } ] } ] }"#,
    );
    let board = &data.boards[0];
    assert_eq!(board.name, "Untitled");
    let labels: Vec<_> = board.labels.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(labels, ["Label", "Bug"]);
    assert_eq!(board.columns[0].name, "New column");
    assert_eq!(board.columns[0].cards[0].title, "Untitled");
    assert_eq!(board.archived_cards[0].card.title, "Old");
}

#[test]
fn blank_checklist_items_are_removed_and_the_rest_trimmed() {
    let (data, _) = load_repaired(
        r#"{ "version": 1, "next_id": 5, "boards": [ { "id": 1, "name": "Work", "columns": [
            { "id": 2, "name": "To do", "cards": [ { "id": 3, "title": "A", "checklist": [
                { "id": 4, "text": "  " }, { "id": 5, "text": " step ", "done": true } ] } ] } ] } ] }"#,
    );
    let items = &data.boards[0].columns[0].cards[0].checklist;
    assert_eq!(items.len(), 1);
    assert_eq!(
        (items[0].id, items[0].text.as_str(), items[0].done),
        (5, "step", true)
    );
}

#[test]
fn an_older_version_is_migrated_and_its_original_kept_as_a_versioned_copy() {
    let json = r#"{ "version": 0, "next_id": 1, "boards": [ { "id": 1, "name": "Work" } ] }"#;
    let (_tmp, dir) = write(json);

    let mut app = opened(Screen::open(dir.clone()));
    assert_eq!(app.data.boards[0].name, "Work");
    assert_eq!(
        fs::read_to_string(dir.join("kanvibe.v0.json")).unwrap(),
        json
    );

    app.data.create_board("Home");
    while let Some(job) = app.save_if_changed() {
        let _ = app.update(job.run());
    }
    assert_eq!(
        fs::read_to_string(dir.join("kanvibe.v0.json")).unwrap(),
        json
    );
    let saved = fs::read_to_string(dir.join("kanvibe.json")).unwrap();
    assert!(saved.contains("\"version\": 1"), "{saved}");
    assert_eq!(storage::load(&dir).unwrap().unwrap().0.boards.len(), 2);
}

#[test]
fn last_board_is_ignored_unless_it_names_an_active_board() {
    // Archived: the next active Board below it (as after archiving); unknown: the first.
    for (last, current) in [(2, 3), (99, 1)] {
        let (_tmp, dir) = write(&format!(
            r#"{{ "version": 1, "next_id": 3, "settings": {{ "last_board": {last} }}, "boards": [
                {{ "id": 1, "name": "Work" }}, {{ "id": 2, "name": "Old", "archived": true }},
                {{ "id": 3, "name": "Home" }} ] }}"#
        ));
        let app = opened(Screen::open(dir));
        assert_eq!(app.current_board, Some(current), "last_board {last}");
    }
}
