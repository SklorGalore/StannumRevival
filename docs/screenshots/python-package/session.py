# Runs a short session in the shape of a notebook and writes it as HTML.
import contextlib, io, html, sys, time, threading, _thread, base64
import numpy as np
import stannum as sn
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt

CASE = "/tmp/claude-0/synth/synthetic_2000.m"
cells = [
'''import stannum as sn
sn.__version__, sn.API_VERSION''',
'''case = sn.open("synthetic_2000.m")
report = case.solve(start="flat")
report.converged, report.iterations, round(report.total_losses_mw, 1)''',
'''buses = report.buses
buses''',
'''low = buses[buses["vm"] < 0.97]
len(low), low["id"][:5], low["vm"][:5]''',
'''worst = report.branches[np.argsort(-np.nan_to_num(report.branches["loading_pct"]))][:3]
worst.to_pandas()[["element", "pf", "loading_pct"]]''',
'''run = case.contingencies()          # screen every outage, then AC on the worst
run.total, run.ac_checked, run.with_overloads, run.rows''',
'''top = run.rows[run.rows["cut_off_buses"] == 0][0]
case.edits = [sn.outage(top["element"])]
after = case.solve()
top["element"], after.limits.branch_overloads, round(after.total_losses_mw - report.total_losses_mw, 2)''',
'''try:
    case.element("bus:999999")
except sn.NotFound as e:
    print(type(e).__name__, e.kind, "-", e)''',
'''case.edits = []
case.solve()
threading.Timer(0.3, _thread.interrupt_main).start()   # stands in for Ctrl+C
t = time.time()
try:
    case.contingencies(method="ac")
except KeyboardInterrupt:
    print(f"stopped after {time.time() - t:.2f} s; case still solves:", case.solve().converged)''',
]
ns = {"sn": sn, "np": np, "threading": threading, "_thread": _thread, "time": time}
import os; os.chdir("/tmp/claude-0/synth")
out_html = []
for i, src in enumerate(cells, 1):
    buf = io.StringIO()
    with contextlib.redirect_stdout(buf):
        lines = src.rstrip().split("\n")
        try:
            last = compile(lines[-1], "<cell>", "eval")
            exec("\n".join(lines[:-1]), ns)
            val = eval(last, ns)
            if val is not None:
                print(repr(val) if not hasattr(val, "to_string") else val.to_string())
        except SyntaxError:
            exec(src, ns)
    out_html.append(f'<div class="cell"><div class="in">In [{i}]:</div><pre class="code">{html.escape(src)}</pre></div>'
                    f'<div class="cell"><div class="out">Out[{i}]:</div><pre class="res">{html.escape(buf.getvalue().rstrip())}</pre></div>')
# plot
b = ns["report"].buses
fig, ax = plt.subplots(figsize=(8, 2.6), dpi=110)
order = np.argsort(b["vm"])
ax.plot(np.arange(len(b)), b["vm"][order], color="#2a7f7a")
ax.axhline(0.95, color="#b04a3a", lw=0.8, ls="--"); ax.axhline(1.05, color="#b04a3a", lw=0.8, ls="--")
ax.set_xlabel("buses, sorted by voltage"); ax.set_ylabel("vm (p.u.)"); ax.set_title("synthetic_2000: report.buses['vm']", fontsize=10)
fig.tight_layout(); pb = io.BytesIO(); fig.savefig(pb, format="png")
img = base64.b64encode(pb.getvalue()).decode()
plot_src = '''import matplotlib.pyplot as plt
plt.plot(np.sort(report.buses["vm"]))'''
out_html.append(f'<div class="cell"><div class="in">In [{len(cells)+1}]:</div><pre class="code">{html.escape(plot_src)}</pre></div><div class="cell"><div class="out"></div><img src="data:image/png;base64,{img}"></div>')
page = f'''<!doctype html><meta charset="utf-8"><style>
body{{font:13px/1.45 ui-monospace,Menlo,Consolas,monospace;background:#fff;color:#1d2125;margin:16px;width:980px}}
h1{{font:600 15px system-ui;margin:0 0 10px}}
.cell{{display:flex;gap:8px;margin:2px 0}} .in,.out{{width:64px;flex:none;text-align:right;color:#2a5fa8;padding-top:6px}} .out{{color:#b0452c}}
pre{{margin:0;padding:6px 8px;flex:1;white-space:pre-wrap}} .code{{background:#f4f6f8;border:1px solid #dde2e6;border-radius:4px}}
img{{max-width:880px}}</style><h1>stannum 0.3.0 in Python {sys.version.split()[0]} (abi3 wheel built from crates/stannum-py)</h1>{"".join(out_html)}'''
open("/tmp/claude-0/shots/session.html", "w").write(page)
print("ok")
