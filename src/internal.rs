/// Configuration management
pub mod config {
    use serde_json::Value;
    use std::env;
    use std::error;
    use std::fs;
    use std::io;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::process;

    pub use crate::utils::system;

    const CONFIG_FILE: &str = ".config/binx.json";
    const DEFAULT_CONFIG: &str = r#"{
      "aliases": {}
    }"#;

    const INSTALL_DIR: &str = ".binx";
    const INSTALL_EXECUTABLE: &str = "binx";

    /// Gets the configuration file path
    pub fn get_config_file_path() -> Result<String, io::Error> {
        let home_dir = system::get_home_dir()?;
        Ok(format!("{}/{}", home_dir, CONFIG_FILE))
    }

    const DESKTOP_PREFIX: &str = "binx_";

    /// Returns the .desktop filename with prefix for an alias
    fn desktop_filename(alias: &str) -> String {
        format!("{}{}", DESKTOP_PREFIX, alias)
    }

    /// Gets the installation directory path (~/.binx)
    pub fn get_install_dir() -> Result<PathBuf, io::Error> {
        let home_dir = system::get_home_dir()?;
        let install_dir = PathBuf::from(home_dir).join(INSTALL_DIR);
        // Create installation directory if it doesn't exist
        if !install_dir.exists() {
            fs::create_dir_all(&install_dir)?;
        }
        Ok(install_dir)
    }

    /// Gets the installation executable path (~/.binx/binx)
    pub fn get_install_executable_path() -> Result<PathBuf, io::Error> {
        let install_dir = get_install_dir()?;
        Ok(install_dir.join(INSTALL_EXECUTABLE))
    }

    /// Gets the current executable path
    pub fn get_current_executable_path() -> Result<PathBuf, io::Error> {
        env::current_exe().map_err(|e| io::Error::new(io::ErrorKind::NotFound, e))
    }

    /// Checks if the current executable is running from the installation path
    pub fn is_running_from_installation() -> bool {
        match (get_current_executable_path(), get_install_executable_path()) {
            (Ok(current), Ok(install)) => current == install,
            _ => false,
        }
    }

    /// Gets the version of the installed executable by running it with --version
    fn get_installed_version() -> Option<String> {
        let install_path = get_install_executable_path().ok()?;
        
        use std::process::Command;
        let output = Command::new(&install_path)
            .arg("--version")
            .output()
            .ok()?;
        
        if output.status.success() {
            let version_output = String::from_utf8_lossy(&output.stdout);
            // Parse version from output like "binx v0.1.0"
            version_output
                .trim()
                .strip_prefix("binx v")
                .map(|v| v.to_string())
        } else {
            None
        }
    }

    /// Checks if the installed version is outdated compared to the current version
    pub fn is_version_outdated(current_version: &str) -> bool {
        if let Some(installed_version) = get_installed_version() {
            // TODO: Compare versions properly
            installed_version != current_version
        } else {
            // If we can't get the version, assume it needs update
            true
        }
    }

    /// Checks if the installation exists and is valid
    fn installation_exists() -> bool {
        match get_install_executable_path() {
            Ok(path) => path.exists() && path.is_file(),
            Err(_) => false,
        }
    }

    /// Performs the installation by moving the executable to ~/.binx/binx
    pub fn perform_installation() -> Result<(), io::Error> {
        let install_path = get_install_executable_path()?;
        let current_path = get_current_executable_path()?;

        // Copy the current executable to the installation path
        fs::copy(&current_path, &install_path)?;

        // Set executable permissions
        let mut perms = fs::metadata(&install_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&install_path, perms)?;

        println!("{:?} ✔", install_path);

        Ok(())
    }

    /// Adds the installation directory to PATH in the shell's rc file
    pub fn add_to_path() -> Result<(), io::Error> {
        let install_dir = get_install_dir()?;
        let install_dir_str = install_dir.to_string_lossy().to_string();
        let rc_path = system::detect_shell_rc().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "Could not detect shell rc file",
            )
        })?;

        // Check if PATH already contains the installation directory
        if let Ok(path) = env::var("PATH") {
            if path.split(':').any(|p| p == install_dir_str) {
                println!("PATH ✔");
                return Ok(());
            }
        }

        // Read the rc file content
        let content = fs::read_to_string(&rc_path).unwrap_or_default();

        // Check if the PATH modification already exists in the rc file
        let path_line = format!("export PATH=\"$PATH:{}\"", install_dir_str);
        if content.contains(&install_dir_str) {
            return Ok(());
        }

        // Append the PATH modification to the rc file
        let new_content = if content.is_empty() {
            format!("{}\n", path_line)
        } else {
            format!("{}\n{}\n", content, path_line)
        };

        fs::write(&rc_path, new_content)?;
        println!("Added {:?} to PATH in {:?}", install_dir, rc_path);
        println!("Please run: source {:?}", rc_path);

        Ok(())
    }

    /// Runs the auto-installation check and performs installation if needed
    pub fn run_auto_installation(current_version: &str) -> Result<(), io::Error> {
        // If running from installation, check if version is outdated
        if is_running_from_installation() {
            if is_version_outdated(current_version) {
                println!("Installed version is outdated, updating...");
                perform_installation()?;
                add_to_path()?;
            }
            return Ok(());
        }

        // If installation exists, check if version is outdated
        if installation_exists() {
            if is_version_outdated(current_version) {
                println!("Installed version is outdated, updating...");
                perform_installation()?;
                add_to_path()?;
            } else {
                println!("{:?} ✔", get_install_dir()?);
            }
            return Ok(());
        }

        // Installation doesn't exist, perform installation
        println!("Performing auto-installation...");
        perform_installation()?;
        add_to_path()?;

        Ok(())
    }

    /// Generates the shell script content for executing a target via binx
    pub fn generate_script_content(alias: &str) -> Result<String, io::Error> {
        let install_dir = get_install_executable_path()?;
        Ok(format!(
            "#!/bin/bash\n# Script generated by binx\nexec {} {} \"$@\"",
            install_dir.display(),
            alias
        ))
    }

    /// Installs a script for the target in ~/.binx/ with executable permissions
    /// and adds the script path to the config
    pub fn install_target_script(alias: &str) -> Result<(), Box<dyn error::Error>> {
        let install_dir = get_install_dir()?;
        let script_path = install_dir.join(alias);

        // Generate script content
        let script_content = generate_script_content(alias)?;

        // Write script (overwrite if exists)
        fs::write(&script_path, script_content)?;

        // Set executable permissions
        let mut perms = fs::metadata(&script_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms)?;

        // Add script path to config
        let mut config = get_config()?;
        let script_path_str = script_path.to_string_lossy().to_string();

        if let Some(aliases) = config.get_mut("aliases").and_then(|a| a.as_object_mut()) {
            if let Some(alias) = aliases.get_mut(alias) {
                // Update existing alias with script path
                if let Some(alias_obj) = alias.as_object_mut() {
                    alias_obj.insert("script".to_string(), serde_json::json!(script_path_str));
                }
            }
        }

        save_config(&config)?;
    
        Ok(())
    }

    /// Finds an icon file (.png or .svg) with the same name as target in the target's directory
    pub fn find_icon_file(target_path: &PathBuf) -> Option<PathBuf> {
        let base_name = target_path.file_stem()?.to_str()?;
        let target_dir = target_path.parent()?;

        let svg_path = target_dir.join(format!("{}.svg", base_name));
        if svg_path.exists() {
            return Some(svg_path);
        }
        
        let png_path = target_dir.join(format!("{}.png", base_name));
        if png_path.exists() {
            return Some(png_path);
        }

        None
    }



    /// Resolves the Exec and TryExec paths for a desktop entry.
    pub fn resolve_desktop_exec(
        alias: &str,
    ) -> Result<(String, String), Box<dyn error::Error>> {
        let binx_exec_path = get_install_executable_path()?.to_string_lossy().to_string();
        Ok((
            format!("{} {}", binx_exec_path, alias),
            binx_exec_path,
        ))
    }

    /// Installs an icon into the user icon theme and returns the icon name for the .desktop file
    pub fn install_icon_file(
        target: &str,
        icon_path: &PathBuf,
    ) -> Result<String, Box<dyn error::Error>> {
        let home_dir = system::get_home_dir()?;
        let icons_base = PathBuf::from(&home_dir).join(".local/share/icons/hicolor");

        let extension = icon_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("png");

        let dest_dir = if extension == "svg" {
            icons_base.join("scalable/apps")
        } else {
            icons_base.join("256x256/apps")
        };

        fs::create_dir_all(&dest_dir)?;
        let dest_path = dest_dir.join(format!("{}.{}", target, extension));
        fs::copy(icon_path, &dest_path)?;

        match process::Command::new("gtk-update-icon-cache")
            .args(["-f", "-t"])
            .arg(&icons_base)
            .output()
        {
            Ok(output) if output.status.success() => {
                println!("Icon cache updated");
            }
            Ok(output) => {
                eprintln!(
                    "Warning: Icon cache update failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            Err(e) => {
                eprintln!("Warning: Could not run gtk-update-icon-cache: {}", e);
            }
        }

        Ok(target.to_string())
    }
    /// Generates the .desktop file content
    pub fn generate_desktop_content(
        alias: &str,
        exec_path: &str,
        try_exec_path: &str,
        icon_name: &str,
    ) -> String {
        // Capitalize first letter for Name
        let display_name = alias
            .chars()
            .enumerate()
            .map(|(i, c)| {
                if i == 0 {
                    c.to_uppercase().collect::<String>()
                } else {
                    c.to_string()
                }
            })
            .collect::<String>();

        format!(
            "[Desktop Entry]\n\
Type=Application\n\
Name={}\n\
Comment=Launch {} via binx\n\
Exec={}\n\
TryExec={}\n\
Icon={}\n\
Terminal=true\n\
NoDisplay=false\n\
Categories=Utility;\n",
            display_name, alias, exec_path, try_exec_path, icon_name
        )
    }

    /// Installs a .desktop file for the target in the applications directory
    pub fn install_desktop_file(
        alias: &str,
        target_path: &PathBuf,
    ) -> Result<(), Box<dyn error::Error>> {
        let apps_dir = system::get_user_applications_dir()?;
        let desktop_name = desktop_filename(alias);
        let desktop_file_path = apps_dir.join(format!("{}.desktop", desktop_name));

        let (exec_path, try_exec_path) = resolve_desktop_exec(alias)?;

        // Install icon into the user icon theme when available
        let icon_name = if let Some(icon_path) = find_icon_file(target_path) {
            install_icon_file(alias, &icon_path)?
        } else {
            "application-x-executable".to_string()
        };

        let desktop_content =
            generate_desktop_content(alias, &exec_path, &try_exec_path, &icon_name);

        fs::write(&desktop_file_path, &desktop_content)?;
        let mut perms = fs::metadata(&desktop_file_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&desktop_file_path, perms)?;
        println!("Installed .desktop file: {:?}", desktop_file_path);

        // Validate the installed desktop file
        match process::Command::new("desktop-file-validate")
            .arg(&desktop_file_path)
            .output()
        {
            Ok(output) if !output.status.success() => {
                eprintln!(
                    "Warning: Desktop file validation failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            _ => {}
        }

        let mut config = get_config()?;
        let desktop_file_path_str = desktop_file_path.to_string_lossy().to_string();

        if let Some(aliases) = config.get_mut("aliases").and_then(|a| a.as_object_mut()) {
            if let Some(alias_obj) = aliases.get_mut(alias).and_then(|a| a.as_object_mut()) {
                alias_obj.insert("desktop".to_string(), serde_json::json!(desktop_file_path_str));
            }
        }

        save_config(&config)?;

        // Update desktop database so launchers pick up the new entry
        println!("Updating desktop database...");
        match process::Command::new("update-desktop-database")
            .arg(&apps_dir)
            .output()
        {
            Ok(output) if output.status.success() => {
                println!("Desktop database updated successfully");
            }
            Ok(output) => {
                eprintln!(
                    "Warning: Desktop database update failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            Err(e) => {
                eprintln!("Warning: Could not run update-desktop-database: {}", e);
            }
        }

        Ok(())
    }

    /// Removes an alias and all associated files (script, .desktop, icon)
    pub fn remove_alias(alias: &str) -> Result<(), Box<dyn error::Error>> {
        let mut config = get_config()?;
        let aliases = config.get("aliases").and_then(|a| a.as_object());

        let alias_data = match aliases.and_then(|a| a.get(alias)) {
            Some(data) => data.clone(),
            None => {
                println!("Alias '{}' not found", alias);
                return Ok(());
            }
        };

        // Remove script from ~/.binx/
        if let Some(script_path) = alias_data.get("script").and_then(|s| s.as_str()) {
            let p = PathBuf::from(script_path);
            if p.exists() {
                fs::remove_file(&p)?;
                println!("Removed script: {:?}", p);
            } else {
                println!("Script not found: {:?}", p);
            }
        }

        // Remove .desktop file
        let apps_dir = system::get_user_applications_dir()?;
        let desktop_name = desktop_filename(alias);
        let desktop_file_path = apps_dir.join(format!("{}.desktop", desktop_name));
        if desktop_file_path.exists() {
            fs::remove_file(&desktop_file_path)?;
            println!("Removed .desktop file: {:?}", desktop_file_path);

            // Update desktop database
            match process::Command::new("update-desktop-database")
                .arg(&apps_dir)
                .output()
            {
                Ok(output) if output.status.success() => {
                    println!("Desktop database updated");
                }
                _ => {}
            }
        }

        // Remove icon if installed
        if let Some(icon_name) = alias_data.get("icon").and_then(|s| s.as_str()) {
            let home_dir = system::get_home_dir()?;
            let icons_base = PathBuf::from(&home_dir).join(".local/share/icons/hicolor");

            for dir in &["scalable/apps", "256x256/apps"] {
                for ext in &["svg", "png"] {
                    let icon_path = icons_base.join(dir).join(format!("{}.{}", icon_name, ext));
                    if icon_path.exists() {
                        fs::remove_file(&icon_path)?;
                        println!("Removed icon: {:?}", icon_path);
                    }
                }
            }
        }

        // Remove from config
        if let Some(aliases) = config.get_mut("aliases").and_then(|a| a.as_object_mut()) {
            aliases.remove(alias);
        }

        save_config(&config)?;
        println!("Removed alias '{}'", alias);

        Ok(())
    }

    pub fn get_config() -> Result<Value, Box<dyn error::Error>> {
        let config_path = get_config_file_path()?;

        let content = match fs::read_to_string(&config_path) {
            Ok(content) => {
                if content.trim().is_empty() {
                    fs::write(&config_path, DEFAULT_CONFIG)?;
                    DEFAULT_CONFIG.to_string()
                } else {
                    content
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                fs::write(&config_path, DEFAULT_CONFIG)?;
                DEFAULT_CONFIG.to_string()
            }
            Err(e) => return Err(e.into()),
        };

        let config: Value = serde_json::from_str(&content)?;
        Ok(config)
    }

    pub fn save_config(config: &Value) -> Result<(), io::Error> {
        let config_path = get_config_file_path()?;
        let config_str = serde_json::to_string_pretty(config)?;
        fs::write(&config_path, config_str)?;
        Ok(())
    }
}

/// Flow management
pub mod flow {
    use nix::unistd::execve;
    use std::env;
    use std::ffi::CString;

    #[allow(unreachable_code)]
    pub fn execute(bin_path: &str, bin_args: &[String]) {
        let path = CString::new(bin_path).expect("Failed to create CString");

        let mut c_args: Vec<CString> = Vec::with_capacity(bin_args.len() + 1);
        c_args.push(path.clone());
        for arg in bin_args {
            c_args.push(CString::new(arg.as_str()).expect("Failed to create CString"));
        }

        let c_args_refs: Vec<&CString> = c_args.iter().collect();

        let c_env: Vec<CString> = env::vars()
            .map(|(k, v)| CString::new(format!("{}={}", k, v)).expect("Failed to create CString"))
            .collect();
        let c_env_refs: Vec<&CString> = c_env.iter().collect();

        execve::<&CString, &CString>(&path, &c_args_refs, &c_env_refs).expect("Failed to exec");
    }
}
