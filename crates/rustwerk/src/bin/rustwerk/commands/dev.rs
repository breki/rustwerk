use anyhow::Result;

use rustwerk::domain::developer::{Developer, DeveloperId};

use crate::{load_project, save_project};

/// Add a developer to the project.
pub(crate) fn cmd_dev_add(
    id: &str,
    name: &str,
    email: Option<&str>,
    role: Option<&str>,
) -> Result<()> {
    let (root, mut project) = load_project()?;
    let dev_id =
        DeveloperId::new(id).map_err(|e| anyhow::anyhow!("{e}"))?;
    let mut dev = Developer::new(name)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    dev.email = email.map(String::from);
    dev.role = role.map(String::from);
    project
        .add_developer(dev_id.clone(), dev)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    save_project(&root, &project)?;
    println!("Added developer {dev_id}");
    Ok(())
}

/// Remove a developer from the project.
pub(crate) fn cmd_dev_remove(id: &str) -> Result<()> {
    let (root, mut project) = load_project()?;
    let dev_id =
        DeveloperId::new(id).map_err(|e| anyhow::anyhow!("{e}"))?;
    let dev = project
        .remove_developer(&dev_id)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    save_project(&root, &project)?;
    println!("Removed developer {dev_id}: {}", dev.name);
    Ok(())
}

/// List all developers in the project.
pub(crate) fn cmd_dev_list() -> Result<()> {
    let (_root, project) = load_project()?;
    if project.developers.is_empty() {
        println!("No developers.");
        return Ok(());
    }
    for (id, dev) in &project.developers {
        let role = dev
            .role
            .as_deref()
            .map_or(String::new(), |r| format!(" ({r})"));
        let email = dev
            .email
            .as_deref()
            .map_or(String::new(), |e| format!(" <{e}>"));
        println!("  {id}  {}{email}{role}", dev.name);
    }
    Ok(())
}
