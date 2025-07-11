import sounddevice as sd
import numpy as np
from scipy import signal
import click
import queue
import sys
from scipy.signal import butter, lfilter, freqz

# Global settings
SAMPLE_RATE = 44100
CHANNELS = 1
CHUNK_SIZE = 1024
DEVICE = None

# Audio processing settings
NOISE_THRESHOLD = 0.02
BUFFER_SIZE = int(SAMPLE_RATE * 0.1)  # Restored to 100ms
GAIN = 0.8
FREQ_SHIFT = 5  # Slight frequency shift in Hz
DECAY_FACTOR = 0.1  # Quick decay for residual noise

# Voice frequency settings
VOICE_LOW_FREQ = 85   # Hz - insan sesi alt sınır
VOICE_HIGH_FREQ = 3400  # Hz - insan sesi üst sınır

# Noise reduction history
prev_output = np.zeros(CHUNK_SIZE)
prev_input = np.zeros(CHUNK_SIZE)


def butter_bandpass(lowcut, highcut, fs, order=5):
    nyq = 0.5 * fs
    low = lowcut / nyq
    high = highcut / nyq
    b, a = butter(order, [low, high], btype='band')
    return b, a

def apply_bandpass_filter(data, lowcut=300, highcut=3400):
    b, a = butter_bandpass(lowcut, highcut, SAMPLE_RATE)
    y = lfilter(b, a, data)
    return y

def noise_gate(data, threshold=NOISE_THRESHOLD):
    mask = np.abs(data) < threshold
    data[mask] = 0
    return data

def echo_cancellation(current_input, prev_input, prev_output):
    # Simple echo cancellation using adaptive filtering
    echo_estimate = prev_output * 0.6  # Estimated echo from previous output
    cancelled = current_input - echo_estimate
    return cancelled

# Queue for audio processing
audio_queue = queue.Queue()

def audio_callback(indata, frames, time, status):
    """Callback function for audio stream"""
    if status:
        print(f'Status: {status}', file=sys.stderr)
    audio_queue.put(indata.copy())

def apply_frequency_shift(data, freq_shift):
    """Apply a subtle frequency shift while preserving voice characteristics"""
    length = len(data)
    t = np.arange(length) / SAMPLE_RATE
    shifted = data * np.exp(2j * np.pi * freq_shift * t)
    return np.real(shifted)

# Hold counter for decay
hold_counter = 0

def process_audio(data):
    """Process audio with frequency shifting and noise control"""
    global prev_output
    try:
        # Convert to mono
        audio = data.flatten()
        
        # Apply voice-specific bandpass filter
        audio = apply_bandpass_filter(audio, lowcut=VOICE_LOW_FREQ, highcut=VOICE_HIGH_FREQ)
        
        # Calculate signal level after filtering
        current_level = np.max(np.abs(audio))
        
        # Apply noise gate
        is_signal = current_level > NOISE_THRESHOLD
        if not is_signal:
            # Apply decay to residual noise
            audio = prev_output * DECAY_FACTOR
            if np.max(np.abs(audio)) < 0.001:
                audio = np.zeros_like(audio)
        else:
            # Normalize and apply gain
            audio = audio / current_level * GAIN if current_level > 0 else audio
            
            # Apply frequency shift to prevent feedback
            audio = apply_frequency_shift(audio, FREQ_SHIFT)
            
            # Smooth transition
            if len(prev_output) > 0:
                fade_len = min(128, len(audio))
                fade_in = np.linspace(0, 1, fade_len)
                audio[:fade_len] = audio[:fade_len] * fade_in
        
        prev_output = audio.copy()
        return audio
        
    except Exception as e:
        print(f"\rAudio processing error: {str(e)}", end="")
        return np.zeros_like(data.flatten())
        
    except Exception as e:
        print(f"\rAudio processing error: {str(e)}", end="")
        return np.zeros_like(data.flatten())


@click.command()
@click.option('--list-devices', is_flag=True, help='List all audio devices')
@click.option('--device', type=int, help='Input device ID')
def main(list_devices, device):
    """Real-time voice transformer CLI"""
    if list_devices:
        print(sd.query_devices())
        return

    global DEVICE
    DEVICE = device

    try:
        # Get default output device
        output_device = sd.default.device[1]
        
        with sd.InputStream(
            device=DEVICE,
            channels=CHANNELS,
            samplerate=SAMPLE_RATE,
            blocksize=CHUNK_SIZE,
            callback=audio_callback
        ), sd.OutputStream(
            device=output_device,
            channels=CHANNELS,
            samplerate=SAMPLE_RATE,
            blocksize=CHUNK_SIZE
        ) as output_stream:
            
            print("Voice transformer started! Press Ctrl+C to stop.")
            print("Processing audio in real-time...")

            while True:
                # Get audio data from queue
                audio_data = audio_queue.get()
                
                # Process audio
                processed_audio = process_audio(audio_data)
                
                # Play processed audio
                output_stream.write(processed_audio.astype(np.float32))

    except KeyboardInterrupt:
        print("\nStopping voice transformer...")
    except Exception as e:
        print(f"Error: {str(e)}")

if __name__ == '__main__':
    main()
