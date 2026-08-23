use clap::Parser;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

mod internal;
mod utils;

pub use crate::internal::app;
pub use crate::internal::config;
pub use crate::internal::flow;

#[derive(Parser)]
#[command(name = app::NAME)]
#[command(about = app::DESCRIPTION, long_about = None)]
#[command(version = app::VERSION)]
#[command(disable_version_flag = true)]
struct Cli {
    /// Target file or alias to execute
    #[arg(required_unless_present = "version", required_unless_present = "list")]
    target: Option<String>,

    /// Remove an alias, its script and .desktop file
    #[arg(short = 'r', long = "remove")]
    remove: bool,

    /// Install a script for the target in ~/.binx/
    #[arg(short = 'i', long = "install")]
    install: bool,

    /// Install a .desktop file for the target
    #[arg(short = 'd', long = "desktop")]
    desktop: bool,

    /// List available aliases
    #[arg(short = 'l', long = "list")]
    list: bool,

    /// Print version information
    #[arg(short = 'v', long = "version")]
    version: bool,

    /// Remaining arguments to pass to the target
    #[arg(trailing_var_arg = true)]
    args: Vec<String>,
}

#[allow(unreachable_code)]
fn main() {
    let cli = Cli::parse();
    let version = app::mode_version();
    
    // Always print version first
    println!("{} v{}", app::NAME, version);

    // Handle --version flag
    if cli.version {
        return;
    }

    // Run auto-installation check first
    if let Err(e) = config::run_auto_installation(&version) {
        eprintln!("Auto-installation failed: {}", e);
        eprintln!("Continuing anyway...");
    }

    // Load configuration
    let mut config = match config::load() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Failed to read/create config file: {}", e);
            return;
        }
    };

    // Available aliases from config
    let available_aliases: String = config.aliases.keys().map(|k| k.clone()).collect::<Vec<String>>().join(", ");

    if cli.list {
        println!("Available aliases: {}", available_aliases);
        return;
    }

    // A path or link to an executable to be aliased
    // or alias of an already aliased executable
    let target = cli.target.expect(
        format!("Target is a required argument. Available aliases: {}", available_aliases).as_str()
    );

    // Arguments to pass to the target command
    let target_args = cli.args;

    let mut target_name: String;
    let mut target_path_str = "".to_string();
    let target_path_buf: PathBuf;
    let mut exists = false;
    
    // First, determine if target is an alias or a path
    if config.aliases.contains_key(&target) {
        // Target is an alias
        target_name = target.clone();
        let alias = config.aliases.get(&target).unwrap();

        match utils::system::resolve_path(&alias.path) {
            Ok(path) => {
                exists = true;
                target_path_str = path.to_string_lossy().to_string();
                target_path_buf = path;
            }
            Err(e) => {
                // Something is wrong with the alias configuration
                eprintln!("Error: alias '{}' has invalid path: {}", target, e);
                return;
            }
        }
    } else if target.starts_with("http://") || target.starts_with("https://") {
        // TODO: Handle HTTP URLs - maybe download and cache?
        eprintln!("Error: HTTP URLs are not yet supported. Target: {}", target);
        return;
    } else {
        // Target is a path (absolute, relative, or ~/...), resolve to absolute
        target_path_buf = match utils::system::resolve_path(&target) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Error: target '{}' is invalid: {}", target, e);
                return;
            }
        };

        // Check if the resolved path is a file (not a directory)
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
        // and avoid duplications
        for (alias_key, alias_val) in config.aliases.iter() {
            if alias_val.path == target_path_str {
                // Already registered under a (possibly different) alias
                target_name = alias_key.clone();
                exists = true;
                break;
            }
        }
    }

    // Handle --remove flag
    if cli.remove {
        if exists {
            if let Err(e) = config::remove_alias(&target_name, &mut config) {
                eprintln!("Failed to remove alias: {}", e);
            }
        } else {
            println!("'{}' is not a registered alias", target);
        }
        return;
    }

    // Must be executable to be registered
    let is_executable = std::fs::metadata(&target_path_str)
        .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false);

    if !is_executable {
        eprintln!("Error: '{}' is not executable", target_path_str.as_str());
        return;
    }

    if !exists {
        // Register new alias
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

        let alias_obj = config::ConfigAlias { 
            path: target_path_str.clone(),
            script: String::new(),
            desktop: String::new(),
            icon: String::new()
        };

        config.aliases.insert(target_name.clone(), alias_obj);

        if let Err(e) = config::save_config(&config) {
            eprintln!("Failed to save config file: {}", e);
        }
    }

    // Handle --install flag
    if cli.install {
        if let Err(e) = config::install_target_script(&target_name, &mut config) {
            eprintln!("Failed to install script for '{}': {}", target_name, e);
            return;
        }
        println!("Successfully installed script for '{}'", target_name);
        println!("You can next time run: {} [args]", target_name);
    }

    // Handle --desktop flag
    if cli.desktop {
        if let Err(e) = config::install_desktop_file(&target_name, &target_path_buf, &mut config) {
            eprintln!("Failed to install .desktop file for '{}': {}", target_name, e);
            return;
        }
        println!("Successfully installed .desktop file for '{}'", target_name);
    }

    flow::execute(target_path_str.as_str(), &target_args);
}
