# Release Notes

## Windows Setup Made Easy (#51)

**ScriptCraft now distributes pre-built Windows executables**, making it simple for Windows users to get started without installing Rust or building from source.

### What's New

- **Windows Downloads:** Download pre-built Windows binaries from [GitHub Releases](https://github.com/NoJuanNobody/scriptcraft/releases)
- **Setup Guide:** New [Windows Setup Guide](docs/windows-setup.md) with step-by-step instructions, troubleshooting, and first-launch help
- **Downloads Section:** Updated README with a prominent Downloads section linking to releases and setup documentation

### For Windows Users

**Getting started is now three steps:**

1. Download the latest `scriptcraft-windows-x64.zip` from [GitHub Releases](https://github.com/NoJuanNobody/scriptcraft/releases)
2. Extract the ZIP file to any folder
3. Double-click `scriptcraft.exe` to launch

No Rust installation, no build tools, no command line required.

**First Launch:** ScriptCraft creates settings and backups in `%APPDATA%\scriptcraft` automatically. The sample script "Night Signal" opens by default. Create a new blank script with **File › New** or open an existing `.fountain` or `.fdx` file.

**Having trouble?** See [Windows Setup Guide: Troubleshooting](docs/windows-setup.md#troubleshooting) for solutions to common issues (Defender warnings, compatibility checks, settings location).

### For Developers and CI

- **Release Workflow:** New `.github/workflows/release.yml` automates Windows binary builds on every tagged release
- **Build Command:** Same as CI—`cargo build --release -p scriptcraft`—ensures consistency and reliability
- **Package Contents:** ZIP includes executable, both license files, and README for easy distribution

When you create a release tag, the workflow automatically:
1. Builds the Windows binary
2. Packages it with supporting files
3. Uploads to GitHub Releases

### Documentation

- [Windows Setup Guide](docs/windows-setup.md) – Complete setup and troubleshooting
- [README Downloads Section](README.md#downloads) – Quick links for all platforms
- [ROADMAP](ROADMAP.md) – Next milestones and planned features

---

**Questions or issues?** Open a [GitHub issue](https://github.com/NoJuanNobody/scriptcraft/issues) or check the [Windows Setup Guide](docs/windows-setup.md#troubleshooting).
