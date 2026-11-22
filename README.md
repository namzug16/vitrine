# Vitrine

A read-only Markdown reader for macOS. Open a folder, browse its Markdown files in a tree, and
read them with tables, code, images and Mermaid diagrams. Vitrine never writes to your files.

Vitrine is a native app built with [Rune](https://github.com/anomalyco/rune). It has no
webview and needs no network.

## Features

- **Projects.** The welcome window keeps the folders you opened. A folder that moved shows
  "Folder not found", with Locate… and Remove.
- **File tree.** All `.md` and `.markdown` files in the folder, under the folder's name. The
  scan skips `.git` and `node_modules`, and shows hidden folders.
- **Reading.** Headings, lists, tables, code blocks, local images and links. A link to another
  Markdown file in the project opens it in the reader. A link to any other local file shows the
  file in Finder; Vitrine does not open it.
- **Mermaid diagrams.** [Pele](https://www.npmjs.com/package/pele) draws them on your
  Mac, without a browser. Click a diagram to expand it: drag to pan, scroll or pinch to zoom,
  and Esc to close.
- **Manual refresh.** Vitrine shows changes on disk only when you press Refresh (⌘R).
- **Git status.** For a file in a Git repository, the status bar shows the branch and whether
  the file is staged, modified, untracked, ignored, in conflict or in a stash. Vitrine runs
  `git` read-only.
- **Fonts.** The reading and code fonts are any font on your Mac. Departure Mono is the
  default, and Inter is the fallback when the font is missing.

## Requirements

- macOS 13 or later on Apple Silicon.
- To build: Rust (the version in `rust-toolchain.toml` installs automatically with `rustup`)
  and the Xcode command line tools.

## Build and install

```sh
make install     # build Vitrine.app and copy it to /Applications
```

| Command          | What it does                                             |
|------------------|----------------------------------------------------------|
| `make run`       | Run a debug build.                                       |
| `make build`     | Build the release binary.                                |
| `make check`     | Run clippy; warnings are errors.                         |
| `make app`       | Build `target/release/Vitrine.app`, signed ad hoc.       |
| `make install`   | Build the app and copy it to `/Applications`.            |
| `make uninstall` | Remove `/Applications/Vitrine.app`.                      |
| `make clean`     | Remove the build output.                                 |

To install in a different folder: `make install INSTALL_DIR=~/Applications`.

The app has an ad-hoc signature. It runs on the Mac that built it. Another Mac blocks it until
you allow it in System Settings > Privacy & Security.

## Keyboard shortcuts

| Shortcut | Action                          |
|----------|---------------------------------|
| ⌘O       | Open a folder                   |
| ⌘R       | Refresh the tree and the file   |
| ⌘W       | Close the project               |
| ⌘,       | Settings                        |
| ⌘= / ⌘-  | Make the text bigger or smaller |
| ⌘0       | Reset the text size             |
| Esc      | Close the expanded diagram      |

## Settings and data

Vitrine keeps its settings and the list of projects as JSON in
`~/Library/Application Support/Vitrine`. If a file is damaged, Vitrine renames it to
`*.json.bad` and starts again with the defaults.

## Tests

There are no unit tests. To check the UI, take screenshots of the app without a window, with
Rune's headless tools:

```sh
cargo run -- --size 1200x780 --snap home.png
```

`--script events.json` sends taps, keys and scrolls before the screenshot, and `--tree out.json`
writes the element tree.

## Licenses

Vitrine is MIT licensed. It includes Pele 1.13.0 (MIT, `assets/pele/LICENSE`) and icons from
[Lucide](https://lucide.dev) (ISC).
