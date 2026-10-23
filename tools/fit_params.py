"""Tabulate fitted patches (fit.rs *.fit.txt): per-param min/median/max, and
physical units for the important ones."""
import sys, glob, os, math
rows = {}
for p in sorted(glob.glob(os.path.join(sys.argv[1], "*.fit.txt"))):
    d = {}
    for line in open(p):
        k, v = line.split()
        d[k] = float(v)
    rows[os.path.basename(p).split('.')[0]] = d
keys = list(next(iter(rows.values())).keys())
ENG = ["VA", "SYNC", "SUPER", "FM", "FB-FM", "RING", "PD", "WT-FM"]
pitch = lambda v: 16 * 2 ** (6 * v)
tune = lambda v: (v - 0.5) * 48
dec = lambda v: 0.03 * 200 ** v
cut = lambda v: 20 * 1000 ** v
fenv = lambda v: 8 * (2*(v-.5)) * abs(2*(v-.5)) ** .5
fdec = lambda v: 0.01 * 400 ** v
gate = lambda v: None if v >= .999 else 0.03 * 100 ** v
rel = lambda v: 0.003 * 1000 ** v
att = lambda v: 0.0005 * 400 ** v
print(f"{'name':<11}{'eng':<6}{'pitch':>6} {'tunes(st)/levels':<34}{'dec':>5}{'att':>6}{'sus':>5}{'gate':>6}{'rel':>6}{'cut':>6}{'res':>5}{'fenv':>6}{'fdec':>6}{'key':>5}{'ft':>3}{'cho':>5}{'dly':>5}{'rev':>5} macros")
for n, d in rows.items():
    tunes = " ".join(f"{tune(d[t]):+5.1f}/{d[l]:.2f}" for t, l in [("Tune2","Level2"),("Tune3","Level3"),("Tune4","Level4")])
    g = gate(d["Gate"])
    print(f"{n:<11}{ENG[int(d['Engine'])]:<6}{pitch(d['Pitch']):6.1f} {tunes:<34}{dec(d['Decay']):5.2f}{att(d['Attack'])*1000:6.1f}{d['Sustain']:5.2f}{(f'{g:.2f}' if g else 'HOLD'):>6}{rel(d['Release']):6.3f}{cut(d['Cutoff']):6.0f}{d['Reso']:5.2f}{fenv(d['FilterEnv']):+6.1f}{fdec(d['FilterDecay']):6.2f}{d['KeyTrack']:5.2f}{int(d['FilterType']):3d}{d['ChorusMix']:5.2f}{d['DelayMix']:5.2f}{d['ReverbMix']:5.2f} {d['MacroX']:.2f} {d['MacroY']:.2f} {d['MacroZ']:.2f}")
print()
for k in keys:
    vals = sorted(r[k] for r in rows.values())
    print(f"{k:<16} min {vals[0]:.3f}  p25 {vals[len(vals)//4]:.3f}  med {vals[len(vals)//2]:.3f}  p75 {vals[3*len(vals)//4]:.3f}  max {vals[-1]:.3f}")
