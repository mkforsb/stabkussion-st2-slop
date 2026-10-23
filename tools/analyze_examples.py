"""Survey the example stabs: length, envelope, stereo, spectrum, partials."""
import sys, glob, os
import numpy as np
from scipy.io import wavfile
from scipy.signal import find_peaks, stft

def db(x):
    return 20 * np.log10(np.maximum(x, 1e-9))

def partials(x, fs, t0, t1, n=8):
    seg = x[int(t0 * fs):int(t1 * fs)]
    if len(seg) < 256:
        return []
    nfft = 1 << 17
    w = np.blackman(len(seg))
    S = np.abs(np.fft.rfft(seg * w, nfft))
    f = np.fft.rfftfreq(nfft, 1 / fs)
    band = (f > 20) & (f < 8000)
    Sd = db(S / S[band].max())
    pk, _ = find_peaks(np.where(band, Sd, -200), height=-40, distance=int(8 / (fs / nfft)), prominence=6)
    pk = sorted(pk, key=lambda i: -Sd[i])[:n]
    return sorted([(f[i], Sd[i]) for i in pk])

def main():
    files = sorted(glob.glob(os.path.join(sys.argv[1], "*.wav")))
    for path in files:
        fs, x = wavfile.read(path)
        x = x.astype(np.float64) / 32768.0
        if x.ndim == 1:
            x = x[:, None]
        L, R = x[:, 0], x[:, -1]
        M, S = (L + R) / 2, (L - R) / 2
        dur = len(M) / fs
        peak = np.abs(x).max()
        width = np.sqrt(np.mean(S**2) / max(np.mean(M**2), 1e-12))
        # Envelope in 5 ms windows.
        hop = int(0.005 * fs)
        env = np.array([np.sqrt(np.mean(M[i:i + hop] ** 2)) for i in range(0, len(M) - hop, hop)])
        edb = db(env / env.max())
        ipk = int(np.argmax(env))
        attack_ms = ipk * 5
        def t_below(th):
            idx = np.where(edb[ipk:] < th)[0]
            return (ipk + idx[0]) * 5 if len(idx) else None
        t20, t40 = t_below(-20), t_below(-40)
        # Spectrum: band energies and centroid over the whole hit.
        F = np.abs(np.fft.rfft(M)) ** 2
        f = np.fft.rfftfreq(len(M), 1 / fs)
        tot = F[f > 20].sum()
        bands = [(20, 100), (100, 300), (300, 1000), (1000, 4000), (4000, 20000)]
        be = [100 * F[(f >= a) & (f < b)].sum() / tot for a, b in bands]
        cen = (F * f)[f > 20].sum() / tot
        # Early vs late centroid (brightness envelope / filter env).
        def centroid(t0, t1):
            seg = M[int(t0 * fs):int(t1 * fs)]
            if len(seg) < 64:
                return float('nan')
            G = np.abs(np.fft.rfft(seg * np.hanning(len(seg)))) ** 2
            g = np.fft.rfftfreq(len(seg), 1 / fs)
            return (G * g).sum() / max(G.sum(), 1e-12)
        pk_t = ipk * 0.005
        c_early = centroid(pk_t, pk_t + 0.04)
        c_mid = centroid(pk_t + 0.1, pk_t + 0.2)
        name = os.path.basename(path)
        print(f"== {name}: {dur:.2f}s peak {db(peak):.1f}dBFS width(S/M) {width:.2f} "
              f"attack {attack_ms}ms -20dB@{t20}ms -40dB@{t40}ms")
        print(f"   bands% <100:{be[0]:.0f} 100-300:{be[1]:.0f} 300-1k:{be[2]:.0f} 1-4k:{be[3]:.0f} >4k:{be[4]:.0f}"
              f"  centroid {cen:.0f}Hz  early {c_early:.0f} → +100ms {c_mid:.0f}")
        for (t0, t1, lab) in [(pk_t, pk_t + 0.06, "early"), (pk_t + 0.08, pk_t + 0.3, "body ")]:
            ps = partials(M, fs, t0, t1)
            if ps:
                lo = ps[0][0]
                txt = "  ".join(f"{fr:.0f}({lv:.0f})" for fr, lv in ps)
                rat = " ".join(f"{fr / lo:.2f}" for fr, _ in ps)
                print(f"   {lab} partials Hz(dB): {txt}")
                print(f"         ratios to lowest: {rat}")

main()
