"""Reference PV curves for the PV curve tests, from an independent solver.

For each study, a MATPOWER case is read, solved tightly with PYPOWER, and
traced with a pseudo-arclength continuation power flow written here from
the textbook method (Ajjarapu and Christy 1992): predictor along the tangent,
corrector on the power flow equations plus the arclength equation. Unlike
Stannum's stepped solves, it goes round the nose, so its largest transfer is
the nose itself. It also records the sink bus voltages from plain PYPOWER
solves at a few transfers.

The transfer: the sink's loads grow at constant power factor, all scaled by
one factor, and every online generator picks up in proportion to its solved
output (the reference bus takes the rest). Generator reactive limits are
off. This matches Stannum's PV curve with sink scaling "load", source
"generation" over every bus, generation limits off and Q limits off.

Run from the repository root (needs numpy, scipy and pypower):

    python crates/stannum-core/scripts/pv_reference.py

It writes crates/stannum-core/tests/fixtures/pv_reference.json.
"""

import copy
import json
import re
from pathlib import Path

import numpy as np
from pypower.api import ppoption, runpf
from pypower.dSbus_dV import dSbus_dV
from pypower.ext2int import ext2int
from pypower.idx_bus import BUS_AREA, BUS_I, BUS_TYPE, PD, QD, VM, VA
from pypower.idx_gen import GEN_BUS, GEN_STATUS, PG
from pypower.makeYbus import makeYbus

ROOT = Path(__file__).resolve().parents[3]
FIXTURES = ROOT / "crates/rs-power/tests/fixtures"
OUT = ROOT / "crates/stannum-core/tests/fixtures/pv_reference.json"

STUDIES = [
    {"case": "case9.m", "name": "case9, load at bus 5", "sink_buses": [5],
     "at": [50, 100, 150, 200, 250, 290]},
    {"case": "case30.m", "name": "case30, area 3 load", "sink_areas": [3],
     "at": [20, 40, 60, 80]},
    {"case": "case118.m", "name": "case118, load at bus 44", "sink_buses": [44],
     "at": [25, 50, 75, 100]},
]

TIGHT = ppoption(VERBOSE=0, OUT_ALL=0, PF_TOL=1e-12, PF_MAX_IT=50, ENFORCE_Q_LIMS=0)


def matrix(text, name):
    body = re.search(r"mpc\." + name + r"\s*=\s*\[(.*?)\];", text, re.S).group(1)
    rows = []
    for line in body.splitlines():
        line = line.split("%")[0].strip().rstrip(";")
        if line:
            rows.append([float(v) for v in line.replace(";", " ").split()])
    return np.array(rows)


def load_case(path):
    text = path.read_text()
    base = float(re.search(r"mpc\.baseMVA\s*=\s*([\d.]+)", text).group(1))
    return {"version": "2", "baseMVA": base, "bus": matrix(text, "bus"),
            "gen": matrix(text, "gen"), "branch": matrix(text, "branch")}


def study(spec):
    ppc = load_case(FIXTURES / spec["case"])
    r0, ok = runpf(copy.deepcopy(ppc), TIGHT)
    assert ok
    bus = ppc["bus"]
    sink = np.array([
        (int(b[BUS_I]) in spec.get("sink_buses", [])
         or int(b[BUS_AREA]) in spec.get("sink_areas", [])) and b[PD] > 0
        for b in bus
    ])
    sink_mw = bus[sink, PD].sum()
    online = r0["gen"][:, GEN_STATUS] > 0
    w = np.where(online, np.maximum(r0["gen"][:, PG], 0.0), 0.0)
    w = w / w.sum()

    def moved(lam, start):
        c = copy.deepcopy(start)
        k = 1 + lam / sink_mw
        c["bus"][:, PD] = np.where(sink, bus[:, PD] * k, bus[:, PD])
        c["bus"][:, QD] = np.where(sink, bus[:, QD] * k, bus[:, QD])
        c["gen"][:, PG] = r0["gen"][:, PG] + w * lam
        return c

    # Sink voltages from plain solves, stepping up from the base.
    sink_ids = [int(b[BUS_I]) for b, s in zip(bus, sink) if s]
    voltages = []
    prev = r0
    for lam in spec["at"]:
        r, ok = runpf(moved(lam, prev), TIGHT)
        assert ok, (spec["name"], lam)
        prev = r
        vm = {int(b[BUS_I]): b[VM] for b in r["bus"]}
        voltages.append({"transfer_mw": lam,
                         "vm_pu": {str(i): vm[i] for i in sink_ids}})

    # Arclength continuation in per unit of transfer (100 MW).
    ri = ext2int(copy.deepcopy(r0))
    base = ri["baseMVA"]
    b, g = ri["bus"], ri["gen"]
    Y = makeYbus(base, b, ri["branch"])[0].toarray()
    n = len(b)
    kinds = b[:, BUS_TYPE].astype(int)
    pv = [i for i in range(n) if kinds[i] == 2]
    pq = [i for i in range(n) if kinds[i] == 1]
    # A PV bus with no online generator is PQ, as PYPOWER treats it.
    gen_buses = {int(x[GEN_BUS]) for x in g if x[GEN_STATUS] > 0}
    pq += [i for i in pv if i not in gen_buses]
    pv = [i for i in pv if i in gen_buses]
    pq.sort()
    a = pv + pq
    # ext2int keeps the bus rows in order for these cases (buses 1..n).
    assert (ri["bus"][:, BUS_I] == np.arange(n)).all()
    sink_int = sink
    Sd0 = (b[:, PD] + 1j * b[:, QD]) / base
    Sg0 = np.zeros(n, complex)
    dS = np.zeros(n, complex)
    for k, x in enumerate(g):
        if x[GEN_STATUS] > 0:
            Sg0[int(x[GEN_BUS])] += x[PG] / base
            dS[int(x[GEN_BUS])] += w[k]
    dS[sink_int] -= Sd0[sink_int] * base / sink_mw
    Vm0 = b[:, VM].copy()
    Va0 = np.deg2rad(b[:, VA])
    na, nq = len(a), len(pq)

    def volts(x):
        va, vm = Va0.copy(), Vm0.copy()
        va[a] = x[:na]
        vm[pq] = x[na:]
        return vm * np.exp(1j * va)

    def F(x, lam):
        V = volts(x)
        mis = V * np.conj(Y @ V) - (Sg0 - Sd0 + lam * dS)
        return np.r_[mis[a].real, mis[pq].imag]

    def J(x):
        dVm, dVa = (np.asarray(m) for m in dSbus_dV(Y, volts(x)))
        return np.block([[dVa[np.ix_(a, a)].real, dVm[np.ix_(a, pq)].real],
                         [dVa[np.ix_(pq, a)].imag, dVm[np.ix_(pq, pq)].imag]])

    dFdl = -np.r_[dS[a].real, dS[pq].imag]
    x = np.r_[Va0[a], Vm0[pq]]
    lam = 0.0
    tangent = None
    nose = 0.0
    m = na + nq
    for _ in range(100000):
        last = np.r_[np.zeros(m), 1.0] if tangent is None else tangent
        A = np.vstack([np.c_[J(x), dFdl], last])
        t = np.linalg.solve(A, np.r_[np.zeros(m), 1.0])
        t /= np.linalg.norm(t)
        if tangent is not None and t @ tangent < 0:
            t = -t
        h = 0.02 if abs(t[-1]) > 0.3 else 0.0002
        z0 = np.r_[x, lam]
        y = z0 + h * t
        for _ in range(30):
            gap = np.r_[F(y[:m], y[m]), t @ (y - z0) - h]
            if np.abs(gap).max() < 1e-11:
                break
            y -= np.linalg.solve(np.vstack([np.c_[J(y[:m]), dFdl], t]), gap)
        else:
            raise RuntimeError(f"{spec['name']}: corrector failed at {lam}")
        x, lam, tangent = y[:m], y[m], t
        nose = max(nose, lam)
        if lam < nose - 0.05:
            break
    return {"name": spec["name"], "case": spec["case"],
            "sink_buses": spec.get("sink_buses", []),
            "sink_areas": spec.get("sink_areas", []),
            "nose_mw": nose * 100.0, "voltages": voltages}


def main():
    out = {"source": "crates/stannum-core/scripts/pv_reference.py",
           "studies": [study(s) for s in STUDIES]}
    OUT.write_text(json.dumps(out, indent=2) + "\n")
    for s in out["studies"]:
        print(f"{s['name']}: nose {s['nose_mw']:.3f} MW")


if __name__ == "__main__":
    main()
