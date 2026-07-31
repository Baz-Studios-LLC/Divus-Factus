## The village finds its tongue

**A new engine under the villagers' words.** The language model now runs on llama.cpp, statically linked into the game — same model, same install, nothing new to do, but a line that took fifteen seconds to compose now takes under one. And it runs entirely on CPU cores the game barely uses, which ends the little hitch that used to precede every bubble: the GPU belongs to the renderer alone, always.

**Whole sentences.** Villagers now speak in complete, plain sentences instead of clipped fragments — "i hope the new field will help, but I still don't know what I should believe" — and their talk picks up whatever has lately happened in the village: weddings, wolves, ground broken, lightning.

**Every word on screen is their own.** The written stock lines are retired from the bubbles entirely — at every distance, a villager either speaks words composed for them in that moment, or holds their peace. And every bubble now reads as a proper sentence: a capital to open, a stop to close.

**Green means their own words.** Composed thoughts wear the green border too, not just composed speech — which now means every bubble should wear it.

## The codex learns Settings

A new page behind the sliders tab: see which model the villagers borrow, and switch between any models in your folder with a click — the change takes hold in seconds and is remembered. Hotkeys, video and sound will live here as they arrive.

## Small things

- The build number sits quietly in the title screen's corner.
- A dev overlay on the backquote key shows the frame rate; saturation tuning moved to F12 to make room for it.
