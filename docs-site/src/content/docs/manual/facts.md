---
title: FACTS devices
description: STATCOMs and UPFCs from PSS/E FACTS control device data, holding a bus voltage within a current limit and sending a set power along a series link.
---

RAW files describe FACTS devices in their **FACTS control device data**.
Every device has a **shunt element** at its sending-end bus `I` that holds a
voltage, like a STATCOM. A device with a terminal-end bus `J` also has a
**series element** that sends a set power to `J`, like a unified power flow
controller (UPFC). Stannum solves both in AC solves; the series element also
moves its active power in DC solves. MATPOWER skips FACTS data.

## What is read

| Field | Use |
| --- | --- |
| `NAME` (`N` before revision 33) | The device's name, also what a MODSW 6 switched shunt's `RMIDNT` names |
| `I`, `J` | Sending-end bus; terminal-end bus, or 0 for a shunt-only device |
| `MODE` | 0 out of service (left out); 1 normal; 2 series link bypassed (only the shunt element is kept, with a warning). Other modes are left out with a warning. |
| `PDES`, `QDES` | Power the series element delivers to `J`, in MW and Mvar |
| `VSET` | Voltage the shunt element holds, in per unit |
| `SHMX` | Largest shunt current, as MVA at 1 p.u. voltage |
| `RMPCT` | Share of the reactive power holding a remote bus |
| `REMOT` (`FCREG` in revision 35) | The bus the shunt element holds instead of `I`, from revision 31 |

Not used, with a warning on each series device: the series element's voltage
and current limits (`VSMX`, `IMX`, `TRMX`, `VTMN`, `VTMX`) and its reactance
`LINX`.

## Shunt element

The shunt element is solved as a reactive source: a helper generator at `I`
that holds `VSET` at its bus (or at `REMOT`, when [remote
regulation](/StannumRevival/manual/remote-regulation/) is on with reactive
limits), within

$$
-\,\text{SHMX}\cdot V_\text{SET} \;\le\; Q \;\le\; \text{SHMX}\cdot V_\text{SET}\ \text{Mvar}.
$$

Its limits are always enforced, even when generator Q limits are not. At a
limit it stops holding the voltage and keeps that output, like a generator,
so the bus voltage then moves freely (on the PSS/E sample case both devices at
bus 153 sit at their limits with the bus at 1.057 p.u., above their 1.015
setpoint). PSS/E's limit is a current, so its output at the limit follows the
voltage; Stannum keeps it at the value for `VSET`, a difference of a few
percent when the voltage is that far off.

A shunt element at a bus where a generator already holds the voltage, or at an
isolated bus, is **held** at zero.

When a solve starts from the last solution, each shunt element starts at the
output it ended with, and one that ended at a limit starts there, like a
generator.

## Series element

In normal mode the series element delivers `PDES` + j`QDES` to `J` and takes
the same from `I`. The shunt element supplies the real power the series
element exchanges, so the device as a whole is lossless. This is a fixed
transfer: the series element's voltage and current limits are not checked,
and the reactive power drawn at `I` is taken as `QDES` (it differs from it by
the voltage ratio across the link).

## Results

The **FACTS** tab in Power flow lists each device with its shunt output, its
range, whether it holds its voltage, sits at a limit or is held, and its series
transfer. The solved case keeps the network as given: the shunt outputs are
not added into bus shunts, and the series transfers are not in the loads.

## Switched shunts that watch a FACTS device

A switched shunt in mode 6 (`MODSW` = 6) switches whole blocks to keep the
shunt element of the FACTS device named in `RMIDNT` within its band, in
fractions of the shunt element's range, as in the [other reactive
modes](/StannumRevival/manual/voltage-controls/#shunts-that-watch-another-device).
`SWREM`, when given, is the device's sending-end bus. A record naming a device
that is not in the case or not in service is frozen at `BINIT` with a warning;
one whose device is held is held too.

## Check against PSS/E

The PSS/E sample case (`t_psse_case3`, revision 30) has a STATCOM and a UPFC
at bus 153, the UPFC sending 350 MW and 40 Mvar to bus 155. Against the
solution saved in the file, with reactive limits and the detailed DC models:

| | Mean \|ΔV\| (p.u.) | Largest \|ΔV\| (p.u.) | Largest \|Δθ\| |
| --- | --- | --- | --- |
| FACTS left out (as MATPOWER) | 0.0025 | 0.024 | 10.8° |
| FACTS modelled | 0.0012 | 0.015 | 0.94° |

Bus 155 solves at 1.0174 p.u. against 1.0172 in the file. The remaining
differences are around the VSC converter at bus 3005, not the FACTS devices.
