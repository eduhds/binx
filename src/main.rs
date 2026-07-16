use nix::unistd::execve;
use std::ffi::CString;
use std::os::unix::fs::PermissionsExt;
use serde_json::Value;
use clap::Parser;
use std::path::PathBuf;

const VERSION: &str = "0.1.0";
const CONFIG_FILE: &str = ".config/binx.json";
const DEFAULT_CONFIG: &str = r#"{
  "aliases": {}
}"#;

/// Installation directory path
const INSTALL_DIR: &str = ".binx";
/// Installation executable name
const INSTALL_EXECUTABLE: &str = "binx";

#[derive(Parser)]
#[command(name = "binx")]
#[command(about = "Execute binaries with alias management", long_about = None)]
#[command(version = VERSION)]
#[command(disable_version_flag = true)]
struct Cli {
    /// Target file or alias to execute
    target: String,

    /// Print version information
    #[arg(short = 'v', long = "version", action = clap::ArgAction::Version)]
    version: (),
}

/// Gets the home directory from environment variables
fn get_home_dir() -> Result<String, std::io::Error> {
    std::env::var("HOME").map_err(|e| std::io::Error::new(std::io::ErrorKind::NotFound, e))
}

/// Gets the configuration file path
fn get_config_file_path() -> Result<String, std::io::Error> {
    let home_dir = get_home_dir()?;
    Ok(format!("{}/{}", home_dir, CONFIG_FILE))
}

/// Gets the installation directory path (~/.binx)
fn get_install_dir() -> Result<PathBuf, std::io::Error> {
    let home_dir = get_home_dir()?;
    Ok(PathBuf::from(home_dir).join(INSTALL_DIR))
}

/// Gets the installation executable path (~/.binx/binx)
fn get_install_executable_path() -> Result<PathBuf, std::io::Error> {
    let install_dir = get_install_dir()?;
    Ok(install_dir.join(INSTALL_EXECUTABLE))
}

/// Gets the current executable path
fn get_current_executable_path() -> Result<PathBuf, std::io::Error> {
    std::env::current_exe().map_err(|e| std::io::Error::new(std::io::ErrorKind::NotFound, e))
}

/// Checks if the current executable is running from the installation path
fn is_running_from_installation() -> bool {
    match (get_current_executable_path(), get_install_executable_path()) {
        (Ok(current), Ok(install)) => current == install,
        _ => false,
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
fn perform_installation() -> Result<(), std::io::Error> {
    let install_dir = get_install_dir()?;
    let install_path = get_install_executable_path()?;
    let current_path = get_current_executable_path()?;

    // Create installation directory if it doesn't exist
    if !install_dir.exists() {
        std::fs::create_dir_all(&install_dir)?;
        println!("Created installation directory: {:?}", install_dir);
    }

    // Copy the current executable to the installation path
    std::fs::copy(&current_path, &install_path)?;
    
    // Set executable permissions
    let mut perms = std::fs::metadata(&install_path)?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&install_path, perms)?;
    
    println!("Installed executable to: {:?}", install_path);
    
    Ok(())
}

/// Detects the current shell and returns the appropriate rc file path
fn detect_shell_rc() -> Option<PathBuf> {
    let home_dir = get_home_dir().ok()?;
    let shell = std::env::var("SHELL").ok()?;
    
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

/// Adds the installation directory to PATH in the shell's rc file
fn add_to_path() -> Result<(), std::io::Error> {
    let install_dir = get_install_dir()?;
    let install_dir_str = install_dir.to_string_lossy().to_string();
    let rc_path = detect_shell_rc().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "Could not detect shell rc file")
    })?;
    
    // Check if PATH already contains the installation directory
    if let Ok(path) = std::env::var("PATH") {
        if path.split(':').any(|p| p == install_dir_str) {
            println!("Installation directory already in PATH");
            return Ok(());
        }
    }
    
    // Read the rc file content
    let content = std::fs::read_to_string(&rc_path).unwrap_or_default();
    
    // Check if the PATH modification already exists in the rc file
    let path_line = format!("export PATH=\"$PATH:{}\"", install_dir_str);
    if content.contains(&install_dir_str) {
        println!("PATH modification already exists in {:?}", rc_path);
        return Ok(());
    }
    
    // Append the PATH modification to the rc file
    let new_content = if content.is_empty() {
        format!("{}\n", path_line)
    } else {
        format!("{}\n{}\n", content, path_line)
    };
    
    std::fs::write(&rc_path, new_content)?;
    println!("Added {:?} to PATH in {:?}", install_dir, rc_path);
    println!("Please run: source {:?}", rc_path);
    
    Ok(())
}

/// Runs the auto-installation check and performs installation if needed
fn run_auto_installation() -> Result<(), std::io::Error> {
    // If running from installation, proceed normally
    if is_running_from_installation() {
        return Ok(());
    }
    
    // If installation exists, respect it and proceed
    if installation_exists() {
        println!("Installation exists at {:?}", get_install_executable_path()?);
        println!("Please use the installed version instead");
        return Ok(());
    }
    
    // Installation doesn't exist, perform installation
    println!("Performing auto-installation...");
    perform_installation()?;
    add_to_path()?;
    
    Ok(())
}

fn get_config() -> Result<Value, Box<dyn std::error::Error>> {
    let config_path = get_config_file_path()?;
    
    let content = match std::fs::read_to_string(&config_path) {
        Ok(content) => {
            if content.trim().is_empty() {
                std::fs::write(&config_path, DEFAULT_CONFIG)?;
                DEFAULT_CONFIG.to_string()
            } else {
                content
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            std::fs::write(&config_path, DEFAULT_CONFIG)?;
            DEFAULT_CONFIG.to_string()
        }
        Err(e) => return Err(e.into()),
    };

    let config: Value = serde_json::from_str(&content)?;
    Ok(config)
}

fn save_config(config: &Value) -> Result<(), std::io::Error> {
    let config_path = get_config_file_path()?;
    let config_str = serde_json::to_string_pretty(config)?;
    std::fs::write(&config_path, config_str)?;
    Ok(())
}

#[allow(unreachable_code)]
fn binx_exec(bin_path: &str, bin_args: &[String]) {
    let path = CString::new(bin_path).expect("Failed to create CString");
    let c_args: Vec<CString> = bin_args.iter()
        .map(|s| CString::new(s.as_str()).expect("Failed to create CString"))
        .collect();
    let c_args_refs: Vec<&CString> = c_args.iter().collect();

    let c_env: Vec<CString> = std::env::vars()
        .map(|(k, v)| CString::new(format!("{}={}", k, v)).expect("Failed to create CString"))
        .collect();
    let c_env_refs: Vec<&CString> = c_env.iter().collect();

    println!("---");

    execve::<&CString, &CString>(&path, &c_args_refs, &c_env_refs).expect("Failed to exec");
}

#[allow(unreachable_code)]
fn main() {
    // Run auto-installation check first
    if let Err(e) = run_auto_installation() {
        eprintln!("Auto-installation failed: {}", e);
        eprintln!("Continuing anyway...");
    }
    
    let cli = Cli::parse();

    let target = cli.target;
    let args: Vec<String> = std::env::args().collect();
    let mut name = String::new();

    println!("binx v{}", VERSION);

    let mut config = match get_config() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Failed to read/create config file: {}", e);
            return;
        }
    };

    let mut absolute_path_str = String::new();
    let mut exists = false;

    let aliases = config.get("aliases").and_then(|a| a.as_object());

    // First, determine if target is an alias or a path
    let is_alias = if let Some(aliases) = aliases {
        aliases.get(&target).is_some()
    } else {
        false
    };

    if is_alias {
        // Target is an alias
        if let Some(aliases) = aliases {
            if let Some(alias) = aliases.get(&target) {
                name = target.clone();
                
                if let Some(alias_path) = alias.get("path").and_then(|p| p.as_str()) {
                    exists = true;
                    absolute_path_str = alias_path.to_string();
                }
            }
        }
    } else {
        // Target may be a path, resolve it
        let absolute_path_buf = match std::fs::canonicalize(&target) {
            Ok(p) => p,
            Err(_) => {
                eprintln!("Error: '{}' is not a valid path or alias", target);
                return;
            }
        };
        
        absolute_path_str = absolute_path_buf.to_str().expect("Failed to convert path to string").to_string();
        
        // Extract basename from the path string
        name = absolute_path_str
            .rsplit('/')
            .next()
            .unwrap_or(&absolute_path_str)
            .to_string();

        // Check if the basename exists as an alias
        if let Some(aliases) = aliases {
            if let Some(alias) = aliases.get(&name) {
                if let Some(alias_path) = alias.get("path").and_then(|p| p.as_str()) {
                    if alias_path == absolute_path_str.as_str() {
                        exists = true;
                    }
                }
            }
        }
    }

    if !exists {
        use std::io::{self, Write};

        print!("Enter alias name [{}]: ", name);
        io::stdout().flush().expect("Failed to flush stdout");

        let mut input = String::new();
        io::stdin().read_line(&mut input).expect("Failed to read input");
        let input = input.trim();

        let alias_name = if input.is_empty() {
            name.clone()
        } else {
            input.to_string()
        };

        if let Some(aliases) = config.get_mut("aliases").and_then(|a| a.as_object_mut()) {
            let alias_obj = serde_json::json!({
                "path": absolute_path_str
            });

            aliases.insert(alias_name.to_string(), alias_obj);
            
            if let Err(e) = save_config(&config) {
                eprintln!("Failed to save config file: {}", e);
            }
        }
    }

    let exec_path = if absolute_path_str.is_empty() {
        target.as_str()
    } else {
        absolute_path_str.as_str()
    };

    let is_executable = std::fs::metadata(exec_path)
        .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false);

    if is_executable {
        binx_exec(exec_path, &args[1..]);
    } else {
        eprintln!("Error: '{}' is not executable", exec_path);
    }
}

