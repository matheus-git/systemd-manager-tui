#![allow(dead_code)]
use chrono::DateTime;

/// Represents a systemd exec command specification as returned by D-Bus properties
/// like ExecStart, ExecStop, etc. Each tuple element corresponds to:
///
/// 1. path - The path to the executable
/// 2. args - Command line arguments
/// 3. ignore_exit_status - Whether to ignore the command's exit status
/// 4. start_timestamp - When the command was started (microseconds)
/// 5. exit_timestamp - When the command exited (microseconds)
/// 6. pid - Process ID of the command
/// 7. exit_code - Exit code of the process
/// 8. exit_status - Exit status of the process
/// 9. user_id - User ID the process runs as
/// 10. group_id - Group ID the process runs as
#[allow(clippy::upper_case_acronyms)]
pub type SASBTTUII = (String, Vec<String>, bool, u64, u64, u64, u64, u32, i32, i32);

#[derive(Debug, Clone)]
pub struct ServiceProperty {
    exec_start: Vec<SASBTTUII>,
    exec_start_pre: Vec<SASBTTUII>,
    exec_start_post: Vec<SASBTTUII>,
    exec_stop: Vec<SASBTTUII>,
    exec_stop_post: Vec<SASBTTUII>,

    exec_main_pid: u32,
    exec_main_start_timestamp: u64,
    exec_main_exit_timestamp: u64,
    exec_main_code: i32,
    exec_main_status: i32,

    main_pid: u32,
    control_pid: u32,

    restart: String,
    restart_usec: u64,

    status_text: String,
    result: String,

    user: String,
    group: String,

    limit_cpu: u64,
    limit_nofile: u64,
    limit_nproc: u64,
    limit_memlock: u64,
    memory_limit: u64,
    cpu_shares: u64,
}

impl ServiceProperty {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        exec_start: Vec<SASBTTUII>,
        exec_start_pre: Vec<SASBTTUII>,
        exec_start_post: Vec<SASBTTUII>,
        exec_stop: Vec<SASBTTUII>,
        exec_stop_post: Vec<SASBTTUII>,

        exec_main_pid: u32,
        exec_main_start_timestamp: u64,
        exec_main_exit_timestamp: u64,
        exec_main_code: i32,
        exec_main_status: i32,

        main_pid: u32,
        control_pid: u32,

        restart: String,
        restart_usec: u64,

        status_text: String,
        result: String,

        user: String,
        group: String,

        limit_cpu: u64,
        limit_nofile: u64,
        limit_nproc: u64,
        limit_memlock: u64,
        memory_limit: u64,
        cpu_shares: u64,
    ) -> Self {
        Self {
            exec_start,
            exec_start_pre,
            exec_start_post,
            exec_stop,
            exec_stop_post,

            exec_main_pid,
            exec_main_start_timestamp,
            exec_main_exit_timestamp,
            exec_main_code,
            exec_main_status,

            main_pid,
            control_pid,

            restart,
            restart_usec,

            status_text,
            result,

            user,
            group,

            limit_cpu,
            limit_nofile,
            limit_nproc,
            limit_memlock,
            memory_limit,
            cpu_shares,
        }
    }

    fn format_exec_field(&self, field: &[SASBTTUII]) -> String {
        field
            .iter()
            .map(|(_, args, _, _, _, _, _, _, _, _)| args.join(" ").to_string())
            .collect::<Vec<String>>()
            .join("\n")
    }

    pub fn formatted_exec_start(&self) -> String {
        self.format_exec_field(&self.exec_start)
    }

    pub fn formatted_exec_start_pre(&self) -> String {
        self.format_exec_field(&self.exec_start_pre)
    }

    pub fn formatted_exec_start_post(&self) -> String {
        self.format_exec_field(&self.exec_start_post)
    }

    pub fn formatted_exec_stop(&self) -> String {
        self.format_exec_field(&self.exec_stop)
    }

    pub fn formatted_exec_stop_post(&self) -> String {
        self.format_exec_field(&self.exec_stop_post)
    }

    pub fn format_timestamp(&self, timestamp: u64) -> String {
        let naive_datetime = DateTime::from_timestamp(timestamp as i64, 0);
        match naive_datetime {
            Some(dt) => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
            None => "".to_string(),
        }
    }
    #[allow(dead_code)]
    pub fn exec_start(&self) -> &Vec<SASBTTUII> {
        &self.exec_start
    }
    #[allow(dead_code)]
    pub fn exec_start_pre(&self) -> &Vec<SASBTTUII> {
        &self.exec_start_pre
    }
    #[allow(dead_code)]
    pub fn exec_start_post(&self) -> &Vec<SASBTTUII> {
        &self.exec_start_post
    }
    #[allow(dead_code)]
    pub fn exec_stop(&self) -> &Vec<SASBTTUII> {
        &self.exec_stop
    }
    #[allow(dead_code)]
    pub fn exec_stop_post(&self) -> &Vec<SASBTTUII> {
        &self.exec_stop_post
    }

    pub fn exec_main_pid(&self) -> u32 {
        self.exec_main_pid
    }
    pub fn exec_main_start_timestamp(&self) -> u64 {
        self.exec_main_start_timestamp
    }
    pub fn exec_main_exit_timestamp(&self) -> u64 {
        self.exec_main_exit_timestamp
    }
    pub fn exec_main_code(&self) -> i32 {
        self.exec_main_code
    }
    pub fn exec_main_status(&self) -> i32 {
        self.exec_main_status
    }

    pub fn main_pid(&self) -> u32 {
        self.main_pid
    }
    pub fn control_pid(&self) -> u32 {
        self.control_pid
    }

    pub fn restart(&self) -> &str {
        &self.restart
    }
    pub fn restart_usec(&self) -> u64 {
        self.restart_usec
    }

    pub fn status_text(&self) -> &str {
        &self.status_text
    }
    pub fn result(&self) -> &str {
        &self.result
    }

    pub fn user(&self) -> &str {
        &self.user
    }
    pub fn group(&self) -> &str {
        &self.group
    }

    pub fn limit_cpu(&self) -> u64 {
        self.limit_cpu
    }
    pub fn limit_nofile(&self) -> u64 {
        self.limit_nofile
    }
    pub fn limit_nproc(&self) -> u64 {
        self.limit_nproc
    }
    pub fn limit_memlock(&self) -> u64 {
        self.limit_memlock
    }
    pub fn memory_limit(&self) -> u64 {
        self.memory_limit
    }
    pub fn cpu_shares(&self) -> u64 {
        self.cpu_shares
    }
}
