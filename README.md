# Voice Transformer v1.0 (Rust Edition)

A real-time voice processing tool written in Rust, designed for minimal latency. This project was originally started in Python and has been completely rewritten in Rust for better performance and control.

## Features

- **Low-Latency Audio Processing**: Directly processes audio streams using `cpal`.
- **Feedback Prevention**: Implements a subtle frequency shift to prevent audio feedback loops in real-time.
- **Noise Reduction**: Includes a basic noise gate to filter out background noise below a certain threshold.
- **Interactive Real-Time Controls**: Adjust parameters like volume, noise gate threshold, frequency shift, and buffer size while the application is running.

## Requirements

- **Rust**: Install from [rustup.rs](https://rustup.rs/).
- **System Audio Libraries**: The `cpal` crate may require system libraries for audio I/O.
    - **Linux (Debian/Ubuntu)**: `sudo apt-get install libasound2-dev`
    - **Windows/macOS**: Dependencies are usually handled by the system.

## Installation and Building

1.  **Clone the repository:**
    ```bash
    git clone https://github.com/Tasdelenn/voice-transformer.git
    cd voice-transformer
    ```

2.  **Build the project:**
    For a development build:
    ```bash
    cargo build
    ```
    For an optimized release build (recommended for running):
    ```bash
    cargo build --release
    ```

## Usage

The compiled executable will be in `./target/release/` or `./target/debug/`.

1.  **List available audio devices:**
    ```bash
    cargo run -- --list-devices
    ```
    This will show you a list of input device IDs you can use.

2.  **Run the application:**
    To run with the default input device:
    ```bash
    cargo run --release
    ```
    To run with a specific input device:
    ```bash
    cargo run --release -- --device <DEVICE_ID>
    ```
    Replace `<DEVICE_ID>` with the ID of your input device from the list.

3.  **Interactive Commands:**
    Once running, you can use the following keys to adjust settings in real-time:
    - `v`: Change volume (0.0 - 1.0).
    - `n`: Change noise gate threshold (0.0 - 0.1).
    - `f`: Change frequency shift (in Hz).
    - `b`: Change the audio buffer size.
    - `d`: Reset all settings to their default values.
    - `s`: Display the current settings.
    - `q`: Quit the application.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
