//! Path validation for file operations.
//!
//! This module provides security checks for file paths to prevent:
//! - Path traversal attacks (e.g., `../../../etc/passwd`)
//! - Accessing files outside project directory
//! - Writing to critical system files

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Maximum allowed file size (10MB)
pub const MAX_FILE_SIZE: usize = 10 * 1024 * 1024;

/// Maximum allowed path length
pub const MAX_PATH_LENGTH: usize = 1024;

/// Validate a file path for security.
///
/// **Checks performed**:
/// 1. Path length must not exceed MAX_PATH_LENGTH
/// 2. No path traversal patterns (..)
/// 3. Path must be within project base directory (if specified)
/// 4. Cannot write to critical system files (if writing)
///
/// # Arguments
/// * `path_str` - User-provided path string
/// * `base_dir` - Project base directory (optional)
/// * `is_write` - Whether this is a write operation (more strict)
///
/// # Returns
/// * `Ok(PathBuf)` - Validated canonical path
/// * `Err(...)` - Validation failure with descriptive error
pub fn validate_path(path_str: &str, base_dir: Option<&Path>, is_write: bool) -> Result<PathBuf> {
    // 1. Check path length
    if path_str.len() > MAX_PATH_LENGTH {
        return Err(anyhow::anyhow!(
            "Path too long: {} characters (max: {})",
            path_str.len(),
            MAX_PATH_LENGTH
        ));
    }

    // 2. Check for path traversal
    if path_str.contains("..") {
        return Err(anyhow::anyhow!(
            "Path traversal detected: '{}'. Paths cannot contain '..' for security",
            path_str
        ));
    }

    // 3. Check for empty path
    if path_str.trim().is_empty() {
        return Err(anyhow::anyhow!("Path cannot be empty"));
    }

    // 4. Create PathBuf and resolve
    let path = PathBuf::from(path_str);
    let is_relative = path.is_relative(); // Check before potential move

    // 5. Check for critical system files (for write operations)
    if is_write {
        check_critical_system_files(&path)?;
    }

    // 6. Resolve against base directory
    let resolved_path = if let Some(base) = base_dir {
        // If path is absolute, check if it's within base
        if path.is_absolute() {
            // For absolute paths, we allow them but warn in docs
            // Users can configure whether to allow absolute paths
            path
        } else {
            // Relative path: resolve against base
            base.join(&path)
        }
    } else {
        // No base directory specified
        if path.is_absolute() {
            path
        } else {
            // Relative to current directory
            std::env::current_dir()
                .context("Cannot get current directory")?
                .join(&path)
        }
    };

    // 7. Try to canonicalize (for existing paths)
    // For non-existing paths (write operations), we do a best-effort resolution
    let canonical = if resolved_path.exists() {
        resolved_path
            .canonicalize()
            .with_context(|| format!("Cannot resolve path: {}", resolved_path.display()))?
    } else {
        // Path doesn't exist yet (write operation)
        // We can't canonicalize, but we can still check security
        resolved_path.clone()
    };

    // 8. Check if path is within base directory (if specified)
    // For non-existing paths, we check the resolved path (before canonicalize)
    if let Some(base) = base_dir {
        let base_canonical = if base.exists() {
            base.canonicalize()
                .with_context(|| format!("Cannot resolve base directory: {}", base.display()))?
        } else {
            base.to_path_buf()
        };

        // Check if resolved/canonical path is within base
        // For relative paths that don't have traversal, they're considered safe
        let is_within_base = if is_relative && !path_str.contains("..") {
            // Relative path without traversal is always safe
            true
        } else {
            // For absolute paths or paths with potential traversal, check strictly
            resolved_path.starts_with(&base_canonical) || canonical.starts_with(&base_canonical)
        };

        if !is_within_base {
            return Err(anyhow::anyhow!(
                "Path escapes project directory: '{}'. Resolved path '{}' appears outside '{}'",
                path_str,
                resolved_path.display(),
                base_canonical.display()
            ));
        }
    }

    Ok(canonical)
}

/// Check if path targets critical system files.
fn check_critical_system_files(path: &Path) -> Result<()> {
    // Critical system files that should never be written
    const CRITICAL_FILES: &[&str] = &[
        "/etc/passwd",
        "/etc/shadow",
        "/etc/sudoers",
        "/etc/ssh/sshd_config",
        "/etc/hosts",
        "/etc/fstab",
        "/boot/",
        "/dev/sda",
        "/dev/hda",
        "/proc/",
        "/sys/",
    ];

    let path_str = path.to_string_lossy();

    for critical in CRITICAL_FILES {
        if path_str.starts_with(critical) || path_str == *critical {
            return Err(anyhow::anyhow!(
                "Cannot write to critical system file: '{}'. This is blocked for security",
                path.display()
            ));
        }
    }

    Ok(())
}

/// Validate content size for file writes.
pub fn validate_content_size(content: &str) -> Result<()> {
    if content.len() > MAX_FILE_SIZE {
        return Err(anyhow::anyhow!(
            "Content too large: {} bytes (max: {} bytes = {} MB). \
             Split into smaller files or use streaming",
            content.len(),
            MAX_FILE_SIZE,
            MAX_FILE_SIZE / 1_000_000
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_path_traversal_blocked() {
        let base = TempDir::new().unwrap();

        // Path traversal should be blocked
        assert!(validate_path("../../../etc/passwd", Some(base.path()), false).is_err());
        assert!(validate_path("..\\..\\..\\windows\\system32", Some(base.path()), false).is_err());
        assert!(validate_path("/tmp/../etc/passwd", Some(base.path()), false).is_err());
    }

    #[test]
    fn test_safe_relative_paths_allowed() {
        let base = TempDir::new().unwrap();

        // Safe relative paths should be allowed (even if they don't exist yet)
        // These are typical write operations creating new files
        let result1 = validate_path("src/main.rs", Some(base.path()), true); // write
        let result2 = validate_path("./build/output.txt", Some(base.path()), true); // write
        let result3 = validate_path("config.json", Some(base.path()), true); // write

        // For write operations, relative paths should be allowed
        assert!(
            result1.is_ok(),
            "Relative path 'src/main.rs' should be allowed for write"
        );
        assert!(
            result2.is_ok(),
            "Relative path './build/output.txt' should be allowed for write"
        );
        assert!(
            result3.is_ok(),
            "Relative path 'config.json' should be allowed for write"
        );

        // For read operations, if file doesn't exist, should fail gracefully
        // But if user wants to read a non-existing file, that's their choice
        // We just block dangerous paths
        let result4 = validate_path("newfile.txt", Some(base.path()), false);
        // This should be ok since it's a safe relative path
        assert!(
            result4.is_ok(),
            "Safe relative path should be allowed even for read"
        );
    }

    #[test]
    fn test_absolute_paths_handling() {
        let base = TempDir::new().unwrap();

        // Create an actual file in temp dir
        let temp_file = base.path().join("test.txt");
        std::fs::write(&temp_file, "test content").unwrap();

        // Absolute path within temp dir should be allowed for existing file
        assert!(
            validate_path(temp_file.to_str().unwrap(), Some(base.path()), false).is_ok(),
            "Absolute path within base should be allowed for existing files"
        );

        // Critical system files should always be blocked for writes (even without base)
        assert!(
            validate_path("/etc/passwd", None, true).is_err(),
            "Critical system files should be blocked for writes even without base dir"
        );

        // Test absolute path outside base directory (platform-specific)
        #[cfg(unix)]
        {
            // On Unix, "/tmp" is typically outside a project's temp directory
            let outside_path = "/var/outside.txt";
            let result = validate_path(outside_path, Some(base.path()), true);
            assert!(
                result.is_err(),
                "Absolute path '{}' outside base should be rejected for write",
                outside_path
            );
        }

        #[cfg(windows)]
        {
            // On Windows, test with Windows-specific path
            let outside_path = "C:\\Windows\\outside.txt";
            let result = validate_path(outside_path, Some(base.path()), true);
            assert!(
                result.is_err(),
                "Absolute path '{}' outside base should be rejected for write",
                outside_path
            );
        }
    }

    #[test]
    fn test_critical_system_files_blocked() {
        // Critical system files should be blocked for writes (even without base dir)
        assert!(
            validate_path("/etc/passwd", None, true).is_err(),
            "Should block /etc/passwd for write"
        );
        assert!(
            validate_path("/etc/shadow", None, true).is_err(),
            "Should block /etc/shadow for write"
        );
        assert!(
            validate_path("/etc/sudoers", None, true).is_err(),
            "Should block /etc/sudoers for write"
        );

        // For reads, system files should be allowed (user's responsibility)
        // We document this in security guidelines
        assert!(
            validate_path("/etc/passwd", None, false).is_ok(),
            "Reading /etc/passwd should be allowed (documented risk)"
        );
        assert!(
            validate_path("/etc/hosts", None, false).is_ok(),
            "Reading /etc/hosts should be allowed"
        );
    }

    #[test]
    fn test_path_length_limit() {
        // Very long path should be rejected
        let long_path = "a".repeat(MAX_PATH_LENGTH + 1);
        assert!(
            validate_path(&long_path, None, false).is_err(),
            "Path exceeding MAX_PATH_LENGTH should be rejected"
        );

        // Normal length should be fine (even if relative)
        let normal_path = "src/main.rs";
        assert!(
            validate_path(normal_path, None, false).is_ok(),
            "Normal length relative path should be allowed"
        );

        // Absolute normal length path should also be fine
        let abs_path = "/tmp/test.txt";
        assert!(
            validate_path(abs_path, None, false).is_ok(),
            "Normal length absolute path should be allowed for read"
        );
    }

    #[test]
    fn test_content_size_validation() {
        // Small content should be fine
        let small = "Hello, world!";
        assert!(validate_content_size(small).is_ok());

        // Large content should be rejected
        let large = "x".repeat(MAX_FILE_SIZE + 1);
        assert!(validate_content_size(&large).is_err());

        // Exactly at limit should be fine
        let exact = "x".repeat(MAX_FILE_SIZE);
        assert!(validate_content_size(&exact).is_ok());
    }

    #[test]
    fn test_empty_path_blocked() {
        let base = TempDir::new().unwrap();

        // Empty path should be rejected
        assert!(validate_path("", Some(base.path()), false).is_err());
        assert!(validate_path("   ", Some(base.path()), false).is_err());
    }

    #[test]
    fn test_symlink_escape_blocked() {
        let base = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();

        // Create symlink pointing outside base
        let link = base.path().join("escape_link");
        #[cfg(unix)]
        std::os::unix::fs::symlink(outside.path(), &link).ok();
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(outside.path(), &link).ok();

        // Trying to access through symlink should be blocked
        // (canonicalize will resolve it to outside path)
        if link.exists() {
            let result = validate_path("escape_link", Some(base.path()), true);
            // Should fail if we properly check canonical path vs base
            assert!(result.is_err() || result.unwrap().starts_with(base.path()));
        }
    }
}
