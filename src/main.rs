use nix::unistd::execve;
use std::ffi::CString;
use std::os::unix::fs::PermissionsExt;
use serde_json::Value;

const VERSION: &str = "0.1.0";
const CONFIG_FILE: &str = ".config/binx.json";
const DEFAULT_CONFIG: &str = r#"{
  "aliases": {}
}"#;

fn get_config_file_path() -> Result<String, std::io::Error> {
    let home_dir = std::env::var("HOME").map_err(|e| std::io::Error::new(std::io::ErrorKind::NotFound, e))?;
    Ok(format!("{}/{}", home_dir, CONFIG_FILE))
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
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        println!("Usage: binx <file> [args...]");
        return;
    }

    let target = &args[1];
    let mut name = "";

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
        aliases.get(target).is_some()
    } else {
        false
    };

    if is_alias {
        // Target is an alias
        if let Some(aliases) = aliases {
            if let Some(alias) = aliases.get(target) {
                name = target;
                
                if let Some(alias_path) = alias.get("path").and_then(|p| p.as_str()) {
                    exists = true;
                    absolute_path_str = alias_path.to_string();
                }
            }
        }
    } else {
        // Target may be a path, resolve it
        let absolute_path_buf = match std::fs::canonicalize(target) {
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
            .unwrap_or(&absolute_path_str);

        // Check if the basename exists as an alias
        if let Some(aliases) = aliases {
            if let Some(alias) = aliases.get(name) {
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
            name
        } else {
            input
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

