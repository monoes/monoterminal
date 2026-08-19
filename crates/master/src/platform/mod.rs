// Platform-specific functionality
// Phase 3 Week 3: Cross-platform file paths
//
// Provides cross-platform abstractions for:
// - Data directories (system-wide and per-user)
// - Log directories
// - Database paths
//
// Platform support:
// - Windows: %ProgramData%, %LOCALAPPDATA%
// - Linux: /var/lib, ~/.local/share, XDG compliance
// - macOS: /Library/Application Support, ~/Library

pub mod paths;

pub use paths::{data_dir, user_data_dir, log_dir, session_db_path};
