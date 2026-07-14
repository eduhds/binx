# binx

A simple alias manager for executables that allows you to run commands by short names instead of full paths.

## Features

- **Alias Management**: Create and manage aliases for executables
- **Automatic Path Resolution**: Resolves absolute paths from relative paths or aliases
- **Interactive Alias Creation**: Prompts for alias name when creating new aliases (defaults to basename)
- **Configuration Storage**: Stores aliases in a JSON configuration file (`~/.config/binx.json`)
- **Executable Verification**: Checks if files are executable before running
- **Environment Preservation**: Passes all environment variables to executed processes (including DISPLAY for GUI apps)

## Installation

```bash
# Clone the repository
git clone <repository-url>
cd binx

# Build the project
cargo build --release

# Install (optional)
sudo cp target/release/binx /usr/local/bin/
```

## Usage

### Basic Usage

```bash
# Run an executable by full path
binx /path/to/executable

# Run an executable by alias (after creating one)
binx myalias
```

### Creating Aliases

When you run an executable that doesn't have an alias, binx will prompt you to create one:

```bash
$ binx /home/user/projects/myapp/target/release/myapp
binx v0.1.0
Enter alias name [myapp]: myapp
---
# myapp executes
```

Press Enter to use the suggested alias name (the basename), or type a custom name.

### Configuration

Aliases are stored in `~/.config/binx.json`:

```json
{
  "aliases": {
    "myapp": {
      "path": "/home/user/projects/myapp/target/release/myapp"
    }
  }
}
```

You can edit this file manually to add, remove, or modify aliases.

## Development

### Building

```bash
cargo build
```

### Running

```bash
cargo run -- <executable> [args...]
```

### Testing

```bash
cargo test
```

## Requirements

- Rust 2024 edition
- Linux/Unix system (uses Unix-specific features)
- `nix` crate with `process` feature
- `serde_json` crate

## License

TODO: Add license information

## Contributing

TODO: Add contribution guidelines
