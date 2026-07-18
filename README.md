# binx

![Rust](https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white)
![Linux](https://img.shields.io/badge/Linux-FCC624?style=for-the-badge&logo=linux&logoColor=black)

Alias manager for executables. Run binaries by short names instead of full paths, with support for script installation and desktop integration.

## Install

```sh
git clone <repository-url>
cd binx
cargo build --release
cp target/release/binx ~/.binx/binx
```

Binx auto-installs itself to `~/.binx/binx` on first run and adds to PATH.

## Uso

### Registrar e executar

```bash
# Register an executable (prompts for alias name)
binx /opt/myapp/bin/myapp

# Run by alias
binx myapp arg1 arg2
```

### Instalar wrapper para shell

```bash
binx /opt/myapp/bin/myapp --install
# Now run directly:
myapp arg1 arg2
```

### Criar entrada desktop

```bash
binx /opt/myapp/bin/myapp --desktop
```

### Remover alias

```bash
binx myapp --remove
```

## Configuração

Aliases são armazenados em `~/.config/binx.json`:

```json
{
  "aliases": {
    "myapp": {
      "path": "/opt/myapp/bin/myapp",
      "script": "/home/user/.binx/myapp",
      "desktop": "/home/user/.local/share/applications/binx_myapp.desktop"
    }
  }
}
```

Os campos `script` e `desktop` são adicionados ao usar `--install` e `--desktop`.

## Desenvolvimento

```sh
# Build debug
cargo build

# Build release
cargo build --release

# Run
cargo run -- <target> [args...]
```

## Créditos ou referências

- [clap](https://github.com/clap-rs/clap) - Command line argument parser
- [nix](https://github.com/nix-rust/nix) - Rust friendly bindings to *nix APIs
- [serde_json](https://github.com/serde-rs/json) - JSON serialization

## Licença

Este projeto está licenciado sob a [GNU General Public License v3.0](LICENSE.txt).
