# Voice Transformer v1.0

Real-time voice processing tool with minimal latency. Features include:
- Voice frequency isolation (85-3400 Hz)
- Feedback prevention
- Noise reduction
- Echo cancellation

## Requirements
- Python 3.8+
- PortAudio (for audio processing)
- Required Python packages listed in `requirements.txt`

## Installation

1. Clone the repository
```bash
git clone https://github.com/YourUsername/voice-transformer.git
cd voice-transformer
```

2. Create and activate virtual environment
```bash
python -m venv venv
# On Windows
.\venv\Scripts\activate
# On Linux/Mac
source venv/bin/activate
```

3. Install dependencies
```bash
pip install -r requirements.txt
```

## Usage

1. List available audio devices:
```bash
python main.py --list-devices
```

2. Run with specific input device:
```bash
python main.py --device DEVICE_ID
```

Replace `DEVICE_ID` with the ID of your input device from the list.

## Features

- **Voice Frequency Isolation**: Focuses on human voice frequencies (85-3400 Hz)
- **Feedback Prevention**: Uses subtle frequency shifting to prevent audio feedback
- **Noise Reduction**: Implements noise gate and adaptive filtering
- **Low Latency**: Minimal processing delay for real-time use

## License
MIT License
