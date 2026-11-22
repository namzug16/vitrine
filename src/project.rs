//! The project window: the Markdown tree beside the reading pane.

use std::path::PathBuf;

use rune::{Cx, El, Tx, col, row, text};
use rune_kit::shell::{PaneState, SplitView, Toolbar};
use rune_kit::{Badge, BadgeVariant, FileNode, FileTree, IconButton, IconButtonVariant, Kit, Tree, TreeItem, TreeModel, TreeStyle};

use crate::reader::{self, Document};
use crate::viewer::{self, Viewer};
use crate::git::{self, GitInfo};
use crate::{App, icons, scan};

pub struct Project {
    pub root: PathBuf,
    pub tree: TreeModel<FileNode<()>>,
    /// Project-relative path of the file in the reading pane.
    pub selected_file: Option<String>,
    pub document: Option<Document>,
    /// Why the folder couldn't be scanned, when it couldn't.
    pub scan_error: Option<String>,
    pub sidebar: PaneState,
    /// Git facts for the project and selected file; `None` outside Git.
    pub git: Option<GitInfo>,
    /// The expanded diagram, over the document.
    pub viewer: Option<Viewer>,
}

impl Project {
    pub fn open(root: PathBuf) -> Project {
        let mut project = Project {
            root,
            tree: TreeModel::new(),
            selected_file: None,
            document: None,
            scan_error: None,
            sidebar: PaneState::new(260.0).range(180.0, 480.0),
            viewer: None,
            git: None,
        };
        project.rescan();
        project.git = git::inspect(&project.root, None);
        project
    }

    pub fn name(&self) -> String {
        self.root.file_name().map_or_else(|| self.root.display().to_string(), |n| n.to_string_lossy().into_owned())
    }

    /// Rebuilds the tree from disk, keeping the folders that were open and still exist.
    pub fn rescan(&mut self) {
        let mut expanded = Vec::new();
        let mut pending: Vec<String> = self.tree.children(None).map(str::to_owned).collect();
        while let Some(id) = pending.pop() {
            if self.tree.is_expanded(&id) {
                pending.extend(self.tree.children(Some(&id)).map(str::to_owned));
                expanded.push(id);
            }
        }
        let files = match scan::markdown_files(&self.root) {
            Ok(files) => {
                self.scan_error = None;
                files
            }
            Err(e) => {
                self.scan_error = Some(e.to_string());
                Vec::new()
            }
        };
        self.tree = FileTree::new().compact_folders(false).build(files.into_iter().map(|path| (path, ())));
        for id in expanded {
            if self.tree.contains(&id) {
                self.tree.set_expanded(&id, true);
            }
        }
        if let Some(file) = &self.selected_file
            && self.tree.contains(file)
        {
            self.tree.select(file);
        }
    }
}

fn project(s: &App) -> &Project {
    s.project.as_ref().expect("project view needs an open project")
}

fn project_mut(s: &mut App) -> &mut Project {
    s.project.as_mut().expect("project view needs an open project")
}

fn select_file(s: &mut App, id: &str, tx: &mut Tx) {
    if project(s).tree.get(id).is_some_and(|n| !n.dir) {
        open_file(s, id, tx);
    }
}

/// Shows `relative` in the reading pane, selecting it in the tree when it's there.
pub fn open_file(s: &mut App, relative: &str, tx: &mut Tx) {
    let p = project_mut(s);
    if p.tree.contains(relative) {
        p.tree.reveal(relative, tx);
    }
    p.selected_file = Some(relative.to_owned());
    reload_selected(s, tx);
}

/// Reads the selected file again from disk.
pub fn reload_selected(s: &mut App, _: &mut Tx) {
    let cache = s.image_cache.as_ref().expect("image cache made in setup");
    let p = s.project.as_mut().expect("project view needs an open project");
    p.document = p.selected_file.as_deref().map(|relative| reader::load(&p.root, relative, cache));
    // shortcut: git runs on the UI thread; move it off if huge repositories make refresh lag.
    p.git = git::inspect(&p.root, p.selected_file.as_deref().map(|relative| p.root.join(relative)).as_deref());
}

/// Rescans the folder and rereads the selected file: the only way Vitrine sees changes on disk.
pub fn refresh(s: &mut App, tx: &mut Tx) {
    let Some(p) = s.project.as_mut() else { return };
    p.rescan();
    if p.selected_file.is_none() {
        p.git = git::inspect(&p.root, None);
    }
    // A file gone from disk leaves the tree with the rescan and shows the missing-file state.
    reload_selected(s, tx);
}

/// The selected file's name and path, and for Git projects the branch and the file's status.
fn status_bar(p: &Project, k: &Kit) -> El {
    let mut bar = row().key("status-bar").w_full().h(30.0).px(12.0).gap(8.0).center_y().border_sides([1.0, 0.0, 0.0, 0.0], k.c.border_weak);
    if let Some(relative) = &p.selected_file {
        let name = relative.rsplit('/').next().unwrap_or(relative);
        bar = bar
            .child(text(name.to_owned()).font(k.ty.small_medium).color(k.c.text_base))
            .child(text(relative.clone()).font(k.ty.small).color(k.c.text_muted).truncate().shrink());
    }
    bar = bar.child(el_grow());
    let Some(git) = &p.git else { return bar };
    bar = bar.child(text(git.head.clone()).font(k.ty.small).color(k.c.text_muted));
    if p.selected_file.is_some() && matches!(p.document, Some(reader::Document::Loaded { .. })) {
        let f = &git.file;
        let labels = [
            (f.conflicted, "Conflicted"),
            (f.untracked, "Untracked"),
            (f.ignored, "Ignored"),
            (f.staged, "Staged"),
            (f.modified, "Modified"),
        ];
        let mut any = false;
        for (on, label) in labels {
            if on {
                any = true;
                bar = bar.child(Badge::new(label).variant(if label == "Conflicted" { BadgeVariant::Accent } else { BadgeVariant::Neutral }).build(k));
            }
        }
        if !any {
            bar = bar.child(Badge::new("Clean").variant(BadgeVariant::Subtle).build(k));
        }
        if !git.stashes_with_file.is_empty() {
            bar = bar.child(Badge::new("In stash").variant(BadgeVariant::Subtle).build(k).help(git.stashes_with_file.join(", ")).key("in-stash"));
        }
    }
    bar
}

fn el_grow() -> El {
    rune::el().grow(1.0)
}

pub fn view(s: &App, k: &Kit, cx: &mut Cx<App>) -> El {
    let p = project(s);
    let tree = Tree::new(|s: &App| &project(s).tree, |s: &mut App| &mut project_mut(s).tree, |n: &FileNode<()>| n.name.as_str())
        .name("Markdown files")
        .style(TreeStyle::from_kit(k).chevron(&icons::CHEVRON_DOWN))
        .row(|n, r| {
            let icon = match (n.dir, r.expanded) {
                (true, Some(true)) => &icons::FOLDER_OPEN,
                (true, _) => &icons::FOLDER,
                (false, _) => &icons::FILE_TEXT,
            };
            TreeItem::new(n.name.clone()).icon(icon)
        })
        .on_select(select_file)
        .build(s, k)
        .key("tree")
        .grow(1.0)
        .min_h(0.0);
    let sidebar = col().w_full().h_full().min_h(0.0).child(tree);

    let diagrams = s.diagrams.as_ref().expect("diagrams start in setup");
    let reading = col()
        .w_full()
        .h_full()
        .min_h(0.0)
        .child(col().key("reading-pane").w_full().grow(1.0).min_h(0.0).child(reader::view(p, diagrams, &s.settings, cx.system_dark(), k)))
        .child(status_bar(p, k));

    let window = SplitView::new(reading)
        .sidebar(&p.sidebar, |s: &mut App| &mut project_mut(s).sidebar, sidebar)
        .toolbar(
            Toolbar::new().title(p.name()).trailing(
                IconButton::new(&icons::REFRESH)
                    .variant(IconButtonVariant::GhostMuted)
                    .label("Refresh")
                    .help("Refresh")
                    .shortcut("⌘R")
                    .on_press_tx(refresh)
                    .build(k)
                    .key("refresh"),
            ),
        )
        .titlebar_inset(cx.titlebar_inset())
        .build(k);
    // The sharper 4x bitmap once drawn, the inline 2x one until then.
    let expanded = p.viewer.as_ref().and_then(|v| {
        let dark = cx.system_dark();
        let sharp = diagrams.get(&reader::diagram_request(&v.source, &s.settings, dark, 4), k);
        let inline = || diagrams.get(&reader::diagram_request(&v.source, &s.settings, dark, 2), k);
        match sharp.or_else(inline) {
            Some(Ok(d)) => Some(viewer::view(v, &d, k)),
            _ => None,
        }
    });
    rune_kit::root(window, expanded)
}
