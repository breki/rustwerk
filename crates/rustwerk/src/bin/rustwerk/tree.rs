use std::env;
use std::io::IsTerminal;

use anyhow::Result;

use rustwerk::domain::project::TreeNode;
use rustwerk::domain::task::Status;

use crate::load_project;

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
    std::io::stdout().is_terminal()
        && env::var_os("NO_COLOR").is_none()
}

/// Status indicator character.
fn status_char(status: Status) -> char {
    match status {
        Status::Done => '\u{2713}',     // ✓
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

/// Entry point for the `tree` command.
pub(super) fn cmd_tree(remaining: bool) -> Result<()> {
    let (_root, project) = load_project()?;
    let nodes = if remaining {
        project.task_tree_remaining()
    } else {
        project.task_tree()
    };
    render_tree(&project.metadata.name, &nodes, use_color());
    Ok(())
}

/// Render the dependency tree to stdout.
fn render_tree(
    name: &str,
    nodes: &[TreeNode],
    color: bool,
) {
    let bold = if color { ansi::BOLD } else { "" };
    let rst = if color { ansi::RESET } else { "" };
    println!("{bold}{name}{rst}");

    if nodes.is_empty() {
        println!("  (no tasks)");
        return;
    }

    for (i, node) in nodes.iter().enumerate() {
        let is_last = i == nodes.len() - 1;
        render_node(node, "", is_last, color);
    }
}

/// Render a single tree node and its children.
fn render_node(
    node: &TreeNode,
    prefix: &str,
    is_last: bool,
    color: bool,
) {
    let connector =
        if is_last { "\u{2514}\u{2500}\u{2500} " }
        else { "\u{251C}\u{2500}\u{2500} " };
    // └──  and  ├──

    let rst = if color { ansi::RESET } else { "" };

    match node {
        TreeNode::Reference { id, status } => {
            let ch = status_char(*status);
            let style =
                if color { status_style(*status) } else { "" };
            let dim =
                if color { ansi::DIM } else { "" };
            println!(
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
            let style =
                if color { status_style(*status) } else { "" };
            println!(
                "{prefix}{connector}\
                 {style}[{ch}]{rst} {style}{id}{rst}"
            );

            let child_prefix = if is_last {
                format!("{prefix}    ")
            } else {
                format!("{prefix}\u{2502}   ")
                // │
            };

            for (j, child) in children.iter().enumerate() {
                let child_is_last =
                    j == children.len() - 1;
                render_node(
                    child,
                    &child_prefix,
                    child_is_last,
                    color,
                );
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
        render_tree("Test", &[], false);
        // No panic — prints "(no tasks)"
    }

    #[test]
    fn render_single_task() {
        let nodes =
            vec![task_node("A", Status::Todo, vec![])];
        render_tree("Test", &nodes, false);
    }

    #[test]
    fn render_with_reference() {
        let nodes = vec![task_node(
            "A",
            Status::Done,
            vec![ref_node("B", Status::Todo)],
        )];
        render_tree("Test", &nodes, false);
    }

    #[test]
    fn render_with_color() {
        let nodes = vec![
            task_node(
                "A",
                Status::Done,
                vec![task_node(
                    "B",
                    Status::InProgress,
                    vec![],
                )],
            ),
            task_node("C", Status::Blocked, vec![]),
        ];
        render_tree("Test", &nodes, true);
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
