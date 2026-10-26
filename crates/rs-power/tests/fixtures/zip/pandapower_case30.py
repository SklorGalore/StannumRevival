"""Reference solution for tests/zip_loads.rs: MATPOWER case30 with every load
split into constant power, constant current and constant impedance parts,
solved by pandapower (independent ZIP load implementation).

    pip install pandapower==3.5.6
    python pandapower_case30.py > case30_zip_pandapower.json
"""
import json

import pandapower as pp
import pandapower.networks as pn

net = pn.case30()
# Shares of each load's P and Q (percent); the rest is constant power.
net.load["const_i_p_percent"] = 30.0
net.load["const_z_p_percent"] = 30.0
net.load["const_i_q_percent"] = 20.0
net.load["const_z_q_percent"] = 50.0
pp.runpp(net, tolerance_mva=1e-10, init="flat", voltage_depend_loads=True)
print(json.dumps({
    "pandapower": pp.__version__,
    "vm": list(net.res_bus.vm_pu),
    "va": list(net.res_bus.va_degree),
    "slack_p_mw": float(net.res_ext_grid.p_mw.iloc[0]),
    "slack_q_mvar": float(net.res_ext_grid.q_mvar.iloc[0]),
}, indent=1))
