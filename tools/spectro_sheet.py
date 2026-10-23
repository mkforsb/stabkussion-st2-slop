"""Contact sheet of log-frequency spectrograms + envelopes for every example."""
import sys, glob, os
import numpy as np
from scipy.io import wavfile
from scipy.signal import stft
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt

files = sorted(glob.glob(os.path.join(sys.argv[1], "*.wav")))
out = sys.argv[2]
cols = 4
rows = (len(files) + cols - 1) // cols
fig, axes = plt.subplots(rows, cols, figsize=(cols * 5, rows * 2.6))
for ax, path in zip(axes.flat, files):
    fs, x = wavfile.read(path)
    x = x.astype(np.float64) / 32768.0
    m = x.mean(axis=1) if x.ndim > 1 else x
    f, t, Z = stft(m, fs, nperseg=4096, noverlap=4096 - 240)
    S = 20 * np.log10(np.abs(Z) + 1e-9)
    S -= S.max()
    ax.pcolormesh(t, f, S, vmin=-60, vmax=0, shading="auto", cmap="magma")
    ax.set_yscale("log")
    ax.set_ylim(25, 3000)
    ax.set_xlim(0, 1.2)
    ax.set_title(os.path.basename(path), fontsize=9)
    ax.tick_params(labelsize=7)
for ax in axes.flat[len(files):]:
    ax.axis("off")
fig.tight_layout()
fig.savefig(out, dpi=70)
