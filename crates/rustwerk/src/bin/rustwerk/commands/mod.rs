pub(crate) mod dev;
pub(crate) mod effort;
#[cfg(feature = "plugins")]
pub(crate) mod plugin;
pub(crate) mod project;
pub(crate) mod report;
pub(crate) mod task;

pub(crate) use dev::{cmd_dev_add, cmd_dev_list, cmd_dev_remove};
pub(crate) use effort::{cmd_effort_estimate, cmd_effort_log};
#[cfg(feature = "plugins")]
pub(crate) use plugin::{
    cmd_plugin_install, cmd_plugin_list, cmd_plugin_push, InstallScope,
};
pub(crate) use project::{cmd_init, cmd_show, cmd_status};
pub(crate) use report::{
    cmd_report_bottlenecks, cmd_report_complete, cmd_report_effort,
};
pub(crate) use task::{
    cmd_depend, cmd_task_add, cmd_task_assign, cmd_task_describe,
    cmd_task_list, cmd_task_remove, cmd_task_rename, cmd_task_status,
    cmd_task_unassign, cmd_task_update, cmd_undepend,
};

#[cfg(test)]
mod tests {
    use rustwerk::domain::task::Status;

    use crate::parse_status;

    #[test]
    fn parse_status_all_variants() {
        assert_eq!(parse_status("todo").unwrap(), Status::Todo);
        assert_eq!(parse_status("in-progress").unwrap(), Status::InProgress);
        assert_eq!(parse_status("in_progress").unwrap(), Status::InProgress);
        assert_eq!(parse_status("inprogress").unwrap(), Status::InProgress);
        assert_eq!(parse_status("blocked").unwrap(), Status::Blocked);
        assert_eq!(parse_status("done").unwrap(), Status::Done);
        assert_eq!(parse_status("TODO").unwrap(), Status::Todo);
        assert_eq!(parse_status("on-hold").unwrap(), Status::OnHold);
        assert_eq!(parse_status("on_hold").unwrap(), Status::OnHold);
        assert_eq!(parse_status("onhold").unwrap(), Status::OnHold);
    }

    #[test]
    fn parse_status_unknown() {
        assert!(parse_status("invalid").is_err());
    }
}
