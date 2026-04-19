use std::io::{self, Write};

use anyhow::Result;
use serde::Serialize;

use rustwerk::domain::developer::{Developer, DeveloperId};

use crate::render::RenderText;
use crate::{load_project, save_project};

/// Developer reference shared by add/remove outputs.
#[derive(Serialize)]
pub(crate) struct DevRef {
    pub(crate) id: DeveloperId,
    pub(crate) name: String,
}

/// `dev add` output.
#[derive(Serialize)]
pub(crate) struct DevAddOutput(pub(crate) DevRef);

impl RenderText for DevAddOutput {
    fn render_text(&self, w: &mut dyn Write) -> io::Result<()> {
        writeln!(w, "Added developer {}", self.0.id)
    }
}

pub(crate) fn cmd_dev_add(
    id: &str,
    name: &str,
    email: Option<&str>,
    role: Option<&str>,
) -> Result<DevAddOutput> {
    let (root, mut project) = load_project()?;
    let dev_id = DeveloperId::new(id)?;
    let mut dev = Developer::new(name)?;
    dev.email = email.map(String::from);
    dev.role = role.map(String::from);
    project.add_developer(dev_id.clone(), dev)?;
    save_project(&root, &project)?;
    Ok(DevAddOutput(DevRef {
        id: dev_id,
        name: name.to_string(),
    }))
}

/// `dev remove` output.
#[derive(Serialize)]
pub(crate) struct DevRemoveOutput(pub(crate) DevRef);

impl RenderText for DevRemoveOutput {
    fn render_text(&self, w: &mut dyn Write) -> io::Result<()> {
        writeln!(w, "Removed developer {}: {}", self.0.id, self.0.name)
    }
}

pub(crate) fn cmd_dev_remove(id: &str) -> Result<DevRemoveOutput> {
    let (root, mut project) = load_project()?;
    let dev_id = DeveloperId::new(id)?;
    let dev = project.remove_developer(&dev_id)?;
    save_project(&root, &project)?;
    Ok(DevRemoveOutput(DevRef {
        id: dev_id,
        name: dev.name,
    }))
}

/// One developer entry in `dev list`.
#[derive(Serialize)]
pub(crate) struct DevListItem {
    pub(crate) id: DeveloperId,
    pub(crate) name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) role: Option<String>,
}

/// `dev list` output.
#[derive(Serialize)]
pub(crate) struct DevListOutput {
    pub(crate) developers: Vec<DevListItem>,
}

impl RenderText for DevListOutput {
    fn render_text(&self, w: &mut dyn Write) -> io::Result<()> {
        if self.developers.is_empty() {
            return writeln!(w, "No developers.");
        }
        for dev in &self.developers {
            let role = dev
                .role
                .as_deref()
                .map_or(String::new(), |r| format!(" ({r})"));
            let email = dev
                .email
                .as_deref()
                .map_or(String::new(), |e| format!(" <{e}>"));
            writeln!(w, "  {}  {}{email}{role}", dev.id, dev.name)?;
        }
        Ok(())
    }
}

pub(crate) fn cmd_dev_list() -> Result<DevListOutput> {
    let (_root, project) = load_project()?;
    let developers = project
        .developers
        .into_iter()
        .map(|(id, dev)| DevListItem {
            id,
            name: dev.name,
            email: dev.email,
            role: dev.role,
        })
        .collect();
    Ok(DevListOutput { developers })
}
