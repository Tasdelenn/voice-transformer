class AudioVisualizer {
    constructor() {
        this.canvas = document.getElementById('visualizer');
        this.ctx = this.canvas.getContext('2d');
        this.socket = null;
        this.animationId = null;
        
        this.inputSpectrum = [];
        this.outputSpectrum = [];
        this.sampleRate = 44100;
        this.fftSize = 1024;
        
        this.setupCanvas();
        this.connectWebSocket();
        this.startAnimation();
    }
    
    setupCanvas() {
        this.canvas.width = window.innerWidth;
        this.canvas.height = window.innerHeight;
        
        window.addEventListener('resize', () => {
            this.canvas.width = window.innerWidth;
            this.canvas.height = window.innerHeight;
        });
    }
    
    connectWebSocket() {
        const protocol = window.location.protocol === 'https:' ? 'wss' : 'ws';
        const host = window.location.host;
        this.socket = new WebSocket(`${protocol}://${host}/ws`);
        
        this.socket.onopen = () => {
            console.log('WebSocket connected');
        };
        
        this.socket.onmessage = (event) => {
            try {
                const data = JSON.parse(event.data);
                console.log('Received WebSocket data:', data); // Debug: log all received data
                if (data.type === 'fft_data') {
                    this.inputSpectrum = data.input_spectrum || [];
                    this.outputSpectrum = data.output_spectrum || [];
                    this.sampleRate = data.sample_rate || 44100;
                    this.fftSize = data.fft_size || 1024;
                    
                    // Debug: log spectrum data
                    console.log(`Input spectrum length: ${this.inputSpectrum.length}, first 5 values:`, this.inputSpectrum.slice(0, 5));
                    console.log(`Output spectrum length: ${this.outputSpectrum.length}, first 5 values:`, this.outputSpectrum.slice(0, 5));
                }
            } catch (e) {
                console.error('Error parsing WebSocket data:', e);
                console.log('Raw event data:', event.data); // Debug: show raw data on error
            }
        };
        
        this.socket.onclose = () => {
            console.log('WebSocket disconnected');
            // Try to reconnect after 2 seconds
            setTimeout(() => this.connectWebSocket(), 2000);
        };
        
        this.socket.onerror = (error) => {
            console.error('WebSocket error:', error);
        };
    }
    
    startAnimation() {
        const animate = () => {
            this.draw();
            this.animationId = requestAnimationFrame(animate);
        };
        animate();
    }
    
    draw() {
        const { width, height } = this.canvas;
        
        // Clear canvas
        this.ctx.fillStyle = '#000';
        this.ctx.fillRect(0, 0, width, height);
        
        // Draw input spectrum (top half)
        this.drawSpectrum(this.inputSpectrum, 0, height / 2, 'INPUT');
        
        // Draw output spectrum (bottom half)
        this.drawSpectrum(this.outputSpectrum, height / 2, height / 2, 'OUTPUT');
        
        // Draw center line
        this.ctx.strokeStyle = '#333';
        this.ctx.lineWidth = 2;
        this.ctx.beginPath();
        this.ctx.moveTo(0, height / 2);
        this.ctx.lineTo(width, height / 2);
        this.ctx.stroke();
    }
    
    drawSpectrum(spectrum, yOffset, sectionHeight, label) {
        if (!spectrum || spectrum.length === 0) return;
        
        const { width } = this.canvas;
        const barWidth = width / spectrum.length;
        const maxMagnitude = Math.max(...spectrum, 0.01);
        
        // Draw frequency bars
        for (let i = 0; i < spectrum.length; i++) {
            const magnitude = spectrum[i];
            const normalizedMagnitude = magnitude / maxMagnitude;
            const barHeight = normalizedMagnitude * sectionHeight * 0.8;
            
            const x = i * barWidth;
            const y = yOffset + sectionHeight - barHeight;
            
            // Color coding by frequency
            const freq = (i / spectrum.length) * (this.sampleRate / 2);
            const color = this.getFrequencyColor(freq);
            
            this.ctx.fillStyle = color;
            this.ctx.fillRect(x, y, barWidth - 1, barHeight);
        }
        
        // Draw label
        this.ctx.fillStyle = '#fff';
        this.ctx.font = '16px monospace';
        this.ctx.fillText(label, 10, yOffset + 25);
        
        // Draw frequency scale
        this.drawFrequencyScale(yOffset, sectionHeight);
    }
    
    getFrequencyColor(freq) {
        if (freq < 250) return '#ff4444';      // Red for bass
        if (freq < 500) return '#ffff44';      // Yellow for low-mid
        if (freq < 2000) return '#44ff44';     // Green for mid
        if (freq < 6000) return '#44ffff';     // Cyan for high-mid
        return '#ff44ff';                      // Magenta for high
    }
    
    drawFrequencyScale(yOffset, sectionHeight) {
        const { width } = this.canvas;
        const steps = 8;
        
        this.ctx.fillStyle = '#888';
        this.ctx.font = '12px monospace';
        
        for (let i = 0; i <= steps; i++) {
            const x = (i / steps) * width;
            const freq = (i / steps) * (this.sampleRate / 2);
            const label = freq < 1000 ? `${freq.toFixed(0)}Hz` : `${(freq/1000).toFixed(1)}kHz`;
            
            this.ctx.fillText(label, x + 5, yOffset + sectionHeight - 10);
        }
    }
}

// Initialize when DOM is loaded
document.addEventListener('DOMContentLoaded', () => {
    new AudioVisualizer();
});
