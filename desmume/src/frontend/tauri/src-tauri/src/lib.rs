use libloading::Library;
use serde::Serialize;
use std::{
    collections::HashSet,
    env,
    ffi::CString,
    os::raw::{c_char, c_int},
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
};
use tauri::{ipc::Response, State};

const FRAME_WIDTH: usize = 256;
const FRAME_HEIGHT: usize = 384;
const FRAME_BYTES: usize = FRAME_WIDTH * FRAME_HEIGHT * 4;

type InitFn = unsafe extern "C" fn() -> c_int;
type ShutdownFn = unsafe extern "C" fn();
type OpenRomFn = unsafe extern "C" fn(*const c_char) -> c_int;
type SetPausedFn = unsafe extern "C" fn(c_int);
type ResetFn = unsafe extern "C" fn() -> c_int;
type SetKeyMaskFn = unsafe extern "C" fn(u16);
type RunFrameFn = unsafe extern "C" fn() -> c_int;
type GetFrameFn = unsafe extern "C" fn(*mut u8, usize) -> c_int;

#[derive(Serialize)]
struct BridgeStatus {
    loaded: bool,
    path: Option<String>,
    error: Option<String>,
    frame_width: usize,
    frame_height: usize,
}

#[derive(Serialize)]
struct EmulatorInfo {
    frame_width: usize,
    frame_height: usize,
    frame_bytes: usize,
}

struct NativeBridge {
    _library: Library,
    path: PathBuf,
    init: InitFn,
    shutdown: ShutdownFn,
    open_rom: OpenRomFn,
    set_paused: SetPausedFn,
    reset: ResetFn,
    set_key_mask: SetKeyMaskFn,
    run_frame: RunFrameFn,
    get_frame: GetFrameFn,
}

struct Emulator {
    bridge: Option<NativeBridge>,
    initialized: bool,
    rom_loaded: bool,
    paused: bool,
    key_mask: u16,
    last_error: Option<String>,
}

struct EmulatorState(Mutex<Emulator>);

impl Default for EmulatorState {
    fn default() -> Self {
        Self(Mutex::new(Emulator {
            bridge: None,
            initialized: false,
            rom_loaded: false,
            paused: true,
            key_mask: 0,
            last_error: None,
        }))
    }
}

impl Drop for Emulator {
    fn drop(&mut self) {
        if self.initialized {
            if let Some(bridge) = &self.bridge {
                unsafe {
                    (bridge.shutdown)();
                }
            }
        }
    }
}

impl NativeBridge {
    fn load() -> Result<Self, String> {
        let mut tried = Vec::new();

        for path in bridge_candidates() {
            tried.push(path.display().to_string());
            if !path.exists() {
                continue;
            }

            prepare_dynamic_library_search(&path);
            let library = unsafe { Library::new(&path) }
                .map_err(|error| format!("Failed to load {}: {error}", path.display()))?;

            let init = load_symbol::<InitFn>(&library, b"tauri_desmume_init\0")?;
            let shutdown = load_symbol::<ShutdownFn>(&library, b"tauri_desmume_shutdown\0")?;
            let open_rom = load_symbol::<OpenRomFn>(&library, b"tauri_desmume_open_rom\0")?;
            let set_paused = load_symbol::<SetPausedFn>(&library, b"tauri_desmume_set_paused\0")?;
            let reset = load_symbol::<ResetFn>(&library, b"tauri_desmume_reset\0")?;
            let set_key_mask = load_symbol::<SetKeyMaskFn>(&library, b"tauri_desmume_set_key_mask\0")?;
            let run_frame = load_symbol::<RunFrameFn>(&library, b"tauri_desmume_run_frame\0")?;
            let get_frame = load_symbol::<GetFrameFn>(&library, b"tauri_desmume_get_frame_rgba\0")?;

            return Ok(Self {
                _library: library,
                path,
                init,
                shutdown,
                open_rom,
                set_paused,
                reset,
                set_key_mask,
                run_frame,
                get_frame,
            });
        }

        Err(format!(
            "Native bridge not found. Tried: {}",
            tried.join(", ")
        ))
    }
}

impl Emulator {
    fn ensure_bridge_loaded(&mut self) -> Result<(), String> {
        if self.bridge.is_none() {
            self.bridge = Some(NativeBridge::load()?);
        }

        Ok(())
    }

    fn ensure_initialized(&mut self) -> Result<(), String> {
        self.ensure_bridge_loaded()?;

        if self.initialized {
            return Ok(());
        }

        let bridge = self.bridge.as_ref().expect("bridge loaded");
        let result = unsafe { (bridge.init)() };
        if result != 0 {
            return Err(format!("Native bridge init failed with code {result}"));
        }

        self.initialized = true;
        Ok(())
    }

    fn status(&self) -> BridgeStatus {
        BridgeStatus {
            loaded: self.bridge.is_some(),
            path: self
                .bridge
                .as_ref()
                .map(|bridge| bridge.path.display().to_string()),
            error: self.last_error.clone(),
            frame_width: FRAME_WIDTH,
            frame_height: FRAME_HEIGHT,
        }
    }
}

fn load_symbol<T: Copy>(library: &Library, name: &[u8]) -> Result<T, String> {
    let symbol = unsafe { library.get::<T>(name) }
        .map_err(|error| format!("Missing bridge symbol {}: {error}", symbol_name(name)))?;
    Ok(*symbol)
}

fn symbol_name(name: &[u8]) -> String {
    String::from_utf8_lossy(name)
        .trim_end_matches('\0')
        .to_string()
}

fn bridge_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    push_env_path(&mut candidates, "DESMUME_TAURI_BRIDGE_PATH");
    push_env_path(&mut candidates, "DESMUME_INTERFACE_PATH");

    let names = bridge_library_names();
    let mut dirs = Vec::new();

    if let Ok(exe) = env::current_exe() {
        if let Some(parent) = exe.parent() {
            dirs.push(parent.to_path_buf());
            dirs.push(parent.join("..").join("..").join("..").join("native").join("bin"));
            dirs.push(
                parent
                    .join("..")
                    .join("..")
                    .join("..")
                    .join("..")
                    .join("interface")
                    .join("windows")
                    .join("__bins"),
            );
            dirs.push(
                parent
                    .join("..")
                    .join("..")
                    .join("..")
                    .join("..")
                    .join("interface")
                    .join("build"),
            );
        }
    }

    if let Ok(cwd) = env::current_dir() {
        dirs.push(cwd.clone());
        dirs.push(cwd.join("native").join("bin"));
        dirs.push(cwd.join("..").join("interface").join("build"));
        dirs.push(cwd.join("..").join("interface").join("windows").join("__bins"));
        dirs.push(cwd.join("src-tauri"));
        dirs.push(cwd.join("src-tauri").join("native").join("bin"));
    }

    for dir in dedupe_paths(dirs) {
        for name in &names {
            candidates.push(dir.join(name));
        }
    }

    dedupe_paths(candidates)
}

fn push_env_path(candidates: &mut Vec<PathBuf>, key: &str) {
    if let Ok(value) = env::var(key) {
        if !value.trim().is_empty() {
            candidates.push(PathBuf::from(value));
        }
    }
}

fn bridge_library_names() -> Vec<&'static str> {
    if cfg!(target_os = "windows") {
        vec![
            "desmume_tauri_bridge.dll",
            "DeSmuME Interface-VS2026-x64-Release.dll",
            "DeSmuME Interface-VS2026-x64-Release Fastbuild.dll",
            "DeSmuME Interface-VS2026-x64-Debug.dll",
            "DeSmuME Interface-VS2022-x64-Release.dll",
            "DeSmuME Interface-VS2022-x64-Release Fastbuild.dll",
            "DeSmuME Interface-VS2022-x64-Debug.dll",
        ]
    } else if cfg!(target_os = "macos") {
        vec!["libdesmume_tauri_bridge.dylib", "libdesmume.dylib"]
    } else {
        vec!["libdesmume_tauri_bridge.so", "libdesmume.so"]
    }
}

fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();

    for path in paths {
        let key = normalize_for_dedupe(&path);
        if seen.insert(key) {
            result.push(path);
        }
    }

    result
}

fn normalize_for_dedupe(path: &Path) -> String {
    path.to_string_lossy().to_lowercase()
}

#[cfg(target_os = "windows")]
fn prepare_dynamic_library_search(path: &Path) {
    let mut dirs = Vec::new();

    if let Some(parent) = path.parent() {
        dirs.push(parent.to_path_buf());

        if parent.file_name().and_then(|name| name.to_str()) == Some("__bins") {
            if let Some(interface_windows) = parent.parent() {
                dirs.push(interface_windows.join("SDL").join("lib").join("x64"));
                dirs.push(interface_windows.join("SDL").join("lib").join("x86"));
            }
        }
    }

    let mut path_entries: Vec<PathBuf> = dirs.into_iter().filter(|dir| dir.exists()).collect();
    if let Some(existing_path) = env::var_os("PATH") {
        path_entries.extend(env::split_paths(&existing_path));
    }

    if let Ok(joined_path) = env::join_paths(path_entries) {
        env::set_var("PATH", joined_path);
    }
}

#[cfg(not(target_os = "windows"))]
fn prepare_dynamic_library_search(_path: &Path) {}

fn lock_emulator<'a>(state: &'a State<'_, EmulatorState>) -> Result<MutexGuard<'a, Emulator>, String> {
    state
        .0
        .lock()
        .map_err(|_| "Emulator state lock is poisoned".to_string())
}

#[tauri::command]
fn bridge_status(state: State<'_, EmulatorState>) -> BridgeStatus {
    let mut emulator = match lock_emulator(&state) {
        Ok(emulator) => emulator,
        Err(error) => {
            return BridgeStatus {
                loaded: false,
                path: None,
                error: Some(error),
                frame_width: FRAME_WIDTH,
                frame_height: FRAME_HEIGHT,
            };
        }
    };

    if emulator.bridge.is_none() {
        if let Err(error) = emulator.ensure_bridge_loaded() {
            emulator.last_error = Some(error);
        }
    }

    emulator.status()
}

#[tauri::command]
fn emulator_info() -> EmulatorInfo {
    EmulatorInfo {
        frame_width: FRAME_WIDTH,
        frame_height: FRAME_HEIGHT,
        frame_bytes: FRAME_BYTES,
    }
}

#[tauri::command]
fn open_rom(path: String, state: State<'_, EmulatorState>) -> Result<(), String> {
    let rom_path = PathBuf::from(&path);
    if !rom_path.exists() {
        return Err(format!("ROM does not exist: {path}"));
    }

    let mut emulator = lock_emulator(&state)?;
    emulator.ensure_initialized()?;

    let bridge = emulator.bridge.as_ref().expect("bridge loaded");
    let c_path = CString::new(path.as_bytes()).map_err(|_| "ROM path contains a NUL byte")?;
    let result = unsafe { (bridge.open_rom)(c_path.as_ptr()) };

    if result != 0 {
        emulator.rom_loaded = false;
        emulator.paused = true;
        return Err(format!("ROM open failed with code {result}"));
    }

    emulator.rom_loaded = true;
    emulator.paused = false;
    emulator.last_error = None;

    Ok(())
}

#[tauri::command]
fn set_paused(paused: bool, state: State<'_, EmulatorState>) -> Result<(), String> {
    let mut emulator = lock_emulator(&state)?;
    emulator.ensure_initialized()?;

    let bridge = emulator.bridge.as_ref().expect("bridge loaded");
    unsafe {
        (bridge.set_paused)(if paused { 1 } else { 0 });
    }
    emulator.paused = paused;

    Ok(())
}

#[tauri::command]
fn reset(state: State<'_, EmulatorState>) -> Result<(), String> {
    let mut emulator = lock_emulator(&state)?;
    emulator.ensure_initialized()?;

    let bridge = emulator.bridge.as_ref().expect("bridge loaded");
    let result = unsafe { (bridge.reset)() };
    if result != 0 {
        return Err(format!("Reset failed with code {result}"));
    }

    emulator.paused = false;
    Ok(())
}

#[tauri::command]
fn set_keys(mask: u16, state: State<'_, EmulatorState>) -> Result<(), String> {
    let mut emulator = lock_emulator(&state)?;
    emulator.key_mask = mask;

    if let Some(bridge) = &emulator.bridge {
        unsafe {
            (bridge.set_key_mask)(mask);
        }
    }

    Ok(())
}

#[tauri::command]
fn frame(state: State<'_, EmulatorState>) -> Result<Response, String> {
    let mut emulator = lock_emulator(&state)?;
    emulator.ensure_initialized()?;

    if !emulator.rom_loaded {
        return Err("No ROM loaded".to_string());
    }

    let bridge = emulator.bridge.as_ref().expect("bridge loaded");
    unsafe {
        (bridge.set_key_mask)(emulator.key_mask);
    }

    let run_result = unsafe { (bridge.run_frame)() };
    if run_result != 0 {
        return Err(format!("Frame execution failed with code {run_result}"));
    }

    let mut bytes = vec![0; FRAME_BYTES];
    let frame_result = unsafe { (bridge.get_frame)(bytes.as_mut_ptr(), bytes.len()) };
    if frame_result != 0 {
        return Err(format!("Frame read failed with code {frame_result}"));
    }

    Ok(Response::new(bytes))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(EmulatorState::default())
        .invoke_handler(tauri::generate_handler![
            bridge_status,
            emulator_info,
            open_rom,
            set_paused,
            reset,
            set_keys,
            frame
        ])
        .run(tauri::generate_context!())
        .expect("error while running Tauri application");
}
