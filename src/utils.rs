/// System utilities
pub mod system {
    use std::env;
    use std::fs;
    use std::io;
    use std::path::PathBuf;

    /// Gets the home directory from environment variables
    pub fn get_home_dir() -> Result<String, io::Error> {
        env::var("HOME").map_err(|e| io::Error::new(io::ErrorKind::NotFound, e))
    }

    /// Detects the current shell and returns the appropriate rc file path
    pub fn detect_shell_rc() -> Option<PathBuf> {
        let home_dir = get_home_dir().ok()?;
        let shell = env::var("SHELL").ok()?;

        let rc_file = if shell.contains("zsh") {
            ".zshrc"
        } else if shell.contains("bash") {
            ".bashrc"
        } else if shell.contains("fish") {
            ".config/fish/config.fish"
        } else {
            // Default to .bashrc for unknown shells
            ".bashrc"
        };

        Some(PathBuf::from(home_dir).join(rc_file))
    }

    /// Resolves a path to its absolute canonical form, expanding ~ to $HOME
    pub fn resolve_path(target: &str) -> Result<PathBuf, io::Error> {
        let expanded = if target == "~" {
            PathBuf::from(get_home_dir()?)
        } else if let Some(rest) = target.strip_prefix("~/") {
            PathBuf::from(get_home_dir()?).join(rest)
        } else {
            PathBuf::from(target)
        };

        std::fs::canonicalize(&expanded).map_err(|e| {
            let p = expanded.to_string_lossy();
            match e.kind() {
                io::ErrorKind::NotFound => {
                    io::Error::new(e.kind(), format!("'{}' does not exist", p))
                }
                io::ErrorKind::PermissionDenied => {
                    io::Error::new(e.kind(), format!("permission denied reading '{}'", p))
                }
                _ => io::Error::new(e.kind(), format!("cannot resolve '{}': {}", p, e)),
            }
        })
    }

    /// Gets the user applications directory (~/.local/share/applications/)
    pub fn get_user_applications_dir() -> Result<PathBuf, io::Error> {
        let home_dir = get_home_dir()?;
        let apps_dir = PathBuf::from(home_dir).join(".local/share/applications");

        // Create directory if it doesn't exist
        if !apps_dir.exists() {
            fs::create_dir_all(&apps_dir)?;
        }

        Ok(apps_dir)
    }
}
