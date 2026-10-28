# Spec: Kanvibe v2 — the open Card, dates, search, and a second coat of paint

Follows the v1 spec (issue #1). Vocabulary from `GLOSSARY.md`; ADRs 0001 (iced), 0002 (single JSON file) and 0003 (Markdown descriptions) still hold.

## Problem Statement

Kanvibe v1 does what it set out to do, but after a few weeks of real use some holes show. A Card can't carry a date, so "when" lives in the title or in my head. Finding a Card means remembering which Board it is on, because the Filter only sees the current Board. A Board with six Columns doesn't fit, and there is no way to tuck one away. Repeating work means retyping Cards. Stale Cards look exactly like fresh ones. A Card can't point at the document it is about, because file links are (rightly) refused. Attaching a Label means opening the Card after typing it. Columns can be dragged but not moved from the keyboard, while Cards can be. The Card panel on the right is a 400px strip that mostly shows empty space while squeezing the Board, and now that the title and description already edit on the Card face it holds only Labels, the Checklist, two buttons and two dates. And the whole thing looks flat: three near-identical greys, short Columns floating in an empty window, no hierarchy between the Board title, Column names and Card text, a description marker that looks like a drag handle, and a dark theme where Cards almost vanish into their Column.

Every one of these has a solution in commercial kanban apps. Most of those solutions come with views, fields and automations I don't want. Kanvibe stays minimal: one Board view, no accounts, no network, one file.

## Solution

Ten changes, each small, none adding a subsystem:

1. **Due date.** One optional date on a Card, typed in the open Card, shown as a badge on the face that turns amber when due today and red when overdue. No reminders, no calendar.
2. **Search.** Ctrl+Shift+F opens a Search view across every Board, active and archived Cards alike, listing matches grouped by Board. Picking a match opens that Board on that Card.
3. **Collapse a Column.** A Column folds to a narrow strip from its menu and unfolds with a click. The fold is remembered.
4. **Duplicate a Card.** One action copies a Card in place with its description, Labels and Checklist (Items unticked). It covers templates and recurring work for one person.
5. **Card ageing.** A Card untouched for two weeks fades on the face. Any change, including moving it, makes it fresh again.
6. **Files.** A Card holds a list of paths to files on this machine, added through the native Open dialog, opened with their default program, never copied.
7. **`#label` in the Capture box.** A word starting with `#` that names one of the Board's Labels attaches it and leaves the title.
8. **Ctrl+Left/Right moves a Column.** When the Selection is a Column with no Card, the Column moves instead.
9. **The open Card replaces the Card panel.** The selected Card expands in place inside its Column to show everything the panel showed. The panel is deleted.
10. **A visual pass.** One contrast scale per theme, full-height Columns, a real type hierarchy, quieter selection, an honest description preview, and a dark theme with distinct layers.

## User Stories

### Due date

1. As a user, I want to give a Card an optional due date, so that "when" lives on the Card and not in its title.
2. As a user, I want to type the date as `YYYY-MM-DD`, `DD/MM/YYYY` or `DD-MM-YYYY`, and to have a bad date refused with the field reverting, so that I never save nonsense.
3. As a user, I want to clear the due date by blanking the field, so that a Card can go back to having none.
4. As a user, I want the face of a Card with a due date to show a small badge like `28 Oct`, with the year added only when it isn't this year, so that I can scan dates without opening Cards.
5. As a user, I want the badge to turn amber when the Card is due today and red when it is overdue, so that late work stands out.
6. As a user, I want "today" to be judged in my local time zone at the moment the Board is drawn, so that a Card due today isn't red at 01:00.
7. As a user, I want the due date kept when a Card moves to another Board, is archived and restored, duplicated, exported and imported, so that it behaves like every other field.
8. As a user, I want the due date badge in the PNG image of the Board, so that the snapshot shows what the Board shows.
9. As a user, I want setting or clearing a due date to be one undo step, so that mistakes are cheap.

### Search

10. As a user, I want Ctrl+Shift+F (or "Search" in the sidebar) to open a Search view in place of the Columns, so that I can find a Card without remembering its Board.
11. As a user, I want to type text that is matched case-insensitively against every Card's title, description and Label names on every active Board, so that one box finds anything.
12. As a user, I want archived Cards and Cards on archived Boards included, marked "archived", so that nothing is out of reach.
13. As a user, I want results grouped under their Board's name, each result showing the Card's title, its Column (or "Archive"), and its Label chips, so that I can tell matches apart.
14. As a user, I want results to appear as I type, with no result until two characters are typed, so that the list is never the whole world.
15. As a user, I want to click a result (or move through results with Up/Down and press Enter) to open its Board with that Card selected and open, so that one keystroke takes me there.
16. As a user, I want picking an archived Card to open its Board's Archive view scrolled to it, so that I can restore it from there.
17. As a user, I want a result on an archived Board to be shown but not openable, with a tooltip saying to restore the Board first, so that the rule about archived Boards stays simple.
18. As a user, I want Esc or "Back to Board" to leave Search and return to the Board I was on, so that I can get back quickly.
19. As a user, I want Search to forget its text when I leave it, so that it opens clean every time.

### Collapse a Column

20. As a user, I want "Collapse" in a Column's menu to fold it to a narrow strip, so that a wide Board fits.
21. As a user, I want the strip to show the Column's count (or `count/limit`) and its name, with the full name in a tooltip, so that I still know what is in it.
22. As a user, I want a click anywhere on the strip to expand the Column again, so that unfolding is one action.
23. As a user, I want the fold remembered in the data file, so that the Board opens the way I left it.
24. As a user, I want to drop a Card on a collapsed Column and have it land at the bottom, so that folding doesn't block moving work.
25. As a user, I want Ctrl+Left/Right to move the selected Card into a collapsed Column at its bottom, and the Selection to land on the Column with no Card, so that the keyboard path still works.
26. As a user, I want Left/Right to skip over the Cards of a collapsed Column and select the Column itself, so that the Selection never points at something hidden.
27. As a user, I want a collapsed Column that is over its Column Limit to still show the warning colour on its count, so that folding hides Cards, not problems.
28. As a user, I want the PNG image to draw collapsed Columns as the same narrow strips, so that the image matches the Board.
29. As a user, I want collapsing and expanding to be undo steps, so that every Board change is covered.

### Duplicate a Card

30. As a user, I want "Duplicate" in the open Card (or Ctrl+D on the selected Card) to make a copy right below the original, so that repeating work is one action.
31. As a user, I want the copy to keep the title, description, Labels, due date, files and Checklist, with every Checklist Item unticked, so that a Card doubles as a template.
32. As a user, I want the copy to get new IDs, created and updated set to now, and to become the selected Card, so that it is a fresh Card and I see it.
33. As a user, I want duplicating to be one undo step, so that an accidental copy is one Ctrl+Z.

### Card ageing

34. As a user, I want a Card whose last change is more than 14 days ago to render faded on the face, so that stale work is visible at a glance.
35. As a user, I want moving a Card to another Column or Board, editing any field, ticking an Item or changing its Labels to count as a change, so that touching a Card un-fades it.
36. As a user, I want the selected Card and the open Card never faded, so that what I am working on is always readable.
37. As a user, I want fading in the PNG image too, so that the image matches the Board.

### Files

38. As a user, I want a "Files" list in the open Card with "+ Add file…" opening the native Open dialog, so that a Card can point at the document it is about.
39. As a user, I want Kanvibe to store only the path, never copy the file, so that nothing on disk is duplicated.
40. As a user, I want each file shown by its name with the full path in a tooltip, so that the list stays short.
41. As a user, I want a click on a file to open it with its default program, so that I get to the document in one click.
42. As a user, I want a file that no longer exists shown greyed with "not found" and not openable, so that I am told instead of guessing.
43. As a user, I want a ✕ on each file to remove it from the Card without a confirmation, since nothing on disk is touched and undo covers it.
44. As a user, I want the face to show a small `2 files` marker when a Card has files, so that I can see it without opening the Card.
45. As a user, I want files kept through move, archive, restore, duplicate, export and import, so that they behave like every other field.
46. As a user, I want adding or removing a file to be one undo step, so that mistakes are cheap.

### `#label` in the Capture box

47. As a user, I want a word `#name` in a Capture box title to attach the Board's Label called `name` (case-insensitive) and be removed from the title, so that capturing and labelling is one line.
48. As a user, I want several `#name` words in one title to attach several Labels, so that the syntax scales.
49. As a user, I want a `#name` that matches no Label to stay in the title as plain text, so that a typo never creates a Label.
50. As a user, I want the same syntax to apply to each line of a multi-line paste, so that bulk capture gets Labels too.
51. As a user, I want a title that is only `#name` words to keep the words as its title, so that what I typed is never thrown away.
52. As a user, I want a `#` inside a word (like `C#` or `item#3`) left alone, so that only a leading `#` means a Label.

### Ctrl+Left/Right moves a Column

53. As a user, I want Ctrl+Left/Right with a Column selected and no Card to move that Column one place, so that I can arrange the Board from the keyboard.
54. As a user, I want the Selection to stay on the moved Column and scroll into view, so that I can keep pressing.
55. As a user, I want Ctrl+Left on the first Column or Ctrl+Right on the last to do nothing and record no step, so that undo isn't full of no-ops.
56. As a user, I want the move to be one undo step, so that a wrong move is one Ctrl+Z.
57. As a user, I want Ctrl+Left/Right with a Card selected to keep moving the Card as today, so that nothing I know changes.

### The open Card

58. As a user, I want Enter on the selected Card, or a click on it, to expand it in place inside its Column instead of opening a panel, so that the Board keeps its full width.
59. As a user, I want the open Card to show, top to bottom: the title, the Label chips with "+ Add Label", the rendered description with its Edit button, the Checklist, the Files list, the due date field, and a footer with "Move to Board…", "Duplicate", "Archive" and the created and updated times, so that everything the panel showed is still there.
60. As a user, I want the open Card to be the selected Card and to follow the arrow keys, so that I can review Cards one after another as before.
61. As a user, I want Esc, a Board switch, or the Card leaving the Board to close it, and a click on empty Board space to leave it open, so that it doesn't vanish by accident.
62. As a user, I want F2, E, L and M to work in the open Card as they did in the panel, so that my hands don't relearn anything.
63. As a user, I want the open Card to render as a normal face in the PNG image, so that the image shows the Board, not my editing state.
64. As a user, I want the open Card to stay visible under a Filter it doesn't match, so that what I am working on never vanishes.
65. As a user, I want the Column to scroll so the whole open Card is in view when it opens, so that I don't hunt for the footer.
66. As a user, I want dragging an open Card to drag its compact face as the ghost, so that the ghost stays small.
67. As a user, I want the Keyboard shortcuts dialog to say "open" instead of "panel", and to list Ctrl+Shift+F (Search), Ctrl+D (Duplicate) and Ctrl+Left/Right on a Column, so that the help matches the app.

### The visual pass

68. As a user, I want three distinct layers in both themes: the page, the Column on it, and the Card on the Column, so that Cards read as objects.
69. As a user, I want Cards white with a hairline border and a one-pixel shadow in the light theme, on a Column a shade darker than the page, so that the Column reads as a tray and the Card as a ticket.
70. As a user, I want the dark theme to have its own palette with the same three steps, so that Cards don't vanish into their Column.
71. As a user, I want Columns to run the full height of the Board area, with the Card list scrolling inside and "+ Add Card" right after the last Card, so that the Board looks like a Board and not like notes on a wall.
72. As a user, I want a type hierarchy: Board title larger and semibold, Column names small and semibold in a muted colour, Card titles one size larger than today, so that the eye knows where to land.
73. As a user, I want a selected Column shown by a tint on its header, and only a selected Card to carry the two-pixel outline, so that the Selection is visible without shouting.
74. As a user, I want the description marker on the face replaced by the first line of the description in muted smaller text, clipped to one line, so that the face tells me something and nothing looks like a drag handle.
75. As a user, I want "+ Add Card" and "+ Add Column" to read as actions, in the text colour at reduced opacity that goes full on hover, so that they don't look disabled.
76. As a user, I want the count badge, Label chips, due badge, files marker and Checklist progress to share one small size and one row style, so that face metadata reads as one line.
77. As a user, I want the PNG image to use the same styles, so that it keeps matching the Board.
78. As a user, I want the shadow, radius and spacing values chosen once and used everywhere, so that nothing drifts.

### Data file

79. As a user, I want a v2 data file refused by v1 Kanvibe with "Install the newer version", so that v1 never silently drops due dates, files or folds.
80. As a user, I want a v1 data file to load into v2 unchanged, so that upgrading costs nothing.
81. As a user, I want an exported Board from v1 to import into v2, and a v2 export to be refused by v1 with a clear message, so that the file rules stay the same as for the data file.

## Implementation Decisions

### Domain

- Card gains `due: Option<String>` (an ISO calendar date, `YYYY-MM-DD`, no time), and `files: Vec<String>` (absolute paths as given by the dialog). Column gains `collapsed: bool`. All three default when absent and are omitted when empty, so a v1 file loads as is.
- The data file `version` becomes 2. The existing "newer file" refusal covers v1 opening a v2 file; the existing "older file" load path accepts v1 with no migration work. Board export carries the same version and the same rule.
- Due date parsing is a pure domain function from the typed text to an ISO date or nothing; it accepts the three formats in story 2 and nothing else. Comparison with today is also pure, taking today as an argument, so tests pass a fixed date.
- Ageing is a pure predicate on a Card's `updated` against a passed-in now, with the 14-day threshold a constant, not a setting. Moving a Card within or between Columns or Boards now touches it; today it does not.
- Duplicate is a domain operation returning the new Card's ID: a deep copy with fresh IDs for the Card and every Checklist Item, every Item set to not done, timestamps set to now, inserted right after the original in the same Column.
- `#label` parsing is a pure function from (title, the Board's Labels) to (clean title, Label IDs). A token is a `#` at a word start followed by one or more non-space characters; matching is case-insensitive and exact on the whole token. Unmatched tokens stay in the title. The Capture box calls it on submit and on each pasted line; `add_card` itself stays unaware of the syntax.
- Search is a pure function over all Boards returning hits of (Board ID, Board archived?, Card ID, Column name or Archive) in Board order then Column order then Card order, reusing the Filter's text predicate. It returns nothing for fewer than two trimmed characters.
- Moving a Column reuses the existing gap-based Column move; the keyboard arm computes the gap from the Column's current index.

### App state and Messages

- The Card panel state becomes a single `open: bool`: when true, the selected Card renders expanded. The panel module goes away; its pieces (labels row, description, checklist, times) move into the open Card's view inside the Column. "Open panel" and "close panel" messages become open and close of the Card. The validation that ran after each update keeps clearing `open` when the Selection has no Card.
- New Messages: set due text / commit due (the due field is a pending text edit like a rename, so it follows the one-pending-edit rule and the idle write); duplicate Card; add file (runs the native Open dialog as a task, like Import does), file chosen, remove file, open file; toggle Column collapse; open Search, Search text changed, Search move (Up/Down), Search pick, close Search; move Column (Direction). Ctrl+Shift+F and Ctrl+D are new arms in the key mapping; Ctrl+Left/Right maps to the existing move-Card message and the update arm branches on whether the Selection has a Card.
- Search is a view state like the Archive view: it replaces the Columns, owns a text input with a widget ID and an `Input` variant so the key mapping knows a text field is focused, and a highlighted index for Up/Down/Enter. Opening Search commits any pending edit; leaving it clears its text. Picking a hit on an active Board switches Board, sets the Selection to that Card and sets `open`; a hit in an Archive opens that Board's Archive view and reveals the Card.
- Collapsed Columns: the Selection may rest on a collapsed Column with no Card. Left/Right across a collapsed Column lands on the Column itself. The drop target of a collapsed Column is its whole strip and means "append". The strip is a fixed narrow width constant beside the Column width constant. iced has no rotated text, so the strip shows the count badge on top and the name below it clipped to the strip width, with the existing clipped-with-tooltip helper giving the full name.
- Key mapping order: Ctrl+Shift+F is matched before Ctrl+F, and Ctrl+D only when no text field is focused. Both are new arms in the mapping and new rows in the shortcuts dialog.
- Opening a file uses the same "open with the system" command the "Open data folder" button uses, with the path; a missing file is checked at view time with a plain existence test and rendered inert.
- Files are chosen with the dialog crate already in use; the dialog allows any file and returns one path per invocation.

### Styling

- Both themes become custom palettes built the way the light theme is today, with explicit colours for the page, the Column surface and the Card surface, so the three layers are chosen, not derived. The Column style loses its shadow and takes the darker-than-page fill; the Card style takes the lightest fill, a hairline border and a one-pixel shadow. The image export already shares these style functions and follows for free.
- Column height is fill; the Card list is the scrollable part; "+ Add Card" sits after the last Card inside the scrollable. The "+ Add Column" ghost matches the Column height.
- The face's description marker is replaced by the first non-blank line of the description, secondary colour, one size smaller, clipped to one line with no tooltip.
- Column selection renders as a header tint (primary at low alpha) in place of the outline; Card selection keeps the outline. The outlined style helper gains a header-tint sibling.
- Size constants: Board title 18, Column name 13, Card title 14, face metadata 12. Radius: Column 10, Card 6. Column gap 12, Card gap 6. These are named once beside the existing Column width constant.

### Glossary and docs

- Glossary: "Card panel" is replaced by **Open Card**: the selected Card, expanded in place to show all of its fields. "Search" is added: a view listing Cards from every Board that match typed text; distinct from Filter, which hides Cards on the current Board. "Due date", "File" and "Collapsed Column" are added. The shortcuts dialog and this spec's wording follow.
- A short ADR records the decision to expand the Card in place rather than in a panel or a modal, and the accepted cost: a 280px Column is narrow for long descriptions.

### Order of work

8 first (smallest, no UI change). Then 9, which deletes the panel and changes the layout. Then 10 on the new layout. Then 1 through 7 in any order; each is independent of the others.

## Testing Decisions

A good test builds an App, sends Messages through `update`, and asserts on the resulting domain data and UI state, never on widget trees. Pure domain functions are tested directly with fixed inputs, including a fixed "today" and "now".

- **Key mapping** (`tests/keys.rs` style): Ctrl+Shift+F, Ctrl+D, and the unchanged Ctrl+Left/Right mapping.
- **Update seam** (`tests/update.rs` style, the `app_with_cards` helper): Ctrl+Left/Right with a Column selected moves the Column and keeps the Selection; at an edge records no step. Enter opens the selected Card; arrows move the open Card; Esc closes it; a Board switch closes it. Duplicate inserts after the original with Items unticked and selects the copy. Collapse persists through a save/load round trip, Left/Right skips the collapsed Column's Cards, Ctrl+Right into it appends. Due text commits as ISO, bad text reverts, blank clears. Add/remove file round-trips and is one undo step. `#label` on submit attaches and strips; unmatched stays; all-tokens keeps the words as the title; applies per pasted line.
- **Domain, direct**: due parsing table (the three formats, a bad one, blank); due status against a fixed today (future, today, overdue); ageing against a fixed now at 13, 14 and 15 days; search across two Boards with an archived Card and an archived Board; `#label` parsing table including `C#` and `item#3`.
- **Storage** (`tests/storage.rs` style): a v1 file loads with all defaults; a v2 file is refused with the "newer" error by a loader given version 1; export and import carry due, files and collapsed.
- **Image** (`tests/image.rs` style): a collapsed Column renders narrow; the open Card renders at face height; the due badge is drawn. These check dimensions and a few pixels, as the existing image tests do.
- **Styling** has no behavioural test beyond the image dimensions; it is reviewed by eye in both themes.

## Out of Scope

Everything else from the commercial-app sweep: reminders and notifications, start dates, a calendar, table or timeline view, priority, custom fields, estimates, time tracking, card numbering, links and dependencies between Cards, sub-cards, Checklist Item dates, recurring Cards, swimlanes, Column colours, a "done" Column semantic, sorting a Column, bulk actions, Board backgrounds and templates, starred Boards and folders, saved filters, automations, a system-tray capture, CSV or PDF export, Trello import, calendar sync, plug-ins, zoom, extra themes, and multi-language. Also out: a date picker widget (typed dates only), dropping files onto a Card from Explorer (dialog only), copying attached files into the data folder, and a rich-text description editor (ADR 0003).

## Further Notes

- Due dates are dates, not datetimes, on purpose: a time implies a reminder, and there are none.
- Files store paths, so a moved or renamed file shows "not found". That is honest and cheap; copying files in would turn a 1 MB JSON app into a document store.
- The 280px Column is the known ceiling of the open Card. If long descriptions prove cramped, the follow-up is to widen the open Card's Column while it is open, not to bring back the panel.
- Search returns nothing under two characters so that a single letter never lists every Card on every Board.
- `#label` matches single-token Label names only. A Label called "In review" can't be typed as a token; that is accepted rather than inventing a quoting syntax.
- Parallel implementation: 8, 9, 2 and 7 touch different parts of the app and can be built at the same time. 10, 3, 1, 4, 6 and 5 all build on the open Card or the face styling, so they start after 9 has merged. All of them add to the Message enum and the key mapping; those merges are additive.
