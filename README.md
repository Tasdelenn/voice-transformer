# Voice Transformer

**Voice Transformer** is a low-latency, real-time audio streaming and processing system designed to transmit high-quality voice audio over a local network (WiFi). It features a custom DSP pipeline for feedback suppression and noise control, making it ideal for using a smartphone or laptop as a wireless microphone for a sound mixer or PA system.

## Features

*   **Low Latency Transport:** Utilizes **QUIC** (via `quinn`) for reliable, ultra-low latency audio streaming over UDP.
*   **Real-Time DSP Pipeline:**
    *   **Acoustic Echo Cancellation (Feedback Suppression):** Uses Frequency Shifting (Pitch Shifting) to break feedback loops.
    *   **Noise Gate:** Automatically mutes audio when below a set threshold to eliminate background hiss.
    *   **Band-Pass Filter:** Focuses on human voice frequencies (300Hz - 3400Hz) to remove rumble and high-frequency noise.
*   **Interactive TUI (Text User Interface):**
    *   **Real-Time Spectrum Analyzer:** Visualize audio frequencies to diagnose feedback or noise.
    *   **Live Controls:** Adjust Gain, Threshold, and Frequency Shift on the fly without restarting.
    *   **VU Meter:** Monitor input/output levels visually.
*   **Cross-Platform:** Written in Rust, runs on Windows, Linux, macOS, and Raspberry Pi.

## Architecture

The system consists of two binaries:

1.  **Sender (Microphone):** Captures audio, applies DSP processing, and streams it to the receiver.
    *   *Target Devices:* Laptops, Mobile Devices (via Termux or future app), Raspberry Pi.
2.  **Receiver (Speaker/Mixer):** Listens for incoming streams and plays audio to the default output device.
    *   *Target Devices:* Desktop PC connected to Mixer, Raspberry Pi, etc.

## Installation

### Prerequisites
*   **Rust:** Install the latest stable version from [rustup.rs](https://rustup.rs/).
*   **Build Tools:**
    *   *Windows:* Visual Studio C++ Build Tools.
    *   *Linux/RPi:* `build-essential`, `libasound2-dev` (ALSA), `pkg-config`.

### Building
Clone the repository and build the project in release mode for optimal performance:

```bash
git clone https://github.com/Tasdelenn/voice-transformer.git
cd voice-transformer
cargo build --release
```

## Usage

### 1. Start the Receiver
Run the receiver on the device connected to your speakers or sound mixer.

```bash
# Listen on all interfaces, port 5000
./target/release/receiver --port 5000
```

### 2. Start the Sender
Run the sender on the device acting as the microphone.

```bash
# Connect to the receiver's IP address (e.g., 192.168.1.100)
# Use --device <INDEX> to select a specific microphone (use --list-devices to see options)
./target/release/sender --ip 192.168.1.100 --port 5000 --device 0
```

### Sender TUI Controls
Once the sender is running, you can control it using the keyboard:

| Key | Action | Description |
| :--- | :--- | :--- |
| `v` / `V` | **Volume** +/- | Increase or decrease gain. |
| `t` / `T` | **Threshold** +/- | Adjust noise gate threshold. Increase to cut background noise. |
| `f` / `F` | **Freq Shift** +/- | Adjust frequency shift amount (Hz). Use ~5Hz to stop feedback. |
| `b` | **Bandpass** | Toggle the 300Hz-3400Hz voice filter. |
| `q` | **Quit** | Stop the application. |

## Roadmap

- [x] Core Audio Transport (QUIC)
- [x] Basic DSP (Noise Gate, Band-Pass, Freq Shift)
- [x] TUI with Spectrum Visualization
- [ ] **Mobile App:** Native Android/iOS sender application.
- [ ] **AI Voice Conversion:** Real-time voice changing using RVC/So-VITS.
- [ ] **Speech-to-Text:** Real-time transcription overlay.

## License
MIT
