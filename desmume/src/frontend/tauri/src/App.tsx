import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import {
  FolderOpen,
  Gamepad2,
  Pause,
  Play,
  Power,
  RotateCcw,
  WifiOff
} from "lucide-react";

const FRAME_WIDTH = 256;
const FRAME_HEIGHT = 384;

const KEY_BITS: Record<string, number> = {
  KeyX: 1 << 0,
  KeyZ: 1 << 1,
  ShiftRight: 1 << 2,
  ShiftLeft: 1 << 2,
  Enter: 1 << 3,
  ArrowRight: 1 << 4,
  ArrowLeft: 1 << 5,
  ArrowUp: 1 << 6,
  ArrowDown: 1 << 7,
  KeyW: 1 << 8,
  KeyQ: 1 << 9,
  KeyS: 1 << 10,
  KeyA: 1 << 11
};

type BridgeStatus = {
  loaded: boolean;
  path: string | null;
  error: string | null;
  frame_width: number;
  frame_height: number;
};

type EmulatorInfo = {
  frame_width: number;
  frame_height: number;
  frame_bytes: number;
};

function bytesFromInvoke(value: unknown): Uint8Array {
  if (value instanceof ArrayBuffer) {
    return new Uint8Array(value);
  }

  if (value instanceof Uint8Array) {
    return value;
  }

  if (Array.isArray(value)) {
    return new Uint8Array(value);
  }

  throw new Error("Unexpected frame response");
}

function App() {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const pressedKeys = useRef<Set<string>>(new Set());
  const frameActive = useRef(true);
  const [bridge, setBridge] = useState<BridgeStatus | null>(null);
  const [info, setInfo] = useState<EmulatorInfo | null>(null);
  const [romName, setRomName] = useState<string>("No ROM loaded");
  const [paused, setPausedState] = useState(true);
  const [running, setRunning] = useState(false);
  const [status, setStatus] = useState("Waiting for native bridge");
  const [keyMask, setKeyMask] = useState(0);

  const bridgeMessage = useMemo(() => {
    if (!bridge) {
      return "Checking bridge";
    }

    if (bridge.loaded) {
      return bridge.path ?? "Bridge loaded";
    }

    return bridge.error ?? "Bridge unavailable";
  }, [bridge]);

  const refreshBridge = useCallback(async () => {
    try {
      const [bridgeStatus, emulatorInfo] = await Promise.all([
        invoke<BridgeStatus>("bridge_status"),
        invoke<EmulatorInfo>("emulator_info")
      ]);
      setBridge(bridgeStatus);
      setInfo(emulatorInfo);
      setStatus(bridgeStatus.loaded ? "Bridge ready" : "Native bridge not found");
    } catch (error) {
      setStatus(String(error));
    }
  }, []);

  const drawFrame = useCallback((bytes: Uint8Array) => {
    const canvas = canvasRef.current;
    const context = canvas?.getContext("2d");
    if (!canvas || !context || bytes.byteLength < FRAME_WIDTH * FRAME_HEIGHT * 4) {
      return;
    }

    const rgba = new Uint8ClampedArray(FRAME_WIDTH * FRAME_HEIGHT * 4);
    rgba.set(bytes.subarray(0, FRAME_WIDTH * FRAME_HEIGHT * 4));
    const image = new ImageData(rgba, FRAME_WIDTH, FRAME_HEIGHT);
    context.putImageData(image, 0, 0);
  }, []);

  const syncKeys = useCallback(async (mask: number) => {
    setKeyMask(mask);
    try {
      await invoke("set_keys", { mask });
    } catch (error) {
      setStatus(String(error));
    }
  }, []);

  useEffect(() => {
    refreshBridge();
  }, [refreshBridge]);

  useEffect(() => {
    const canvas = canvasRef.current;
    const context = canvas?.getContext("2d");
    if (!canvas || !context) {
      return;
    }

    context.fillStyle = "#050505";
    context.fillRect(0, 0, FRAME_WIDTH, FRAME_HEIGHT);
  }, []);

  useEffect(() => {
    frameActive.current = true;

    const loop = async () => {
      if (!frameActive.current) {
        return;
      }

      if (running) {
        try {
          const response = await invoke("frame");
          drawFrame(bytesFromInvoke(response));
        } catch (error) {
          setRunning(false);
          setStatus(String(error));
        }
      }

      requestAnimationFrame(loop);
    };

    requestAnimationFrame(loop);

    return () => {
      frameActive.current = false;
    };
  }, [drawFrame, running]);

  useEffect(() => {
    const updateMask = () => {
      let nextMask = 0;
      for (const code of pressedKeys.current) {
        nextMask |= KEY_BITS[code] ?? 0;
      }
      syncKeys(nextMask);
    };

    const onKeyDown = (event: KeyboardEvent) => {
      if (!(event.code in KEY_BITS)) {
        return;
      }
      event.preventDefault();
      if (!pressedKeys.current.has(event.code)) {
        pressedKeys.current.add(event.code);
        updateMask();
      }
    };

    const onKeyUp = (event: KeyboardEvent) => {
      if (!(event.code in KEY_BITS)) {
        return;
      }
      event.preventDefault();
      pressedKeys.current.delete(event.code);
      updateMask();
    };

    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("keyup", onKeyUp);

    return () => {
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("keyup", onKeyUp);
    };
  }, [syncKeys]);

  const openRom = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [
          {
            name: "Nintendo DS ROM",
            extensions: ["nds", "srl", "zip", "7z", "rar"]
          }
        ]
      });

      if (typeof selected !== "string") {
        return;
      }

      await invoke("open_rom", { path: selected });
      setRomName(selected.split(/[\\/]/).pop() ?? selected);
      setPausedState(false);
      setRunning(true);
      setStatus("Running");
      await refreshBridge();
    } catch (error) {
      setRunning(false);
      setStatus(String(error));
    }
  };

  const togglePause = async () => {
    const nextPaused = !paused;
    try {
      await invoke("set_paused", { paused: nextPaused });
      setPausedState(nextPaused);
      setRunning(!nextPaused);
      setStatus(nextPaused ? "Paused" : "Running");
    } catch (error) {
      setStatus(String(error));
    }
  };

  const reset = async () => {
    try {
      await invoke("reset");
      setPausedState(false);
      setRunning(true);
      setStatus("Reset");
    } catch (error) {
      setStatus(String(error));
    }
  };

  return (
    <main className="appShell">
      <section className="topBar">
        <div className="brand">
          <Gamepad2 aria-hidden="true" />
          <div>
            <h1>DeSmuME Tauri</h1>
            <p>{romName}</p>
          </div>
        </div>

        <div className="actions">
          <button type="button" className="primary" onClick={openRom} title="Open ROM">
            <FolderOpen aria-hidden="true" />
            <span>Open ROM</span>
          </button>
          <button type="button" onClick={togglePause} title={paused ? "Resume" : "Pause"}>
            {paused ? <Play aria-hidden="true" /> : <Pause aria-hidden="true" />}
          </button>
          <button type="button" onClick={reset} title="Reset">
            <RotateCcw aria-hidden="true" />
          </button>
        </div>
      </section>

      <section className="workspace">
        <div className="screenColumn">
          <div className="screenFrame">
            <canvas ref={canvasRef} width={FRAME_WIDTH} height={FRAME_HEIGHT} />
          </div>
        </div>

        <aside className="sidePanel">
          <div className="statusBlock">
            <span className={bridge?.loaded ? "statusDot ready" : "statusDot"} />
            <div>
              <h2>{status}</h2>
              <p>{bridgeMessage}</p>
            </div>
          </div>

          <div className="metricGrid">
            <div>
              <span>Frame</span>
              <strong>{info ? `${info.frame_width} x ${info.frame_height}` : "256 x 384"}</strong>
            </div>
            <div>
              <span>Input</span>
              <strong>0x{keyMask.toString(16).padStart(4, "0")}</strong>
            </div>
          </div>

          <div className="inputPanel">
            <h2>Input</h2>
            <div className="inputPad">
              {["Up", "Down", "Left", "Right", "A", "B", "X", "Y", "L", "R", "Start", "Select"].map(
                (label) => (
                  <span key={label}>{label}</span>
                )
              )}
            </div>
          </div>

          {!bridge?.loaded && (
            <div className="bridgeNotice">
              <WifiOff aria-hidden="true" />
              <p>Native bridge unavailable.</p>
            </div>
          )}

          <div className="powerState">
            <Power aria-hidden="true" />
            <span>{running ? "Core active" : "Core idle"}</span>
          </div>
        </aside>
      </section>
    </main>
  );
}

export default App;
