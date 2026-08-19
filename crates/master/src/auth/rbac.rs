//! Role-Based Access Control (RBAC)
//! Implements SRS §3.2.3: Authorization model for multi-user collaboration

use anyhow::{bail, Result};
use std::collections::HashMap;

/// Permission level for a user on a session
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permission {
    /// Full control (owner)
    Owner,
    /// Read and write access (can send input, resize)
    ReadWrite,
    /// Read-only access (can attach and view, but not send input)
    ReadOnly,
}

impl Permission {
    /// Parse permission from string (from ACL)
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "owner" => Ok(Permission::Owner),
            "editor" | "readwrite" | "rw" => Ok(Permission::ReadWrite),
            "viewer" | "readonly" | "ro" => Ok(Permission::ReadOnly),
            _ => bail!("Invalid permission: {}", s),
        }
    }

    /// Convert permission to string (for ACL storage)
    pub fn as_str(&self) -> &'static str {
        match self {
            Permission::Owner => "owner",
            Permission::ReadWrite => "editor",
            Permission::ReadOnly => "viewer",
        }
    }
}

/// Action that requires permission check
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Attach to session (view output)
    Read,
    /// Send input to session
    Write,
    /// Resize session terminal
    Resize,
    /// Terminate session
    Kill,
    /// Modify session ACL (share with others)
    Share,
}

/// Check if a user has permission to perform an action on a session
///
/// # Arguments
/// * `owner_user_id` - Session owner (from SessionRecord)
/// * `acl` - Access control list (user_email -> permission string)
/// * `user_id` - User attempting the action
/// * `action` - Action to perform
///
/// # Returns
/// Ok(()) if allowed, Err if denied
pub fn check_permission(
    owner_user_id: Option<&str>,
    acl: Option<&HashMap<String, String>>,
    user_id: &str,
    action: Action,
) -> Result<()> {
    // Owner always has full access
    if let Some(owner) = owner_user_id {
        if owner == user_id {
            return Ok(());
        }
    }

    // Check ACL for explicit permission
    let permission = if let Some(acl_map) = acl {
        acl_map.get(user_id)
            .and_then(|p| Permission::from_str(p).ok())
    } else {
        None
    };

    // Determine if permission level allows action
    match (permission, action) {
        // No permission in ACL
        (None, _) => bail!("User {} has no access to session", user_id),

        // Owner can do everything (already checked above, but for completeness)
        (Some(Permission::Owner), _) => Ok(()),

        // ReadWrite can read, write, resize
        (Some(Permission::ReadWrite), Action::Read) => Ok(()),
        (Some(Permission::ReadWrite), Action::Write) => Ok(()),
        (Some(Permission::ReadWrite), Action::Resize) => Ok(()),
        (Some(Permission::ReadWrite), Action::Kill) => {
            bail!("User {} does not have permission to kill session (requires owner)", user_id)
        }
        (Some(Permission::ReadWrite), Action::Share) => {
            bail!("User {} does not have permission to share session (requires owner)", user_id)
        }

        // ReadOnly can only read
        (Some(Permission::ReadOnly), Action::Read) => Ok(()),
        (Some(Permission::ReadOnly), _) => {
            bail!("User {} has read-only access, cannot perform {:?}", user_id, action)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_parsing() {
        assert_eq!(Permission::from_str("owner").unwrap(), Permission::Owner);
        assert_eq!(Permission::from_str("editor").unwrap(), Permission::ReadWrite);
        assert_eq!(Permission::from_str("viewer").unwrap(), Permission::ReadOnly);
        assert_eq!(Permission::from_str("ro").unwrap(), Permission::ReadOnly);
        assert!(Permission::from_str("invalid").is_err());
    }

    #[test]
    fn test_owner_full_access() {
        let owner = Some("alice@example.com");
        let acl = None;

        assert!(check_permission(owner, acl, "alice@example.com", Action::Read).is_ok());
        assert!(check_permission(owner, acl, "alice@example.com", Action::Write).is_ok());
        assert!(check_permission(owner, acl, "alice@example.com", Action::Kill).is_ok());
        assert!(check_permission(owner, acl, "alice@example.com", Action::Share).is_ok());
    }

    #[test]
    fn test_editor_permissions() {
        let owner = Some("alice@example.com");
        let mut acl = HashMap::new();
        acl.insert("bob@example.com".to_string(), "editor".to_string());

        assert!(check_permission(owner, Some(&acl), "bob@example.com", Action::Read).is_ok());
        assert!(check_permission(owner, Some(&acl), "bob@example.com", Action::Write).is_ok());
        assert!(check_permission(owner, Some(&acl), "bob@example.com", Action::Resize).is_ok());
        assert!(check_permission(owner, Some(&acl), "bob@example.com", Action::Kill).is_err());
        assert!(check_permission(owner, Some(&acl), "bob@example.com", Action::Share).is_err());
    }

    #[test]
    fn test_viewer_permissions() {
        let owner = Some("alice@example.com");
        let mut acl = HashMap::new();
        acl.insert("charlie@example.com".to_string(), "viewer".to_string());

        assert!(check_permission(owner, Some(&acl), "charlie@example.com", Action::Read).is_ok());
        assert!(check_permission(owner, Some(&acl), "charlie@example.com", Action::Write).is_err());
        assert!(check_permission(owner, Some(&acl), "charlie@example.com", Action::Resize).is_err());
        assert!(check_permission(owner, Some(&acl), "charlie@example.com", Action::Kill).is_err());
    }

    #[test]
    fn test_no_permission() {
        let owner = Some("alice@example.com");
        let acl = HashMap::new(); // Empty ACL

        assert!(check_permission(owner, Some(&acl), "eve@example.com", Action::Read).is_err());
    }
}
