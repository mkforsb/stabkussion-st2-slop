"""Side-by-side spectrograms of reference vs fit from fit.rs's NAME.fit.wav
(reference, 0.3 s gap, fit)."""
import sys, glob, os
import numpy as np
from scipy.io import wavfile
from scipy.signal import stft
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt

d, out = sys.argv[1], sys.argv[2]
names = sys.argv[3:]
files = sorted(glob.glob(os.path.join(d, "*.fit.wav")))
if names:
    files = [f for f in files if any(n in f for n in names)]
fig, axes = plt.subplots(len(files), 1, figsize=(10, 2.2 * len(files)), squeeze=False)
for ax, path in zip(axes[:, 0], files):
    fs, x = wavfile.read(path)
    m = x.astype(np.float64).mean(axis=1) / 32768
    f, t, Z = stft(m, fs, nperseg=2048, noverlap=2048 - 120)
    S = 20 * np.log10(np.abs(Z) + 1e-9)
    S -= S.max()
    ax.pcolormesh(t, f, S, vmin=-60, vmax=0, shading="auto", cmap="magma")
    ax.set_yscale("log"); ax.set_ylim(25, 3000)
    ax.set_title(os.path.basename(path), fontsize=8); ax.tick_params(labelsize=6)
fig.tight_layout(); fig.savefig(out, dpi=60)
