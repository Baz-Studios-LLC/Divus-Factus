#!/usr/bin/env python3
"""Render Made By Belief to WAV files with a tiny dependency-free synth."""

from __future__ import annotations

import math
import random
import wave
from pathlib import Path

SAMPLE_RATE = 44_100
BPM = 78
BEAT = 60.0 / BPM
BAR = BEAT * 4
ROOT = Path(__file__).resolve().parent


def hz(name: str) -> float:
    names = {
        "C": 0,
        "C#": 1,
        "D": 2,
        "Eb": 3,
        "E": 4,
        "F": 5,
        "F#": 6,
        "G": 7,
        "Ab": 8,
        "A": 9,
        "Bb": 10,
        "B": 11,
    }
    note = name[:-1]
    octave = int(name[-1])
    midi = 12 * (octave + 1) + names[note]
    return 440.0 * (2.0 ** ((midi - 69) / 12.0))


def soft_clip(x: float) -> float:
    return math.tanh(x * 1.15) * 0.88


def env(t: float, dur: float, attack: float, release: float) -> float:
    if t < 0.0 or t > dur:
        return 0.0
    a = min(t / max(attack, 0.001), 1.0)
    r = min((dur - t) / max(release, 0.001), 1.0)
    return min(a, r, 1.0)


def add_tone(
    left: list[float],
    right: list[float],
    start: float,
    dur: float,
    freq: float,
    amp: float,
    pan: float,
    kind: str,
) -> None:
    start_i = max(0, int(start * SAMPLE_RATE))
    end_i = min(len(left), int((start + dur) * SAMPLE_RATE))
    phase_offset = random.Random(int(freq * 1000 + start_i)).random() * math.tau
    for i in range(start_i, end_i):
        t = i / SAMPLE_RATE - start
        if kind == "melody":
            e = env(t, dur, 0.05, 0.22)
            vibrato = math.sin((t * 5.2 + 0.3) * math.tau) * 0.006
            f = freq * (1.0 + vibrato)
            x = (
                math.sin(math.tau * f * t + phase_offset)
                + 0.35 * math.sin(math.tau * f * 2.0 * t + phase_offset * 0.4)
                + 0.12 * math.sin(math.tau * f * 3.0 * t)
            ) * e
        elif kind == "drone":
            e = env(t, dur, 0.7, 0.7)
            slow = 0.9 + 0.1 * math.sin(math.tau * 0.13 * (start + t))
            x = (
                math.sin(math.tau * freq * t + phase_offset)
                + 0.45 * math.sin(math.tau * freq * 0.5 * t)
                + 0.18 * math.sin(math.tau * freq * 1.5 * t)
            ) * e * slow
        elif kind == "bell":
            e = math.exp(-t * 1.75) * env(t, dur, 0.003, 0.08)
            x = (
                math.sin(math.tau * freq * t)
                + 0.55 * math.sin(math.tau * freq * 2.01 * t)
                + 0.2 * math.sin(math.tau * freq * 3.97 * t)
            ) * e
        else:
            e = math.exp(-t * 11.0)
            noise = random.Random(i + start_i).uniform(-1.0, 1.0) * 0.18
            x = (math.sin(math.tau * freq * t) + noise) * e
        l = amp * x * math.cos(pan * math.pi / 2.0)
        r = amp * x * math.sin(pan * math.pi / 2.0)
        left[i] += l
        right[i] += r


def melody_events() -> list[tuple[float, float, str, float]]:
    bars = [
        ["D4", "F4", "E4", "D4"],
        ["A3", "D4", "F4", "E4"],
        ["G4", "F4", "E4", "C4"],
        ["D4", None, "A3", None],
        ["D4", "F4", "A4", "G4"],
        ["F4", "E4", "D4", "C4"],
        ["A3", "C4", "D4", "F4"],
        ["E4", None, "D4", None],
        ["F4", "A4", "G4", "F4"],
        ["E4", "D4", "C4", "A3"],
        ["C4", "D4", "F4", "E4"],
        ["D4", None, "A3", None],
        ["G4", "F4", "E4", "D4"],
        ["A3", "D4", "C4", "A3"],
        ["D4", "F4", "E4", "C4"],
        ["D4", None, None, None],
    ]
    out = []
    for b, notes in enumerate(bars):
        for beat, note in enumerate(notes):
            if note is None:
                continue
            dur = BEAT
            if b in {3, 7, 11} and beat == 0:
                dur = BEAT * 2
            if b == 15 and beat == 0:
                dur = BEAT * 3.9
            out.append((b * BAR + beat * BEAT, dur, note, 0.21))
    return out


def drone_events() -> list[tuple[float, float, str, float]]:
    chords = [
        ("D2", "A1"),
        ("D2", "A1"),
        ("C2", "G1"),
        ("D2", "A1"),
        ("D2", "A1"),
        ("C2", "G1"),
        ("A1", "C2"),
        ("D2", "A1"),
        ("D2", "A1"),
        ("C2", "G1"),
        ("A1", "C2"),
        ("D2", "A1"),
        ("G1", "D2"),
        ("A1", "C2"),
        ("D2", "A1"),
        ("D2", "A1"),
    ]
    out = []
    for b, chord in enumerate(chords):
        for note in chord:
            out.append((b * BAR, BAR, note, 0.11))
    return out


def bell_events(prayer: bool = False) -> list[tuple[float, float, str, float]]:
    bars = [0, 8, 15] if not prayer else [0, 2, 4, 6]
    out = []
    for b in bars:
        out.append((b * BAR, BEAT * 2.4, "D5", 0.09))
        out.append((b * BAR + BEAT * 0.5, BEAT * 1.6, "A4", 0.06))
    return out


def pulse_events(bars: int = 16) -> list[tuple[float, float, str, float]]:
    out = []
    for b in range(bars):
        out.append((b * BAR, BEAT * 0.35, "A1", 0.17))
        out.append((b * BAR + BEAT * 2, BEAT * 0.35, "D2", 0.12))
    return out


def render(path: Path, variant: str) -> None:
    bars = 16 if variant == "full" else 8
    seconds = bars * BAR
    samples = int(seconds * SAMPLE_RATE)
    left = [0.0] * samples
    right = [0.0] * samples

    if variant == "full":
        for start, dur, note, amp in drone_events():
            add_tone(left, right, start, dur, hz(note), amp, 0.38, "drone")
        for start, dur, note, amp in pulse_events(16):
            add_tone(left, right, start, dur, hz(note), amp, 0.5, "pulse")
        for start, dur, note, amp in melody_events():
            add_tone(left, right, start, dur, hz(note), amp, 0.62, "melody")
        for start, dur, note, amp in bell_events(False):
            add_tone(left, right, start, dur, hz(note), amp, 0.55, "bell")
    else:
        for start, dur, note, amp in drone_events()[8:16]:
            add_tone(left, right, start - 8 * BAR, dur, hz(note), amp * 0.9, 0.42, "drone")
        for start, dur, note, amp in bell_events(True):
            add_tone(left, right, start, dur, hz(note), amp * 1.1, 0.56, "bell")
        for start, dur, note, amp in pulse_events(8):
            add_tone(left, right, start, dur, hz(note), amp * 0.35, 0.5, "pulse")

    peak = max(max(abs(x) for x in left), max(abs(x) for x in right), 1e-6)
    gain = 0.82 / peak
    with wave.open(str(path), "wb") as wav:
        wav.setnchannels(2)
        wav.setsampwidth(2)
        wav.setframerate(SAMPLE_RATE)
        frames = bytearray()
        for l, r in zip(left, right):
            li = int(max(-1.0, min(1.0, soft_clip(l * gain))) * 32767)
            ri = int(max(-1.0, min(1.0, soft_clip(r * gain))) * 32767)
            frames += li.to_bytes(2, "little", signed=True)
            frames += ri.to_bytes(2, "little", signed=True)
        wav.writeframes(frames)


def main() -> None:
    render(ROOT / "made_by_belief.wav", "full")
    render(ROOT / "made_by_belief_prayer_board.wav", "prayer")
    print(ROOT / "made_by_belief.wav")
    print(ROOT / "made_by_belief_prayer_board.wav")


if __name__ == "__main__":
    main()

