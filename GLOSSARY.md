# Kanvibe

A personal, offline kanban app: one person organising their own work across several boards.

## Language

**Board**:
A named workspace holding an ordered set of Columns, its own Labels, and its Archive of Cards.
_Avoid_: Project, workspace, space

**Column**:
An ordered, named stage on a Board that holds Cards. No Column has special meaning; "Done" is just a name.
_Avoid_: List, lane, stage, status

**Card**:
A single piece of work on a Board, living in exactly one place: one Column while active, or the Board's Archive (remembering its original Column) while archived.
_Avoid_: Task, ticket, item, note, issue

**Label**:
A named, coloured tag defined on a Board and attached to any of that Board's Cards.
_Avoid_: Tag, category

**Checklist**:
The list of Checklist Items inside a Card.
_Avoid_: Subtasks, todo list

**Checklist Item**:
A line in a Checklist that is either done or not done.
_Avoid_: Subtask, step

**Column Limit**:
An optional maximum number of Cards for a Column; exceeding it is flagged, never blocked.
_Avoid_: WIP limit, cap

**Archive**:
Where archived Cards and Boards go: hidden from normal view but restorable. Cards and Boards are only ever destroyed by permanently deleting them from the Archive.
_Avoid_: Trash, bin, recycle bin

**Selection**:
The Column, and optionally one of its Cards, that keyboard actions act on. Shown with an outline.
_Avoid_: Focus, cursor, highlight

**Open Card**:
The selected Card, expanded in place to show all of its fields.
_Avoid_: Panel, detail view, modal, inspector

**Capture box**:
The one-line input at the bottom of a Column where a Card is created by typing its title.
_Avoid_: Quick add, composer, new card form

**Filter**:
Text and Labels that hide non-matching Cards on the current Board. Never saved, cleared on Board switch.
_Avoid_: Search, query
