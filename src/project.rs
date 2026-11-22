//! The project window: the Markdown tree beside the reading pane.

use std::path::PathBuf;

use rune::{Cx, El, Tx, col};
use rune_kit::shell::{PaneState, SplitView, Toolbar};
use rune_kit::{FileNode, FileTree, Kit, Tree, TreeItem, TreeModel, TreeStyle};

use crate::reader::{self, Document};
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
        };
        project.rescan();
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
        if let Some(file) = &self.selected_file {
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

    let reading = col().key("reading-pane").w_full().h_full().min_h(0.0).child(reader::view(p, k));

    SplitView::new(reading)
        .sidebar(&p.sidebar, |s: &mut App| &mut project_mut(s).sidebar, sidebar)
        .toolbar(Toolbar::new().title(p.name()))
        .titlebar_inset(cx.titlebar_inset())
        .build(k)
}
