# DeSmuME Tauri Frontend Prototype

This directory contains a Tauri v2 prototype frontend for DeSmuME.

The web UI is React/Vite. The Rust backend exposes Tauri commands and loads a native bridge dynamically. The native bridge symbols are built into the existing `frontend/interface` shared library by adding `native/desmume_tauri_bridge.cpp` to that target.

## Requirements

- Node.js and npm.
- Rust and Cargo.
- The normal DeSmuME interface build requirements for your platform.

## Build the Native Bridge

Windows:

```powershell
cd ..\interface\windows
& "C:\Program Files\Microsoft Visual Studio\2022\Community\MSBuild\Current\Bin\MSBuild.exe" DeSmuME_Interface.sln /p:Configuration=Release /p:Platform=x64
```

Linux/macOS:

```sh
cd ../interface
meson build
ninja -C build
```

If the generated library is not in a default search location, set `DESMUME_TAURI_BRIDGE_PATH` to the full DLL, SO, or DYLIB path before starting Tauri.

## Run

```sh
npm install
npm run tauri dev
```

## Commands

- `bridge_status`
- `emulator_info`
- `open_rom(path)`
- `set_paused(paused)`
- `reset`
- `set_keys(mask)`
- `frame`
