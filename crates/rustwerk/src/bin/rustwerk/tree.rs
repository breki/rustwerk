use std::env;
use std::io::{self, IsTerminal, Write};

use anyhow::Result;
use serde::Serialize;

use rustwerk::domain::project::TreeNode;
use rustwerk::domain::task::{Status, TaskId};

use crate::load_project;
use crate::render::RenderText;

/// ANSI color codes.
mod ansi {
    pub(super) const RESET: &str = "\x1b[0m";
    pub(super) const BOLD: &str = "\x1b[1m";
    pub(super) const DIM: &str = "\x1b[2m";
    pub(super) const GREEN: &str = "\x1b[32m";
    pub(super) const YELLOW: &str = "\x1b[33m";
    pub(super) const RED: &str = "\x1b[31m";
}

/// Check whether color output is enabled.
fn use_color() -> bool {
    io::stdout().is_terminal() && env::var_os("NO_COLOR").is_none()
}

/// Status indicator character.
fn status_char(status: Status) -> char {
    match status {
        Status::Done => '\u{2713}', // ✓
        Status::InProgress => '>',
        Status::Blocked => '!',
        Status::OnHold => '~',
        Status::Todo => ' ',
    }
}

/// ANSI style for a task status.
fn status_style(status: Status) -> &'static str {
    match status {
        Status::Done => ansi::GREEN,
        Status::InProgress => ansi::YELLOW,
        Status::Blocked => ansi::RED,
        Status::OnHold | Status::Todo => ansi::DIM,
    }
}

/// Serialized tree node.
#[derive(Serialize)]
pub(crate) struct TreeNodeDto {
    pub(crate) id: TaskId,
    pub(crate) status: Status,
    pub(crate) depth: usize,
    pub(crate) reference: bool,
    pub(crate) children: Vec<TreeNodeDto>,
}

fn to_dto(node: &TreeNode, depth: usize) -> TreeNodeDto {
    match node {
        TreeNode::Reference { id, status } => TreeNodeDto {
            id: id.clone(),
            status: *status,
            depth,
            reference: true,
            children: Vec::new(),
        },
        TreeNode::Task {
            id,
            status,
            children,
        } => TreeNodeDto {
            id: id.clone(),
            status: *status,
            depth,
            reference: false,
            children: children.iter().map(|c| to_dto(c, depth + 1)).collect(),
        },
    }
}

/// `tree` command output.
#[derive(Serialize)]
pub(crate) struct TreeOutput {
    pub(crate) name: String,
    pub(crate) nodes: Vec<TreeNodeDto>,
    #[serde(skip_serializing)]
    raw: Vec<TreeNode>,
}

impl RenderText for TreeOutput {
    fn render_text(&self, w: &mut dyn Write) -> io::Result<()> {
        render_tree(w, &self.name, &self.raw, use_color());
        Ok(())
    }
}

/// Entry point for the `tree` command.
pub(super) fn cmd_tree(remaining: bool) -> Result<TreeOutput> {
    let (_root, project) = load_project()?;
    let raw = if remaining {
        project.task_tree_remaining()
    } else {
        project.task_tree()
    };
    let nodes = raw.iter().map(|n| to_dto(n, 0)).collect();
    Ok(TreeOutput {
        name: project.metadata.name,
        nodes,
        raw,
    })
}

/// Render the dependency tree to a writer.
fn render_tree(w: &mut dyn Write, name: &str, nodes: &[TreeNode], color: bool) {
    let bold = if color { ansi::BOLD } else { "" };
    let rst = if color { ansi::RESET } else { "" };
    let _ = writeln!(w, "{bold}{name}{rst}");

    if nodes.is_empty() {
        let _ = writeln!(w, "  (no tasks)");
        return;
    }

    for (i, node) in nodes.iter().enumerate() {
        let is_last = i == nodes.len() - 1;
        render_node(w, node, "", is_last, color);
    }
}

/// Render a single tree node and its children.
fn render_node(
    w: &mut dyn Write,
    node: &TreeNode,
    prefix: &str,
    is_last: bool,
    color: bool,
) {
    let connector = if is_last {
        "\u{2514}\u{2500}\u{2500} " // └──
    } else {
        "\u{251C}\u{2500}\u{2500} " // ├──
    };

    let rst = if color { ansi::RESET } else { "" };

    match node {
        TreeNode::Reference { id, status } => {
            let ch = status_char(*status);
            let style = if color { status_style(*status) } else { "" };
            let dim = if color { ansi::DIM } else { "" };
            let _ = writeln!(
                w,
                "{prefix}{connector}\
                 {style}[{ch}]{rst} \
                 {dim}{id} \u{2192} (see above){rst}"
            );
        }
        TreeNode::Task {
            id,
            status,
            children,
        } => {
            let ch = status_char(*status);
            let style = if color { status_style(*status) } else { "" };
            let _ = writeln!(
                w,
                "{prefix}{connector}\
                 {style}[{ch}]{rst} {style}{id}{rst}"
            );

            let child_prefix = if is_last {
                format!("{prefix}    ")
            } else {
                format!("{prefix}\u{2502}   ") // │
            };

            for (j, child) in children.iter().enumerate() {
                let child_is_last = j == children.len() - 1;
                render_node(w, child, &child_prefix, child_is_last, color);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustwerk::domain::task::TaskId;

    fn task_node(
        id: &str,
        status: Status,
        children: Vec<TreeNode>,
    ) -> TreeNode {
        TreeNode::Task {
            id: TaskId::new(id).unwrap(),
            status,
            children,
        }
    }

    fn ref_node(id: &str, status: Status) -> TreeNode {
        TreeNode::Reference {
            id: TaskId::new(id).unwrap(),
            status,
        }
    }

    #[test]
    fn render_empty_shows_no_tasks() {
        let mut buf = Vec::new();
        render_tree(&mut buf, "Test", &[], false);
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("Test"));
        assert!(out.contains("(no tasks)"));
    }

    #[test]
    fn render_single_task() {
        let nodes = vec![task_node("A", Status::Todo, vec![])];
        let mut buf = Vec::new();
        render_tree(&mut buf, "Test", &nodes, false);
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("A"), "output: {out}");
        assert!(out.contains("[ ]"), "todo indicator: {out}");
    }

    #[test]
    fn render_with_reference() {
        let nodes = vec![task_node(
            "A",
            Status::Done,
            vec![ref_node("B", Status::Todo)],
        )];
        let mut buf = Vec::new();
        render_tree(&mut buf, "Test", &nodes, false);
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("see above"), "reference: {out}");
    }

    #[test]
    fn render_with_color() {
        let nodes = vec![
            task_node(
                "A",
                Status::Done,
                vec![task_node("B", Status::InProgress, vec![])],
            ),
            task_node("C", Status::Blocked, vec![]),
        ];
        let mut buf = Vec::new();
        render_tree(&mut buf, "Test", &nodes, true);
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains(ansi::GREEN), "green for done: {out}");
        assert!(out.contains(ansi::RED), "red for blocked: {out}");
    }

    #[test]
    fn render_box_drawing_chars() {
        let nodes = vec![
            task_node(
                "A",
                Status::Todo,
                vec![task_node("B", Status::Todo, vec![])],
            ),
            task_node("C", Status::Todo, vec![]),
        ];
        let mut buf = Vec::new();
        render_tree(&mut buf, "P", &nodes, false);
        let out = String::from_utf8(buf).unwrap();
        // ├── for non-last, └── for last
        assert!(out.contains('\u{251C}'), "branch: {out}");
        assert!(out.contains('\u{2514}'), "corner: {out}");
        // │ for continuation
        assert!(out.contains('\u{2502}'), "vertical: {out}");
    }

    #[test]
    fn status_chars_all_variants() {
        assert_eq!(status_char(Status::Done), '\u{2713}');
        assert_eq!(status_char(Status::InProgress), '>');
        assert_eq!(status_char(Status::Blocked), '!');
        assert_eq!(status_char(Status::OnHold), '~');
        assert_eq!(status_char(Status::Todo), ' ');
    }
}
