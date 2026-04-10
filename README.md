# 🔊 Rust Soundboard

A shockingly fast, lightweight, and performant cross-platform soundboard

It uses the brilliant immediate-mode GUI library **egui** for the interface, and **cpal / rodio** for the low-level audio engine, giving you the ability to target virtual audio cables to seamlessly pipe sound into communication apps like Discord or in-game voice chat.

## ✨ Features

- **Dual Audio Pipeline (Local Echo)**: It outputs to a primary broadcast device (like Virtual Audio Cable) *and* simultaneously clones the audio to your local speakers, so you can monitor what you're playing.
- **Isolated Live Volume Control**: Adjust the **Virtual Output** volume separately from your **Local Speaker** echo volume. Change volumes on the fly—the system dynamically scales the decoders in real-time.
- **Targeted Audio Delivery**: A device dropdown menu specifically detects and allows you to bind directly to `VB-CABLE Input`, bypassing your operating system's default mixer.
- **Folder and Tab Organization**: Load entire directories of sound bytes. The app parses `.mp3`, `.wav`, `.ogg`, and `.flac`, placing them in tabs for fast, responsive navigation.
- **Per-Sound Tweaking**: Each sound tile contains its own granular volume level. Have a quiet clip? Just turn up that single slider.
- **Persistent State**: The Soundboard auto-saves its volume values, custom directories, selected output device, and window state. When you open the app again, you pick up exactly where you left off.
- **Lightning Search**: Search for sounds rapidly with the real-time filter bar.
- **Low Memory Footprint**: Minimal CPU and RAM usage, guaranteeing that your gaming or audio recording performance isn't degraded.

## 🛠️ Requirements & Setup

> [!IMPORTANT]
> **CRITICAL FIRST STEP:** You **must** install a virtual audio driver on your machine before using this software, or the app's external routing features will not function correctly.
> * **Windows**: Download and install [VB-AUDIO Virtual Cable](https://download.vb-audio.com/Download_CABLE/VBCABLE_Driver_Pack45.zip).
> * **Linux**: Set up a virtual null sink via PulseAudio or PipeWire.

### 2. Building

Ensure you have Rust and Cargo installed via [rustup](https://rustup.rs/). Then clone the repo and run:

```bash
cargo check
cargo build --release
```

### 3. Running the Application

You can start the app via Cargo:
```bash
cargo run --release
```

**Executing the Compiled Binary Directly:**
For everyday use, you do not need to use `cargo`. After building, the standalone executable is located in the `target/release` folder:
* **Windows**: `target\release\soundboard.exe` (Double-click to run, or create a desktop shortcut).
* **Linux**: `./target/release/soundboard`

### 4. Audio Routing
2. In the top bar, set `Output:` to **CABLE Input (VB-Audio Virtual Cable)**.
3. Check the **🎧 Echo to Speakers** box to hear the sounds yourself. Use the *Virtual Vol* slider to adjust what your friends hear, and *Local Vol* for what you hear!
4. **Link to Discord / App**: Go to your communication app's Voice Settings. Set your **Input Device / Microphone** to **CABLE Output (VB-Audio Virtual Cable)**. 

## 🏗️ Architecture

- `src/main.rs`: Orchestrator that bridges the GUI with the audio engine configuration.
- `src/audio.rs`: The beating heart of the program. Configures isolated `cpal` threads and `rodio` sinks, managing state locks and dynamic multi-volume crossfading perfectly.
- `src/gui.rs`: Everything visual. Runs on `eframe::App`, drawing the grid, sliders, and device selection combo boxes effortlessly at native 60fps+.
- `src/sound.rs`: Handles recursive scanning (`walkdir`) and parses the filesystem to inject entries to the interface.
- `src/config.rs`: Reads/writes preferences using `serde_json`.

## 📜 License

This project is licensed under the MIT License.
