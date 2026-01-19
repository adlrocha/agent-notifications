//! Attention detectors for CLI process monitoring
//!
//! Currently not used - kept for potential future enhancement.
//! The monitor uses a simple process-alive check instead.

use crate::models::Task;
use std::fs;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub enum AttentionReason {
    WaitingForInput,
    ProcessStalled,
    #[allow(dead_code)]
    Custom(String),
}

impl AttentionReason {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn as_str(&self) -> String {
        match self {
            AttentionReason::WaitingForInput => "Waiting for input".to_string(),
            AttentionReason::ProcessStalled => "Process stalled (no activity)".to_string(),
            AttentionReason::Custom(s) => s.clone(),
        }
    }
}

pub struct TaskContext {
    pub pid: i32,
    pub last_check: SystemTime,
    pub last_cpu_time: Option<u64>,
    pub idle_duration: Duration,
}

pub trait AttentionDetector: Send {
    fn check(&self, task: &Task, context: &TaskContext) -> Option<AttentionReason>;
}

/// Detector that checks if process is waiting on stdin
pub struct ProcessStateDetector;

impl ProcessStateDetector {
    pub fn new() -> Self {
        Self
    }

    fn check_process_state(&self, pid: i32) -> Option<String> {
        // Check if process is in "sleeping" state and waiting on stdin
        // Read from /proc/<pid>/stat
        let stat_path = format!("/proc/{}/stat", pid);
        let stat_content = fs::read_to_string(&stat_path).ok()?;

        // Parse the stat file (format: pid (comm) state ...)
        let parts: Vec<&str> = stat_content.split_whitespace().collect();
        if parts.len() < 3 {
            return None;
        }

        let state = parts[2]; // Third field is the state

        // Check if in 'S' (sleeping/interruptible) state
        if state == "S" {
            // Check file descriptors to see if stdin is being read
            let fd_path = format!("/proc/{}/fd/0", pid);
            if let Ok(link) = fs::read_link(&fd_path) {
                let link_str = link.to_string_lossy();
                // If stdin is connected to terminal and process is sleeping,
                // it might be waiting for input
                if link_str.contains("/dev/pts/") || link_str.contains("/dev/tty") {
                    return Some("waiting_input".to_string());
                }
            }
        }

        Some(state.to_string())
    }
}

impl AttentionDetector for ProcessStateDetector {
    fn check(&self, task: &Task, context: &TaskContext) -> Option<AttentionReason> {
        if let Some(state) = self.check_process_state(context.pid) {
            if state == "waiting_input" {
                // Additional checks to reduce false positives:
                // Only flag if task has been running for at least 10 seconds
                // AND idle for at least 5 seconds
                let task_age = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64
                    - task.created_at.timestamp();

                if task_age > 10 && context.idle_duration.as_secs() > 5 {
                    return Some(AttentionReason::WaitingForInput);
                }
            }
        }
        None
    }
}

/// Detector that checks if process has been inactive for too long
pub struct StallDetector {
    timeout: Duration,
}

impl StallDetector {
    pub fn new(timeout: Duration) -> Self {
        Self { timeout }
    }

    fn get_process_cpu_time(&self, pid: i32) -> Option<u64> {
        let stat_path = format!("/proc/{}/stat", pid);
        let stat_content = fs::read_to_string(&stat_path).ok()?;

        let parts: Vec<&str> = stat_content.split_whitespace().collect();
        if parts.len() < 15 {
            return None;
        }

        // Fields 13 and 14 are utime and stime (user and system CPU time)
        let utime: u64 = parts[13].parse().ok()?;
        let stime: u64 = parts[14].parse().ok()?;

        Some(utime + stime)
    }
}

impl AttentionDetector for StallDetector {
    fn check(&self, task: &Task, context: &TaskContext) -> Option<AttentionReason> {
        // Check if process CPU usage has changed since last check
        if let Some(current_cpu) = self.get_process_cpu_time(context.pid) {
            if let Some(last_cpu) = context.last_cpu_time {
                // If CPU time hasn't changed AND we've been idle past timeout
                if current_cpu == last_cpu && context.idle_duration > self.timeout {
                    // Additional check: ensure task has been running long enough
                    let task_age = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as i64
                        - task.created_at.timestamp();

                    if task_age > 30 {
                        return Some(AttentionReason::ProcessStalled);
                    }
                }
            }
        }

        None
    }
}

/// Detector that uses lsof to check if process is reading from stdin
#[allow(dead_code)]
pub struct StdinDetector;

impl StdinDetector {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self
    }

    #[allow(dead_code)]
    fn is_reading_stdin(&self, pid: i32) -> bool {
        // Use lsof to check if process has stdin open for reading
        let output = Command::new("lsof")
            .args(["-p", &pid.to_string(), "-a", "-d", "0"])
            .output();

        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // If lsof shows fd 0 (stdin) and it's a character device or pipe, check mode
            stdout.lines().count() > 1
        } else {
            false
        }
    }
}

impl AttentionDetector for StdinDetector {
    fn check(&self, task: &Task, context: &TaskContext) -> Option<AttentionReason> {
        // This is a more aggressive check than ProcessStateDetector
        // Only enable if lsof is available and we want detailed stdin tracking
        if self.is_reading_stdin(context.pid) {
            // Additional heuristic: check if process has been running for more than a few seconds
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;
            let task_age = now - task.created_at.timestamp();

            // If task is older than 30 seconds and still reading stdin, likely waiting
            if task_age > 30 {
                return Some(AttentionReason::WaitingForInput);
            }
        }
        None
    }
}

pub fn create_default_detectors() -> Vec<Box<dyn AttentionDetector>> {
    vec![
        Box::new(ProcessStateDetector::new()),
        Box::new(StallDetector::new(Duration::from_secs(600))), // 10 minutes
                                                                // StdinDetector is more invasive (requires lsof), so we exclude it by default
                                                                // Box::new(StdinDetector::new()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attention_reason_display() {
        assert_eq!(
            AttentionReason::WaitingForInput.as_str(),
            "Waiting for input"
        );
        assert_eq!(
            AttentionReason::ProcessStalled.as_str(),
            "Process stalled (no activity)"
        );
        assert_eq!(AttentionReason::Custom("Test".to_string()).as_str(), "Test");
    }

    #[test]
    fn test_detector_creation() {
        let detectors = create_default_detectors();
        assert_eq!(detectors.len(), 2); // ProcessState + Stall
    }
}
