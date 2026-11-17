<h1 align="center">PSeInt Interpreter</h1>

<p align="center">An initial Rust library and CLI scaffold for the PSeInt pseudolanguage.</p>

<p align="center">
  <a href="https://diegopn.github.io/potoo-site/">Website</a> ·
  <a href="https://diegopn.github.io/potoo-site/manual/pseint/">User manual</a>
</p>

<p align="center">
  <a href="https://www.rust-lang.org/"><img alt="Rust" src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white"></a>
  <a href="https://doc.rust-lang.org/cargo/"><img alt="Cargo" src="https://img.shields.io/badge/Cargo-DEA584?style=for-the-badge&logo=rust&logoColor=white"></a>
  <img alt="Rust library" src="https://img.shields.io/badge/Rust_library-4A86CF?style=for-the-badge">
  <img alt="CLI" src="https://img.shields.io/badge/CLI-6A4C93?style=for-the-badge">
</p>

PSeInt is a pseudolanguage for expressing algorithms in Spanish. Its language profiles can be configured, so this repository does not yet select a compatibility profile.

This repository is only a starting scaffold. It does not yet parse or execute PSeInt source, and no interpreter features are currently implemented. The Rust implementation will be developed here independently from the original application.

## Original language references

- [PSeInt official website](https://pseint.sourceforge.net/)
- [PSeInt pseudocode reference](https://pseint.sourceforge.net/index.php?page=pseudocodigo.php)
- [PSeInt features and language profiles](https://pseint.sourceforge.net/index.php?page=features.php)

These references define the language compatibility work. This repository will list its supported syntax, runtime behavior, and limitations after implementation begins.

## Development

Run these commands from the repository root:

```sh
cargo fmt --all -- --check
cargo check
cargo test
```

## Project policies

- [Code of conduct](CODE_OF_CONDUCT.md)
- [Contributing](CONTRIBUTING.md)
- [Security policy](SECURITY.md)
- [GPL-3.0-or-later license](LICENSE)
