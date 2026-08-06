use clap::Parser;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

mod internal;
mod utils;

pub use crate::internal::config;
pub use crate::internal::flow;

const VERSION: &str = if cfg!(debug_assertions) {
    "0.1.0-debug"
} else {
    "0.1.0"
};

#[derive(Parser)]
#[command(name = "binx")]
#[command(about = "Execute binaries with alias management", long_about = None)]
#[command(version = VERSION)]
#[command(disable_version_flag = true)]
struct Cli {
    /// Target file or alias to execute
    target: String,

    /// Remove an alias, its script and .desktop file
    #[arg(short = 'r', long = "remove")]
    remove: bool,

    /// Install a script for the target in ~/.binx/
    #[arg(short = 'i', long = "install")]
    install: bool,

    /// Install a .desktop file for the target
    #[arg(short = 'd', long = "desktop")]
    desktop: bool,

    /// Print version information
    #[arg(short = 'v', long = "version", action = clap::ArgAction::Version)]
    version: (),

    /// Remaining arguments to pass to the target
    #[arg(trailing_var_arg = true)]
    args: Vec<String>,
}

#[allow(unreachable_code)]
fn main() {
    // Run auto-installation check first
    if let Err(e) = config::run_auto_installation(VERSION) {
        eprintln!("Auto-installation failed: {}", e);
        eprintln!("Continuing anyway...");
    }

    let cli = Cli::parse();

    let target = cli.target;
    let args = cli.args;
    
    println!("binx v{}", VERSION);
    
    let mut config = match config::get_config() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Failed to read/create config file: {}", e);
            return;
        }
    };
    
    let mut target_name = String::new();
    let mut target_path_str = String::new();
    let mut target_path_buf = PathBuf::new();
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
                target_name = target.clone();

                if let Some(alias_path) = alias.get("path").and_then(|p| p.as_str()) {
                    match utils::system::resolve_path(alias_path) {
                        Ok(resolved) => {
                            exists = true;
                            target_path_str = resolved.to_string_lossy().to_string();
                            target_path_buf = resolved;
                        }
                        Err(e) => {
                            eprintln!("Error: alias '{}' has invalid path: {}", target, e);
                            return;
                        }
                    }
                }
            }
        }
    } else {
        // Target is a path (absolute, relative, or ~/...), resolve to absolute
        target_path_buf = match utils::system::resolve_path(&target) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Error: {}", e);
                return;
            }
        };

        if !target_path_buf.is_file() {
            eprintln!("Error: '{}' is not a file", target);
            return;
        }

        target_path_str = target_path_buf
            .to_str()
            .expect("Failed to convert path to string")
            .to_string();

        // Extract basename from the path string as default name suggestion
        target_name = target_path_str
            .rsplit('/')
            .next()
            .unwrap_or(&target_path_str)
            .to_string();

        // Search ALL aliases by path value to detect existing registrations
        if let Some(aliases) = aliases {
            for (alias_key, alias_val) in aliases.iter() {
                if let Some(alias_path) = alias_val.get("path").and_then(|p| p.as_str()) {
                    if alias_path == target_path_str.as_str() {
                        // Already registered under a (possibly different) alias
                        target_name = alias_key.clone();
                        exists = true;
                        break;
                    }
                }
            }
        }
    }

    // Handle --remove flag
    if cli.remove {
        if exists {
            if let Err(e) = config::remove_alias(&target_name) {
                eprintln!("Failed to remove alias: {}", e);
            }
        } else {
            println!("'{}' is not a registered alias", target);
        }
        return;
    }

    if !exists {
        use std::io::{self, Write};

        print!("Enter alias name [{}]: ", target_name);
        io::stdout().flush().expect("Failed to flush stdout");

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .expect("Failed to read input");
        let input = input.trim().replace(' ', "_");

        if !input.is_empty() {
            target_name = input;
        }

        if let Some(aliases) = config.get_mut("aliases").and_then(|a| a.as_object_mut()) {
            let alias_obj = serde_json::json!({
                "path": target_path_str
            });

            aliases.insert(target_name.to_string(), alias_obj);

            if let Err(e) = config::save_config(&config) {
                eprintln!("Failed to save config file: {}", e);
            }
        }
    }

    // Handle --install flag
    if cli.install {
        if let Err(e) = config::install_target_script(&target_name) {
            eprintln!("Failed to install script for '{}': {}", target_name, e);
            return;
        }
        println!("Successfully installed script for '{}'", target_name);
        println!("You can next time run: {} [args]", target_name);
    }

    // Handle --desktop flag
    if cli.desktop {
        if let Err(e) = config::install_desktop_file(&target_name, &target_path_buf) {
            eprintln!("Failed to install .desktop file for '{}': {}", target_name, e);
            return;
        }
        println!("Successfully installed .desktop file for '{}'", target_name);
    }

    let is_executable = std::fs::metadata(&target_path_str)
        .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false);

    if is_executable {
        flow::execute(target_path_str.as_str(), &args);
    } else {
        eprintln!("Error: '{}' is not executable", target_path_str.as_str());
    }
}
