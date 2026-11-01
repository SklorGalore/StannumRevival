"""Reference QV curves for the QV curve tests, from an independent solver.

For each study, a MATPOWER case is read and solved tightly with PYPOWER.
Then a generator with no active power is added at the study bus, the bus
becomes a voltage-controlled bus held at one voltage after another, and
PYPOWER solves each one, starting from the last. The added generator's
reactive output at each voltage is the curve. Generator reactive limits
are off. This matches Stannum's QV curve with Q limits off, at a load bus
(no generator of its own and no voltage controls in a MATPOWER case).

Run from the repository root (needs numpy, scipy and pypower):

    python crates/stannum-core/scripts/qv_reference.py

It writes crates/stannum-core/tests/fixtures/qv_reference.json.
"""

import copy
import json
import re
from pathlib import Path

import numpy as np
from pypower.api import ppoption, runpf
from pypower.idx_bus import BUS_I, BUS_TYPE, VM, VA
from pypower.idx_gen import GEN_BUS, PG, QG, QMAX, QMIN, VG, GEN_STATUS, PMAX, PMIN

ROOT = Path(__file__).resolve().parents[3]
FIXTURES = ROOT / "crates/rs-power/tests/fixtures"
OUT = ROOT / "crates/stannum-core/tests/fixtures/qv_reference.json"

STUDIES = [
    {"case": "case9.m", "name": "case9, bus 5", "bus": 5},
    {"case": "case30.m", "name": "case30, bus 30", "bus": 30},
    {"case": "case118.m", "name": "case118, bus 44", "bus": 44},
]

# Voltages on a 0.01 p.u. grid, from 1.10 p.u. down to 0.30 p.u.
HIGH, LOW, STEP = 110, 30, 1

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
    row = int(np.where(ppc["bus"][:, BUS_I] == spec["bus"])[0][0])
    assert ppc["bus"][row, BUS_TYPE] == 1, "the study bus must be a load bus"
    assert spec["bus"] not in ppc["gen"][:, GEN_BUS]
    base_vm = float(r0["bus"][row, VM])

    held = copy.deepcopy(ppc)
    held["bus"][:, VM] = r0["bus"][:, VM]
    held["bus"][:, VA] = r0["bus"][:, VA]
    held["gen"][:, QG] = r0["gen"][:, QG]
    held["bus"][row, BUS_TYPE] = 2
    extra = np.zeros(held["gen"].shape[1])
    extra[GEN_BUS] = spec["bus"]
    extra[QMAX] = 1e6
    extra[QMIN] = -1e6
    extra[VG] = base_vm
    extra[GEN_STATUS] = 1
    extra[PMAX] = extra[PMIN] = extra[PG] = 0.0
    if held["gen"].shape[1] > 6:
        extra[6] = ppc["baseMVA"]
    held["gen"] = np.vstack([held["gen"], extra])

    def solve(start, v):
        case = copy.deepcopy(start)
        case["gen"][-1, VG] = v
        case["bus"][row, VM] = v
        r, ok = runpf(case, TIGHT)
        if not ok:
            return None
        nxt = copy.deepcopy(held)
        nxt["bus"][:, VM] = r["bus"][:, VM]
        nxt["bus"][:, VA] = r["bus"][:, VA]
        return nxt, float(r["gen"][-1, QG])

    first = solve(held, base_vm)
    assert first is not None
    points = {}
    start = first[0]
    for k in range(int(np.floor(base_vm * 100 + 0.25)) + 1, HIGH + 1, STEP):
        got = solve(start, k / 100)
        if got is None:
            break
        start, points[k] = got[0], got[1]
    start = first[0]
    for k in range(int(np.ceil(base_vm * 100 - 0.25)) - 1, LOW - 1, -STEP):
        got = solve(start, k / 100)
        if got is None:
            break
        start, points[k] = got[0], got[1]
    ks = sorted(points)
    q = [points[k] for k in ks]
    i = int(np.argmin(q))
    return {"case": spec["case"], "name": spec["name"], "bus": spec["bus"],
            "base_vm_pu": base_vm, "q_at_base_mvar": first[1],
            "points": [{"v_pu": k / 100, "q_mvar": points[k]} for k in ks],
            "q_min_mvar": q[i], "v_at_q_min_pu": ks[i] / 100}


def main():
    out = {"source": "crates/stannum-core/scripts/qv_reference.py",
           "studies": [study(s) for s in STUDIES]}
    OUT.write_text(json.dumps(out, indent=2) + "\n")
    for s in out["studies"]:
        print(f"{s['name']}: bottom {s['q_min_mvar']:.3f} Mvar at {s['v_at_q_min_pu']:.2f} p.u., "
              f"{len(s['points'])} points from {s['points'][0]['v_pu']} p.u.")


if __name__ == "__main__":
    main()
