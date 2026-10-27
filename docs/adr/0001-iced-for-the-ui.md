# iced for the UI

Kanvibe uses iced (Elm architecture: state, messages, `update`, `view`) instead of egui. egui has built-in drag and drop and would have been faster to a working board; we chose iced because a single `update` function over plain state makes undo/redo and testing straightforward. The cost we accepted: Card, Column and sidebar Board drag and drop is built ourselves on `mouse_area`, cursor subscriptions and `stack`, not taken from a library.
