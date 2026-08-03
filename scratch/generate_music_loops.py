import math
import struct
import wave
import array
import random
import os

SAMPLE_RATE = 44100

# Precompute sine table for 10x faster synthesis in pure Python
TABLE_SIZE = 65536
SINE_TABLE = [math.sin(2.0 * math.pi * i / TABLE_SIZE) for i in range(TABLE_SIZE)]

def fast_sin(phase):
    """Fast sine lookup from 0.0 to 1.0 phase."""
    idx = int((phase % 1.0) * TABLE_SIZE)
    return SINE_TABLE[idx]

def note_to_freq(note_name):
    """Convert note name like 'C4', 'F#3', 'Eb5' to Hz."""
    notes = {'C': 0, 'D': 2, 'E': 4, 'F': 5, 'G': 7, 'A': 9, 'B': 11}
    name = note_name[:-1]
    octave = int(note_name[-1])
    
    base = notes[name[0]]
    if len(name) > 1:
        if name[1] == '#':
            base += 1
        elif name[1] == 'b':
            base -= 1
            
    midi = 12 * (octave + 1) + base
    return 440.0 * (2.0 ** ((midi - 69) / 12.0))

class StereoBuffer:
    def __init__(self, duration_sec):
        self.num_samples = int(SAMPLE_RATE * duration_sec)
        self.left = array.array('f', [0.0] * self.num_samples)
        self.right = array.array('f', [0.0] * self.num_samples)
        self.duration = duration_sec

    def add_sample(self, index, l_val, r_val):
        idx = index % self.num_samples
        self.left[idx] += l_val
        self.right[idx] += r_val

    def apply_reverb(self, delay_ms=120, feedback=0.35, wet=0.25):
        delay_samples = int(SAMPLE_RATE * (delay_ms / 1000.0))
        new_l = array.array('f', self.left)
        new_r = array.array('f', self.right)
        
        for i in range(self.num_samples):
            prev_l = new_l[(i - delay_samples) % self.num_samples]
            prev_r = new_r[(i - delay_samples * 7 // 5) % self.num_samples]
            
            new_l[i] += prev_r * feedback
            new_r[i] += prev_l * feedback
            
        for i in range(self.num_samples):
            self.left[i] = self.left[i] * (1.0 - wet) + new_l[i] * wet
            self.right[i] = self.right[i] * (1.0 - wet) + new_r[i] * wet

    def normalize(self, target_peak=0.70):
        max_val = max(max(abs(x) for x in self.left), max(abs(x) for x in self.right))
        if max_val < 1e-6:
            return
            
        scale = target_peak / max_val
        for i in range(self.num_samples):
            self.left[i] *= scale
            self.right[i] *= scale

    def export_wav(self, filepath):
        self.normalize()
        out = bytearray()
        for i in range(self.num_samples):
            l_int = max(-32767, min(32767, int(self.left[i] * 32767)))
            r_int = max(-32767, min(32767, int(self.right[i] * 32767)))
            out.extend(struct.pack('<hh', l_int, r_int))
            
        with wave.open(filepath, 'wb') as f:
            f.setnchannels(2)
            f.setsampwidth(2)
            f.setframerate(SAMPLE_RATE)
            f.writeframes(out)
        print(f"Exported {filepath} ({self.duration:.2f}s)")

# ------------------------------------------------------------- FAST SYNTH HELPERS

def render_bell_note(buf, start_sec, freq, duration_sec, amplitude=0.3, pan=0.5):
    """Crystal / Glockenspiel bell synth note."""
    num_samples = int(SAMPLE_RATE * duration_sec)
    start_idx = int(SAMPLE_RATE * start_sec)
    
    l_pan = math.cos(pan * math.pi / 2)
    r_pan = math.sin(pan * math.pi / 2)
    
    phase_inc1 = freq / SAMPLE_RATE
    phase_inc2 = (freq * 2.002) / SAMPLE_RATE
    phase_inc3 = (freq * 3.01) / SAMPLE_RATE
    phase_inc4 = (freq * 5.4) / SAMPLE_RATE
    
    dec_rate = -4.5 / (duration_sec * SAMPLE_RATE)
    
    p1 = p2 = p3 = p4 = 0.0
    
    for i in range(num_samples):
        t = i / SAMPLE_RATE
        env = math.exp(dec_rate * i) * (1.0 - math.exp(-200.0 * t))
        
        v1 = fast_sin(p1)
        v2 = 0.5 * fast_sin(p2)
        v3 = 0.25 * fast_sin(p3)
        v4 = 0.12 * fast_sin(p4)
        
        p1 += phase_inc1
        p2 += phase_inc2
        p3 += phase_inc3
        p4 += phase_inc4
        
        sig = (v1 + v2 + v3 + v4) * env * amplitude
        buf.add_sample(start_idx + i, sig * l_pan, sig * r_pan)

def render_plucked_string(buf, start_sec, freq, duration_sec, amplitude=0.3, pan=0.5):
    """Acoustic plucked string (harp / lute)."""
    num_samples = int(SAMPLE_RATE * duration_sec)
    start_idx = int(SAMPLE_RATE * start_sec)
    
    l_pan = math.cos(pan * math.pi / 2)
    r_pan = math.sin(pan * math.pi / 2)
    
    p1_inc = freq / SAMPLE_RATE
    p2_inc = (freq * 2) / SAMPLE_RATE
    p3_inc = (freq * 3) / SAMPLE_RATE
    p4_inc = (freq * 4) / SAMPLE_RATE
    body_inc = (freq * 0.5) / SAMPLE_RATE
    
    dec_rate = -3.2 / (duration_sec * SAMPLE_RATE)
    body_dec = -6.0 / SAMPLE_RATE
    
    p1 = p2 = p3 = p4 = p_body = 0.0
    
    for i in range(num_samples):
        t = i / SAMPLE_RATE
        env = (1.0 - math.exp(-300.0 * t)) * math.exp(dec_rate * i)
        body_env = math.exp(body_dec * i)
        
        h1 = fast_sin(p1)
        h2 = 0.4 * fast_sin(p2)
        h3 = 0.2 * fast_sin(p3)
        h4 = 0.1 * fast_sin(p4)
        body = 0.15 * fast_sin(p_body) * body_env
        
        p1 += p1_inc
        p2 += p2_inc
        p3 += p3_inc
        p4 += p4_inc
        p_body += body_inc
        
        sig = (h1 + h2 + h3 + h4 + body) * env * amplitude
        buf.add_sample(start_idx + i, sig * l_pan, sig * r_pan)

def render_pad_chord(buf, start_sec, chord_freqs, duration_sec, amplitude=0.2, pan=0.5, bright=0.5):
    """Warm lush string / synth pad chord."""
    num_samples = int(SAMPLE_RATE * duration_sec)
    start_idx = int(SAMPLE_RATE * start_sec)
    
    l_pan = math.cos(pan * math.pi / 2)
    r_pan = math.sin(pan * math.pi / 2)
    
    phases = [[0.0, 0.0, 0.0, 0.0] for _ in chord_freqs]
    incs = []
    for f in chord_freqs:
        incs.append([
            f / SAMPLE_RATE,
            (f * 1.003) / SAMPLE_RATE,
            (f * 2.0) / SAMPLE_RATE,
            (f * 1.001) / SAMPLE_RATE
        ])
        
    num_chords = len(chord_freqs)
    
    for i in range(num_samples):
        t = i / SAMPLE_RATE
        attack = min(1.0, t / (duration_sec * 0.25))
        release = min(1.0, (duration_sec - t) / (duration_sec * 0.25))
        env = attack * release
        
        sig = 0.0
        for idx in range(num_chords):
            p = phases[idx]
            inc = incs[idx]
            
            v1 = fast_sin(p[0])
            v2 = fast_sin(p[1])
            v3 = 0.5 * fast_sin(p[2]) * bright
            v4 = fast_sin(p[3])
            
            p[0] += inc[0]
            p[1] += inc[1]
            p[2] += inc[2]
            p[3] += inc[3]
            
            sig += (v1 + v2 + v3 + v4) / num_chords
            
        final_sig = sig * env * amplitude
        buf.add_sample(start_idx + i, final_sig * l_pan, final_sig * r_pan)

def render_flute_melody(buf, start_sec, freq, duration_sec, amplitude=0.25, pan=0.5):
    """Expressive woodwind / flute note."""
    num_samples = int(SAMPLE_RATE * duration_sec)
    start_idx = int(SAMPLE_RATE * start_sec)
    
    l_pan = math.cos(pan * math.pi / 2)
    r_pan = math.sin(pan * math.pi / 2)
    
    p1 = p2 = p3 = 0.0
    
    for i in range(num_samples):
        t = i / SAMPLE_RATE
        attack = min(1.0, t / 0.12)
        release = min(1.0, (duration_sec - t) / 0.15)
        env = attack * release
        
        vib_depth = min(1.0, t / 0.3) * 0.008
        vib = fast_sin(5.0 * t) * vib_depth
        f_curr = freq * (1.0 + vib)
        
        w1 = fast_sin(p1)
        w2 = 0.15 * fast_sin(p2)
        w3 = 0.05 * fast_sin(p3)
        
        p1 += f_curr / SAMPLE_RATE
        p2 += (f_curr * 2) / SAMPLE_RATE
        p3 += (f_curr * 3) / SAMPLE_RATE
        
        sig = (w1 + w2 + w3) * env * amplitude
        buf.add_sample(start_idx + i, sig * l_pan, sig * r_pan)

# ------------------------------------------------------------- COMPOSITIONS

def generate_winter_hymn(output_path):
    duration = 32.0
    buf = StereoBuffer(duration)
    
    chord1 = [note_to_freq('C2'), note_to_freq('G2'), note_to_freq('Eb3')]
    chord2 = [note_to_freq('Ab1'), note_to_freq('Eb2'), note_to_freq('C3')]
    chord3 = [note_to_freq('F2'), note_to_freq('C3'), note_to_freq('Ab3')]
    chord4 = [note_to_freq('G2'), note_to_freq('D3'), note_to_freq('B3')]
    
    chords = [(0.0, 8.0, chord1), (8.0, 8.0, chord2), (16.0, 8.0, chord3), (24.0, 8.0, chord4)]
    
    for start, dur, ch in chords:
        render_pad_chord(buf, start, ch, dur, amplitude=0.25, pan=0.5, bright=0.3)
        render_pad_chord(buf, start, [f * 2.0 for f in ch], dur, amplitude=0.12, pan=0.4, bright=0.5)

    melody = [
        (0.0, 'G4', 1.5), (1.5, 'C5', 1.5), (3.0, 'Eb5', 2.0), (5.0, 'D5', 1.5), (6.5, 'C5', 1.5),
        (8.0, 'Eb5', 1.5), (9.5, 'F5', 1.5), (11.0, 'G5', 2.5), (13.5, 'F5', 1.5), (15.0, 'Eb5', 1.0),
        (16.0, 'C5', 1.5), (17.5, 'D5', 1.5), (19.0, 'Eb5', 2.0), (21.0, 'C5', 1.5), (22.5, 'Ab4', 1.5),
        (24.0, 'G4', 2.0), (26.0, 'B4', 2.0), (28.0, 'C5', 4.0)
    ]
    
    for t_off, n_name, dur in melody:
        freq = note_to_freq(n_name)
        render_bell_note(buf, t_off, freq, dur * 1.2, amplitude=0.28, pan=0.35 + 0.3 * (t_off / 32.0))
        render_bell_note(buf, t_off + 0.05, freq * 0.5, dur * 1.5, amplitude=0.10, pan=0.6)

    buf.apply_reverb(delay_ms=180, feedback=0.42, wet=0.30)
    buf.export_wav(output_path)


def generate_gathering_storm(output_path):
    duration = 30.0
    buf = StereoBuffer(duration)
    
    beat_sec = 0.5 # 120 BPM
    total_beats = int(duration / beat_sec)
    
    bass_notes = ['D2', 'D2', 'F2', 'D2', 'A2', 'D2', 'Bb2', 'A2']
    for b in range(total_beats):
        t_off = b * beat_sec
        note = bass_notes[b % len(bass_notes)]
        freq = note_to_freq(note)
        render_plucked_string(buf, t_off, freq * 0.5, 0.45, amplitude=0.35, pan=0.5)

    chords = [
        (0.0, 7.5, [note_to_freq('D2'), note_to_freq('A2'), note_to_freq('F3')]),
        (7.5, 7.5, [note_to_freq('Bb1'), note_to_freq('F2'), note_to_freq('D3')]),
        (15.0, 7.5, [note_to_freq('G1'), note_to_freq('D2'), note_to_freq('Bb2')]),
        (22.5, 7.5, [note_to_freq('A1'), note_to_freq('E2'), note_to_freq('C#3')]),
    ]
    
    for start, dur, ch in chords:
        render_pad_chord(buf, start, ch, dur, amplitude=0.30, pan=0.5, bright=0.7)
        render_pad_chord(buf, start, [f * 2.0 for f in ch], dur, amplitude=0.18, pan=0.3, bright=0.8)

    tremolo_notes = [
        (0.0, 7.5, 'A4'), (7.5, 7.5, 'Bb4'), (15.0, 7.5, 'D5'), (22.5, 7.5, 'C#5')
    ]
    for start, dur, n_name in tremolo_notes:
        freq = note_to_freq(n_name)
        steps = int(dur / 0.25) # 8th notes for efficiency
        for s in range(steps):
            t_sub = start + s * 0.25
            render_flute_melody(buf, t_sub, freq, 0.22, amplitude=0.15, pan=0.7 if s % 2 == 0 else 0.3)

    buf.apply_reverb(delay_ms=140, feedback=0.45, wet=0.32)
    buf.export_wav(output_path)


def generate_wilderness_trail(output_path):
    duration = 36.0
    buf = StereoBuffer(duration)
    
    beat_sec = 0.25
    total_steps = int(duration / beat_sec)
    
    arpeggio_chords = [
        ['G3', 'B3', 'D4', 'G4'],
        ['D3', 'F#3', 'A3', 'D4'],
        ['E3', 'G3', 'B3', 'E4'],
        ['C3', 'E3', 'G3', 'C4']
    ]
    
    for s in range(total_steps):
        t_off = s * beat_sec
        chord_idx = (s // 16) % len(arpeggio_chords)
        chord = arpeggio_chords[chord_idx]
        note = chord[s % 4]
        freq = note_to_freq(note)
        pan = 0.25 + 0.5 * ((s % 4) / 3.0)
        render_plucked_string(buf, t_off, freq, 0.4, amplitude=0.26, pan=pan)

    pad_chords = [
        (0.0, 9.0, [note_to_freq('G2'), note_to_freq('D3'), note_to_freq('B3')]),
        (9.0, 9.0, [note_to_freq('D2'), note_to_freq('A2'), note_to_freq('F#3')]),
        (18.0, 9.0, [note_to_freq('E2'), note_to_freq('B2'), note_to_freq('G3')]),
        (27.0, 9.0, [note_to_freq('C2'), note_to_freq('G2'), note_to_freq('E3')]),
    ]
    for start, dur, ch in pad_chords:
        render_pad_chord(buf, start, ch, dur, amplitude=0.18, pan=0.5, bright=0.4)

    flute_melody = [
        (1.0, 'D5', 1.5), (2.5, 'G5', 2.0), (4.5, 'F#5', 1.5), (6.0, 'E5', 2.0),
        (9.0, 'F#5', 1.5), (10.5, 'A5', 2.0), (12.5, 'G5', 1.5), (14.0, 'D5', 2.5),
        (18.0, 'B5', 2.0), (20.0, 'A5', 1.5), (21.5, 'G5', 1.5), (23.0, 'E5', 2.5),
        (27.0, 'G5', 2.0), (29.0, 'A5', 2.0), (31.0, 'G5', 4.0)
    ]
    for t_off, n_name, dur in flute_melody:
        freq = note_to_freq(n_name)
        render_flute_melody(buf, t_off, freq, dur, amplitude=0.25, pan=0.5)

    buf.apply_reverb(delay_ms=110, feedback=0.32, wet=0.22)
    buf.export_wav(output_path)


if __name__ == '__main__':
    os.makedirs('assets/audio', exist_ok=True)
    print("Generating 3 fitting song loops for Divus Factus...")
    generate_winter_hymn('assets/audio/winter_hymn.wav')
    generate_gathering_storm('assets/audio/gathering_storm.wav')
    generate_wilderness_trail('assets/audio/wilderness_trail.wav')
    print("All 3 new song loops generated successfully!")
