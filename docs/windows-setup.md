# Windows Setup Guide

This guide walks you through installing and running ScriptCraft on Windows.

## Prerequisites

**Windows 10 or later** is required to run ScriptCraft.

## Option 1: Download the Portable Executable (Easiest)

The simplest way to get started is to download the pre-built Windows binary.

1. Go to the [ScriptCraft Releases](https://github.com/NoJuanNobody/scriptcraft/releases) page
2. Find the latest release and download `scriptcraft-windows-x64.zip`
3. Extract the ZIP file to a folder of your choice (e.g., `C:\Users\YourName\scriptcraft`)
4. Double-click `scriptcraft.exe` to launch the app

That's it! No installation, no dependencies to manage.

### First Launch

When you run ScriptCraft for the first time on Windows:

- The app creates a settings folder at `%APPDATA%\scriptcraft` (usually `C:\Users\YourName\AppData\Roaming\scriptcraft`)
- This folder stores your preferences, autosave recovery, backups, and custom dictionary
- A sample script ("Night Signal") opens by default
- Create a new blank script with **File › New** or press **Ctrl+N**

### Common Tasks

- **Open an existing script:** File › Open (**Ctrl+O**), or drag a `.fountain` or `.fdx` file onto the window
- **Save your work:** File › Save (**Ctrl+S**). Autosave is enabled by default
- **Export to PDF:** File › Export as PDF
- **Get help:** Press **F1** or visit the menu bar for commands and shortcuts

## Option 2: Build from Source

If you prefer to build ScriptCraft yourself, or if you want the latest development version:

### Install Rust

1. Download and run the Rust installer from [rustup.rs](https://rustup.rs/)
2. Follow the on-screen prompts to install Rust (choose the default options)
3. Close and reopen your terminal for the changes to take effect

### Build and Run

```sh
git clone https://github.com/NoJuanNobody/scriptcraft
cd scriptcraft
cargo run --release -p scriptcraft
```

The first build takes a few minutes. After that, the executable is at `target\release\scriptcraft.exe`.

## Troubleshooting

### "The app won't open" or "Windows Defender flags the app"

This sometimes happens with unsigned binaries. Windows Defender may quarantine the `.exe` on first download.

**Solution:**
1. Right-click `scriptcraft.exe`
2. Click **Properties**
3. At the bottom, check **Unblock** and click **Apply**
4. Click **OK** and try again

### "Windows says the app is not compatible"

ScriptCraft requires **Windows 10 or later**. If you're on Windows 7 or 8, you'll need to upgrade to run the app. Alternatively, you can build from source with an older Rust toolchain (not supported or tested).

### "I can't find my settings folder"

Settings and backups are stored at:

```
%APPDATA%\scriptcraft
```

In Windows Explorer, you can paste this path directly into the address bar to jump to it. On most systems, this is:

```
C:\Users\YourName\AppData\Roaming\scriptcraft
```

You can also open this folder from ScriptCraft via **Help › About**, which shows all the folders in use.

### "How do I uninstall?"

Since ScriptCraft is portable, just delete the folder you extracted. Your settings and files are kept in `%APPDATA%\scriptcraft`; delete that folder too if you want to remove them.

## Next Steps

- Read the [README](../README.md) for an overview of features
- Check the [Roadmap](../ROADMAP.md) to see what's coming next
- Visit [docs](../docs/) for technical documentation, including the MCP server and CLI tool

## Questions?

Open an issue on [GitHub](https://github.com/NoJuanNobody/scriptcraft/issues) or check the existing discussions.
