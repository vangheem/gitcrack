# gitcrack

A read-only terminal UI for reviewing git changes. It opens in the repo that contains your current directory and starts on the working tree. You can switch to a commit, a branch compared with the default branch, or an open pull request without checking anything out.

## Install

Requires Rust, `git`, and `gh` (only for pull requests). `~/.local/bin` should be on your `PATH`.

```sh
make install
```

That builds a release binary and installs it to `~/.local/bin/gitcrack`. Use another prefix with `make install PREFIX=/some/path`.

From a checkout, `make run` starts it without installing. Other targets: `test`, `check`, `lint`, `fmt`, `clean`.

## Use

```sh
cd path/to/repo
gitcrack
```

The left pane is the file list. The right pane is the diff for the selected file.

| Key | Action |
| --- | --- |
| up / down, or tab / shift-tab | previous / next file |
| shift-up / shift-down, or k / j | scroll the diff |
| page up / page down | page the diff |
| space | page the diff |
| ctrl-u / ctrl-d | half-page |
| home / end | top / bottom of the diff |
| `[` / `]` | previous / next file |
| c | pick a commit |
| b | diff a branch against the default branch |
| p | list open pull requests via `gh` |
| w | back to the working tree |
| r | refresh |
| ? | help |
| q or ctrl-c | quit |

In a picker, up/down moves and enter opens the selection. Esc or q closes the picker. In the commit picker, space sets a range base; enter on another commit diffs that range.

The mouse wheel scrolls the diff. Drag to select text; the selection is copied to the clipboard. gitcrack does not commit, checkout, or otherwise change the repo.
