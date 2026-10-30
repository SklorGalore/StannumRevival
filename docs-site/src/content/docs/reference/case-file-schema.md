---
title: Case file tables
description: "Every table and field of the stannum-case 0.1 format: type, unit and default."
---

<!-- Written by the stannum-format tests from crates/stannum-format/schema/stannum-case.toml; don't edit it here. -->

:::caution[Preview]
Format version 0.1 is a preview: it may change in any Stannum release until it reaches 1.0. See [Case file (.stn)](/StannumRevival/reference/case-file/).
:::

Stannum case: the network and its operating state.

A field left out has its default; one with no default is absent, which means none. Units are in the field names (`_pu`, `_mw`, `_kv` and so on) and in the Unit column. Fields starting `x.` are extensions any table may hold.

## case

The case itself: exactly one record, with the id `main`.

Id: `main` · Typed references: `case:<id>`

| Field | Type | Unit | Default | Meaning |
|---|---|---|---|---|
| `name` | string |  | `""` | Case title (RAW title line 1). |
| `description` | string |  | `""` | Second title line or a longer description. |
| `base_mva` | float | MVA | `100` | System MVA base; per-unit values on the system base use it. |
| `frequency_hz` | float | Hz | `60` | System base frequency. |
| `rating_names` | list of string |  | `[A, B, C]` | What rating slots 1 to 12 are, by name. The app treats the first three as ratings A, B and C. |

## area

A control area.

Id: `{number}` · Typed references: `area:<id>` · Sorted by `number`

| Field | Type | Unit | Default | Meaning |
|---|---|---|---|---|
| `number` | int |  | required | Area number. |
| `name` | string |  | `""` | Area name. |
| `slack_bus` | [bus](#bus) id |  | none | Bus whose generation balances the area's interchange; none when absent. |
| `export_mw` | float | MW | `0` | Desired net export; negative imports. |
| `tolerance_mw` | float | MW | `10` | Interchange tolerance band. |

## zone

A zone.

Id: `{number}` · Typed references: `zone:<id>` · Sorted by `number`

| Field | Type | Unit | Default | Meaning |
|---|---|---|---|---|
| `number` | int |  | required | Zone number. |
| `name` | string |  | `""` | Zone name. |

## owner

An owner.

Id: `{number}` · Typed references: `owner:<id>` · Sorted by `number`

| Field | Type | Unit | Default | Meaning |
|---|---|---|---|---|
| `number` | int |  | required | Owner number. |
| `name` | string |  | `""` | Owner name. |

## area_transfer

A scheduled transfer between two areas.

Id: `{from_area}:{to_area}:{label}` · Typed references: `area-transfer:<id>` · Sorted by `from_area`, `to_area`, `label`

| Field | Type | Unit | Default | Meaning |
|---|---|---|---|---|
| `from_area` | [area](#area) id (may be absent) |  | required | Exporting area. |
| `to_area` | [area](#area) id (may be absent) |  | required | Importing area. |
| `label` | string |  | `1` | Transfer identifier. |
| `p_mw` | float | MW | `0` | Scheduled transfer from `from_area` to `to_area`. |

## bus

A bus.

Id: `{number}` · Typed references: `bus:<id>` · Sorted by `number`

| Field | Type | Unit | Default | Meaning |
|---|---|---|---|---|
| `number` | int |  | required | Bus number, 1 to 999997, unique in the case. |
| `name` | string |  | `""` | Bus name. |
| `kv` | float | kV | `0` | Base voltage, line to line. |
| `type` | `pq` \| `pv` \| `ref` \| `isolated` |  | `pq` | How power flow treats the bus: `pq` load bus, `pv` generator bus, `ref` swing bus, `isolated` out of the solve. |
| `area` | [area](#area) id (may be absent) |  | `1` | Area. |
| `zone` | [zone](#zone) id (may be absent) |  | `1` | Zone. |
| `owner` | [owner](#owner) id (may be absent) |  | `1` | Owner. |
| `vm_pu` | float | pu | `1` | Voltage magnitude: the starting or last solved value. |
| `va_deg` | float | deg | `0` | Voltage angle: the starting or last solved value. |
| `vmax_pu` | float | pu | `1.1` | Normal upper voltage limit. |
| `vmin_pu` | float | pu | `0.9` | Normal lower voltage limit. |
| `evmax_pu` | float | pu | `1.1` | Emergency upper voltage limit. |
| `evmin_pu` | float | pu | `0.9` | Emergency lower voltage limit. |
| `lat` | float | deg | none | Latitude (WGS 84), when the bus has a place. |
| `lon` | float | deg | none | Longitude (WGS 84), when the bus has a place. |

## load

A load. Every reactive part is MVAr consumed, so positive is inductive.

Id: `{bus}:{label}` · Typed references: `load:<id>` · Sorted by `bus`, `label`

| Field | Type | Unit | Default | Meaning |
|---|---|---|---|---|
| `bus` | [bus](#bus) id |  | required | Bus. |
| `label` | string |  | `1` | Load identifier, unique at its bus. |
| `status` | on/off |  | `on` | In service (`on`) or out of service (`off`). |
| `area` | [area](#area) id (may be absent) |  | none | Area, when it isn't the bus's. |
| `zone` | [zone](#zone) id (may be absent) |  | none | Zone, when it isn't the bus's. |
| `owner` | [owner](#owner) id (may be absent) |  | none | Owner, when it isn't the bus's. |
| `p_mw` | float | MW | `0` | Constant-power active load. |
| `q_mvar` | float | MVAr | `0` | Constant-power reactive load. |
| `ip_mw` | float | MW | `0` | Constant-current active load at 1 pu voltage. |
| `iq_mvar` | float | MVAr | `0` | Constant-current reactive load at 1 pu voltage. |
| `yp_mw` | float | MW | `0` | Constant-admittance active load at 1 pu voltage. |
| `yq_mvar` | float | MVAr | `0` | Constant-admittance reactive load at 1 pu voltage (the negative of PSS/E's YQ). |
| `scalable` | true/false |  | `true` | Whether load scaling may change it. |
| `interruptible` | true/false |  | `false` | Whether it is interruptible. |
| `dgen_p_mw` | float | MW | `0` | Distributed generation, active. |
| `dgen_q_mvar` | float | MVAr | `0` | Distributed generation, reactive. |
| `dgen_on` | true/false |  | `false` | Whether the distributed generation is in service. |
| `load_type` | string |  | `""` | Free-text load type. |

## shunt

A fixed shunt.

Id: `{bus}:{label}` · Typed references: `shunt:<id>` · Sorted by `bus`, `label`

| Field | Type | Unit | Default | Meaning |
|---|---|---|---|---|
| `bus` | [bus](#bus) id |  | required | Bus. |
| `label` | string |  | `1` | Shunt identifier, unique at its bus. |
| `status` | on/off |  | `on` | In service (`on`) or out of service (`off`). |
| `g_mw` | float | MW | `0` | Active power consumed at 1 pu voltage. |
| `b_mvar` | float | MVAr | `0` | Reactive power injected at 1 pu voltage; positive is capacitive. |

## generator

A generator.

Id: `{bus}:{label}` · Typed references: `generator:<id>` · Sorted by `bus`, `label`

| Field | Type | Unit | Default | Meaning |
|---|---|---|---|---|
| `bus` | [bus](#bus) id |  | required | Bus. |
| `label` | string |  | `1` | Machine identifier, unique at its bus. |
| `status` | on/off |  | `on` | In service (`on`) or out of service (`off`). |
| `p_mw` | float | MW | `0` | Active output. |
| `q_mvar` | float | MVAr | `0` | Reactive output: the starting or last solved value. |
| `qmax_mvar` | float | MVAr | `9999` | Upper reactive limit. |
| `qmin_mvar` | float | MVAr | `-9999` | Lower reactive limit. |
| `vset_pu` | float | pu | `1` | Voltage setpoint at the regulated bus. |
| `reg_bus` | [bus](#bus) id |  | none | Bus whose voltage it holds, when not its own. |
| `reg_node` | int |  | none | Node of the regulated bus in its substation, when the bus has one. |
| `mva_base` | float | MVA | none | Machine base; the system base when absent. |
| `source_r_pu` | float | pu | `0` | Source resistance on the machine base. |
| `source_x_pu` | float | pu | `1` | Source reactance on the machine base. |
| `gsu_r_pu` | float | pu | `0` | Step-up transformer resistance on the machine base; 0 when there is none. |
| `gsu_x_pu` | float | pu | `0` | Step-up transformer reactance on the machine base; 0 when there is none. |
| `gsu_tap_pu` | float | pu | `1` | Step-up transformer ratio. |
| `rmpct` | float | % | `100` | Share of the reactive power that holds a remote bus. |
| `pmax_mw` | float | MW | `9999` | Upper active limit. |
| `pmin_mw` | float | MW | `-9999` | Lower active limit. |
| `dispatch` | `both` \| `down_only` \| `none` \| `up_only` |  | `both` | Which way dispatch may move it: `both`, `down_only`, `none`, `up_only` (PSS/E BASLOD 0 to 3). |
| `owners` | list of [owner id, float] |  | `[]` | Owners and their shares, as `[owner, fraction]` pairs, at most four. Empty means the owner of the first bus, 100 percent. |
| `wind_mode` | `conventional` \| `q_limits` \| `pf_limits` \| `fixed_pf` |  | `conventional` | `conventional`, or a wind machine whose reactive limits come from `q_limits`, from `pf_limits` (wind_pf) or that holds `fixed_pf`. |
| `wind_pf` | float |  | `1` | Power factor for the wind modes that use one. |

## line

An AC line.

Id: `{sort(from,to)}:{ckt}` · Typed references: `line:<id>` · Sorted by `from`, `to`, `ckt`

| Field | Type | Unit | Default | Meaning |
|---|---|---|---|---|
| `from` | [bus](#bus) id |  | required | From bus. |
| `to` | [bus](#bus) id |  | required | To bus. |
| `ckt` | string |  | `1` | Circuit identifier, unique between the two buses. |
| `status` | on/off |  | `on` | In service (`on`) or out of service (`off`). |
| `r_pu` | float | pu | `0` | Series resistance on the system base. |
| `x_pu` | float | pu | `0` | Series reactance on the system base. |
| `b_pu` | float | pu | `0` | Total charging susceptance on the system base. |
| `name` | string |  | `""` | Line name. |
| `rate1_mva` | float | MVA | `0` | Rating 1 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `rate2_mva` | float | MVA | `0` | Rating 2 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `rate3_mva` | float | MVA | `0` | Rating 3 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `rate4_mva` | float | MVA | `0` | Rating 4 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `rate5_mva` | float | MVA | `0` | Rating 5 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `rate6_mva` | float | MVA | `0` | Rating 6 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `rate7_mva` | float | MVA | `0` | Rating 7 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `rate8_mva` | float | MVA | `0` | Rating 8 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `rate9_mva` | float | MVA | `0` | Rating 9 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `rate10_mva` | float | MVA | `0` | Rating 10 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `rate11_mva` | float | MVA | `0` | Rating 11 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `rate12_mva` | float | MVA | `0` | Rating 12 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `gi_pu` | float | pu | `0` | Shunt conductance at the from end, system base. |
| `bi_pu` | float | pu | `0` | Shunt susceptance at the from end, system base. |
| `gj_pu` | float | pu | `0` | Shunt conductance at the to end, system base. |
| `bj_pu` | float | pu | `0` | Shunt susceptance at the to end, system base. |
| `metered` | `from` \| `to` |  | `from` | Which end losses are metered at. |
| `length` | float |  | `0` | Length, in the unit the case's author chose. |
| `owners` | list of [owner id, float] |  | `[]` | Owners and their shares, as `[owner, fraction]` pairs, at most four. Empty means the owner of the first bus, 100 percent. |

## switch

A switching device (breaker, disconnector or other).

Id: `{sort(from,to)}:{ckt}` · Typed references: `switch:<id>` · Sorted by `from`, `to`, `ckt`

| Field | Type | Unit | Default | Meaning |
|---|---|---|---|---|
| `from` | [bus](#bus) id |  | required | From bus. |
| `to` | [bus](#bus) id |  | required | To bus. |
| `ckt` | string |  | `1` | Circuit identifier. |
| `x_pu` | float | pu | `0.0001` | Reactance on the system base. |
| `rate1_mva` | float | MVA | `0` | Rating 1 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `rate2_mva` | float | MVA | `0` | Rating 2 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `rate3_mva` | float | MVA | `0` | Rating 3 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `rate4_mva` | float | MVA | `0` | Rating 4 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `rate5_mva` | float | MVA | `0` | Rating 5 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `rate6_mva` | float | MVA | `0` | Rating 6 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `rate7_mva` | float | MVA | `0` | Rating 7 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `rate8_mva` | float | MVA | `0` | Rating 8 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `rate9_mva` | float | MVA | `0` | Rating 9 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `rate10_mva` | float | MVA | `0` | Rating 10 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `rate11_mva` | float | MVA | `0` | Rating 11 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `rate12_mva` | float | MVA | `0` | Rating 12 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `status` | on/off |  | `on` | Closed (`on`) or open (`off`). |
| `normal` | `closed` \| `open` |  | `closed` | Its normal state. |
| `metered` | `from` \| `to` |  | `from` | Which end losses are metered at. |
| `kind` | `generic` \| `breaker` \| `disconnector` |  | `generic` | What it is. |
| `name` | string |  | `""` | Device name. |

## transformer

A two-winding transformer. Impedances are per unit on its own `mva_base` and winding voltages.

Id: `{sort(from,to)}:{ckt}` · Typed references: `transformer:<id>` · Sorted by `from`, `to`, `ckt`

| Field | Type | Unit | Default | Meaning |
|---|---|---|---|---|
| `from` | [bus](#bus) id |  | required | Winding 1 bus. |
| `to` | [bus](#bus) id |  | required | Winding 2 bus. |
| `ckt` | string |  | `1` | Circuit identifier. |
| `status` | on/off |  | `on` | In service (`on`) or out of service (`off`). |
| `name` | string |  | `""` | Transformer name. |
| `metered` | `from` \| `to` |  | `to` | Which end losses are metered at. |
| `mva_base` | float | MVA | none | Base of `r_pu`, `x_pu` and the magnetizing admittance; the system base when absent. |
| `r_pu` | float | pu | `0` | Series resistance on `mva_base`. |
| `x_pu` | float | pu | `0` | Series reactance on `mva_base`. |
| `mag_g_pu` | float | pu | `0` | Magnetizing conductance on `mva_base`. |
| `mag_b_pu` | float | pu | `0` | Magnetizing susceptance on `mva_base`; negative is inductive. |
| `w1_tap_pu` | float | pu | `1` | Winding 1 ratio, in pu of its nominal voltage. |
| `w1_kv` | float | kV | none | Winding 1 nominal voltage; the from bus's base voltage when absent. |
| `w1_angle_deg` | float | deg | `0` | Winding 1 phase shift. |
| `w2_tap_pu` | float | pu | `1` | Winding 2 ratio, in pu of its nominal voltage. |
| `w2_kv` | float | kV | none | Winding 2 nominal voltage; the to bus's base voltage when absent. |
| `rate1_mva` | float | MVA | `0` | Rating 1 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `rate2_mva` | float | MVA | `0` | Rating 2 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `rate3_mva` | float | MVA | `0` | Rating 3 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `rate4_mva` | float | MVA | `0` | Rating 4 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `rate5_mva` | float | MVA | `0` | Rating 5 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `rate6_mva` | float | MVA | `0` | Rating 6 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `rate7_mva` | float | MVA | `0` | Rating 7 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `rate8_mva` | float | MVA | `0` | Rating 8 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `rate9_mva` | float | MVA | `0` | Rating 9 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `rate10_mva` | float | MVA | `0` | Rating 10 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `rate11_mva` | float | MVA | `0` | Rating 11 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `rate12_mva` | float | MVA | `0` | Rating 12 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `control` | `none` \| `voltage` \| `reactive_flow` \| `active_flow` \| `dc_line` \| `asymmetric_active_flow` |  | `none` | What the winding's tap or angle adjusts to hold: `none`, `voltage`, `reactive_flow`, `active_flow` (a phase shifter), `dc_line`, `asymmetric_active_flow`. |
| `control_on` | true/false |  | `true` | Whether that adjustment is enabled; `false` keeps the control data but locks the tap. |
| `reg_bus` | [bus](#bus) id |  | none | Bus whose voltage the control holds. |
| `reg_bus_reversed` | true/false |  | `false` | The regulated bus is on the winding's own side, so the tap moves the other way (PSS/E negative CONT). |
| `reg_node` | int |  | none | Node of the regulated bus in its substation. |
| `adjust_max` | float | pu or deg | `1.1` | Upper limit of the tap (pu of the winding's nominal voltage) or of the angle (degrees, active-flow control). |
| `adjust_min` | float | pu or deg | `0.9` | Lower limit of the tap or angle. |
| `band_max` | float | pu, MVAr or MW | `1.1` | Upper end of the band the control holds: voltage in pu, or flow in MVAr or MW. |
| `band_min` | float | pu, MVAr or MW | `0.9` | Lower end of that band. |
| `steps` | int |  | `33` | Number of tap positions. |
| `impedance_table` | [impedance_table](#impedance_table) id |  | none | Impedance correction table. |
| `comp_r_pu` | float | pu | `0` | Load-drop compensation resistance. |
| `comp_x_pu` | float | pu | `0` | Load-drop compensation reactance. |
| `comp_angle_deg` | float | deg | `0` | Winding connection angle for asymmetric control. |
| `owners` | list of [owner id, float] |  | `[]` | Owners and their shares, as `[owner, fraction]` pairs, at most four. Empty means the owner of the first bus, 100 percent. |
| `vector_group` | string |  | `""` | Vector group, such as YNd1. |

## transformer3

A three-winding transformer, one record for all three windings. Its typed references use the `transformer` kind, like two-winding ones.

Id: `{sort(bus1,bus2,bus3)}:{ckt}` · Typed references: `transformer:<id>` · Sorted by `bus1`, `bus2`, `bus3`, `ckt`

| Field | Type | Unit | Default | Meaning |
|---|---|---|---|---|
| `bus1` | [bus](#bus) id |  | required | Winding 1 bus. |
| `bus2` | [bus](#bus) id |  | required | Winding 2 bus. |
| `bus3` | [bus](#bus) id |  | required | Winding 3 bus. |
| `ckt` | string |  | `1` | Circuit identifier. |
| `status` | `on` \| `off` \| `w1_off` \| `w2_off` \| `w3_off` |  | `on` | `on`, `off`, or one winding out: `w1_off`, `w2_off`, `w3_off`. |
| `name` | string |  | `""` | Transformer name. |
| `metered` | `w1` \| `w2` \| `w3` |  | `w2` | Which winding losses are metered at. |
| `mva_base12` | float | MVA | none | Base of the winding 1 to 2 impedance; the system base when absent. |
| `r12_pu` | float | pu | `0` | Winding 1 to 2 resistance on `mva_base12`. |
| `x12_pu` | float | pu | `0` | Winding 1 to 2 reactance on `mva_base12`. |
| `mva_base23` | float | MVA | none | Base of the winding 2 to 3 impedance; the system base when absent. |
| `r23_pu` | float | pu | `0` | Winding 2 to 3 resistance on `mva_base23`. |
| `x23_pu` | float | pu | `0` | Winding 2 to 3 reactance on `mva_base23`. |
| `mva_base31` | float | MVA | none | Base of the winding 3 to 1 impedance; the system base when absent. |
| `r31_pu` | float | pu | `0` | Winding 3 to 1 resistance on `mva_base31`. |
| `x31_pu` | float | pu | `0` | Winding 3 to 1 reactance on `mva_base31`. |
| `mag_g_pu` | float | pu | `0` | Magnetizing conductance on `mva_base12`. |
| `mag_b_pu` | float | pu | `0` | Magnetizing susceptance on `mva_base12`; negative is inductive. |
| `star_vm_pu` | float | pu | `1` | Star point voltage magnitude. |
| `star_va_deg` | float | deg | `0` | Star point voltage angle. |
| `w1_tap_pu` | float | pu | `1` | Winding 1 ratio, in pu of its nominal voltage. |
| `w1_kv` | float | kV | none | Winding 1 nominal voltage; bus 1's base voltage when absent. |
| `w1_angle_deg` | float | deg | `0` | Winding 1 phase shift. |
| `w1_rate1_mva` | float | MVA | `0` | Rating 1 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `w1_rate2_mva` | float | MVA | `0` | Rating 2 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `w1_rate3_mva` | float | MVA | `0` | Rating 3 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `w1_rate4_mva` | float | MVA | `0` | Rating 4 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `w1_rate5_mva` | float | MVA | `0` | Rating 5 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `w1_rate6_mva` | float | MVA | `0` | Rating 6 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `w1_rate7_mva` | float | MVA | `0` | Rating 7 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `w1_rate8_mva` | float | MVA | `0` | Rating 8 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `w1_rate9_mva` | float | MVA | `0` | Rating 9 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `w1_rate10_mva` | float | MVA | `0` | Rating 10 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `w1_rate11_mva` | float | MVA | `0` | Rating 11 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `w1_rate12_mva` | float | MVA | `0` | Rating 12 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `w1_control` | `none` \| `voltage` \| `reactive_flow` \| `active_flow` \| `dc_line` \| `asymmetric_active_flow` |  | `none` | What the winding's tap or angle adjusts to hold: `none`, `voltage`, `reactive_flow`, `active_flow` (a phase shifter), `dc_line`, `asymmetric_active_flow`. |
| `w1_control_on` | true/false |  | `true` | Whether that adjustment is enabled; `false` keeps the control data but locks the tap. |
| `w1_reg_bus` | [bus](#bus) id |  | none | Bus whose voltage the control holds. |
| `w1_reg_bus_reversed` | true/false |  | `false` | The regulated bus is on the winding's own side, so the tap moves the other way (PSS/E negative CONT). |
| `w1_reg_node` | int |  | none | Node of the regulated bus in its substation. |
| `w1_adjust_max` | float | pu or deg | `1.1` | Upper limit of the tap (pu of the winding's nominal voltage) or of the angle (degrees, active-flow control). |
| `w1_adjust_min` | float | pu or deg | `0.9` | Lower limit of the tap or angle. |
| `w1_band_max` | float | pu, MVAr or MW | `1.1` | Upper end of the band the control holds: voltage in pu, or flow in MVAr or MW. |
| `w1_band_min` | float | pu, MVAr or MW | `0.9` | Lower end of that band. |
| `w1_steps` | int |  | `33` | Number of tap positions. |
| `w1_impedance_table` | [impedance_table](#impedance_table) id |  | none | Impedance correction table. |
| `w1_comp_r_pu` | float | pu | `0` | Load-drop compensation resistance. |
| `w1_comp_x_pu` | float | pu | `0` | Load-drop compensation reactance. |
| `w1_comp_angle_deg` | float | deg | `0` | Winding connection angle for asymmetric control. |
| `w2_tap_pu` | float | pu | `1` | Winding 2 ratio, in pu of its nominal voltage. |
| `w2_kv` | float | kV | none | Winding 2 nominal voltage; bus 2's base voltage when absent. |
| `w2_angle_deg` | float | deg | `0` | Winding 2 phase shift. |
| `w2_rate1_mva` | float | MVA | `0` | Rating 1 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `w2_rate2_mva` | float | MVA | `0` | Rating 2 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `w2_rate3_mva` | float | MVA | `0` | Rating 3 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `w2_rate4_mva` | float | MVA | `0` | Rating 4 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `w2_rate5_mva` | float | MVA | `0` | Rating 5 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `w2_rate6_mva` | float | MVA | `0` | Rating 6 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `w2_rate7_mva` | float | MVA | `0` | Rating 7 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `w2_rate8_mva` | float | MVA | `0` | Rating 8 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `w2_rate9_mva` | float | MVA | `0` | Rating 9 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `w2_rate10_mva` | float | MVA | `0` | Rating 10 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `w2_rate11_mva` | float | MVA | `0` | Rating 11 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `w2_rate12_mva` | float | MVA | `0` | Rating 12 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `w2_control` | `none` \| `voltage` \| `reactive_flow` \| `active_flow` \| `dc_line` \| `asymmetric_active_flow` |  | `none` | What the winding's tap or angle adjusts to hold: `none`, `voltage`, `reactive_flow`, `active_flow` (a phase shifter), `dc_line`, `asymmetric_active_flow`. |
| `w2_control_on` | true/false |  | `true` | Whether that adjustment is enabled; `false` keeps the control data but locks the tap. |
| `w2_reg_bus` | [bus](#bus) id |  | none | Bus whose voltage the control holds. |
| `w2_reg_bus_reversed` | true/false |  | `false` | The regulated bus is on the winding's own side, so the tap moves the other way (PSS/E negative CONT). |
| `w2_reg_node` | int |  | none | Node of the regulated bus in its substation. |
| `w2_adjust_max` | float | pu or deg | `1.1` | Upper limit of the tap (pu of the winding's nominal voltage) or of the angle (degrees, active-flow control). |
| `w2_adjust_min` | float | pu or deg | `0.9` | Lower limit of the tap or angle. |
| `w2_band_max` | float | pu, MVAr or MW | `1.1` | Upper end of the band the control holds: voltage in pu, or flow in MVAr or MW. |
| `w2_band_min` | float | pu, MVAr or MW | `0.9` | Lower end of that band. |
| `w2_steps` | int |  | `33` | Number of tap positions. |
| `w2_impedance_table` | [impedance_table](#impedance_table) id |  | none | Impedance correction table. |
| `w2_comp_r_pu` | float | pu | `0` | Load-drop compensation resistance. |
| `w2_comp_x_pu` | float | pu | `0` | Load-drop compensation reactance. |
| `w2_comp_angle_deg` | float | deg | `0` | Winding connection angle for asymmetric control. |
| `w3_tap_pu` | float | pu | `1` | Winding 3 ratio, in pu of its nominal voltage. |
| `w3_kv` | float | kV | none | Winding 3 nominal voltage; bus 3's base voltage when absent. |
| `w3_angle_deg` | float | deg | `0` | Winding 3 phase shift. |
| `w3_rate1_mva` | float | MVA | `0` | Rating 1 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `w3_rate2_mva` | float | MVA | `0` | Rating 2 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `w3_rate3_mva` | float | MVA | `0` | Rating 3 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `w3_rate4_mva` | float | MVA | `0` | Rating 4 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `w3_rate5_mva` | float | MVA | `0` | Rating 5 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `w3_rate6_mva` | float | MVA | `0` | Rating 6 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `w3_rate7_mva` | float | MVA | `0` | Rating 7 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `w3_rate8_mva` | float | MVA | `0` | Rating 8 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `w3_rate9_mva` | float | MVA | `0` | Rating 9 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `w3_rate10_mva` | float | MVA | `0` | Rating 10 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `w3_rate11_mva` | float | MVA | `0` | Rating 11 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `w3_rate12_mva` | float | MVA | `0` | Rating 12 in MVA; 0 means none. `case.rating_names` says what each slot is. |
| `w3_control` | `none` \| `voltage` \| `reactive_flow` \| `active_flow` \| `dc_line` \| `asymmetric_active_flow` |  | `none` | What the winding's tap or angle adjusts to hold: `none`, `voltage`, `reactive_flow`, `active_flow` (a phase shifter), `dc_line`, `asymmetric_active_flow`. |
| `w3_control_on` | true/false |  | `true` | Whether that adjustment is enabled; `false` keeps the control data but locks the tap. |
| `w3_reg_bus` | [bus](#bus) id |  | none | Bus whose voltage the control holds. |
| `w3_reg_bus_reversed` | true/false |  | `false` | The regulated bus is on the winding's own side, so the tap moves the other way (PSS/E negative CONT). |
| `w3_reg_node` | int |  | none | Node of the regulated bus in its substation. |
| `w3_adjust_max` | float | pu or deg | `1.1` | Upper limit of the tap (pu of the winding's nominal voltage) or of the angle (degrees, active-flow control). |
| `w3_adjust_min` | float | pu or deg | `0.9` | Lower limit of the tap or angle. |
| `w3_band_max` | float | pu, MVAr or MW | `1.1` | Upper end of the band the control holds: voltage in pu, or flow in MVAr or MW. |
| `w3_band_min` | float | pu, MVAr or MW | `0.9` | Lower end of that band. |
| `w3_steps` | int |  | `33` | Number of tap positions. |
| `w3_impedance_table` | [impedance_table](#impedance_table) id |  | none | Impedance correction table. |
| `w3_comp_r_pu` | float | pu | `0` | Load-drop compensation resistance. |
| `w3_comp_x_pu` | float | pu | `0` | Load-drop compensation reactance. |
| `w3_comp_angle_deg` | float | deg | `0` | Winding connection angle for asymmetric control. |
| `owners` | list of [owner id, float] |  | `[]` | Owners and their shares, as `[owner, fraction]` pairs, at most four. Empty means the owner of the first bus, 100 percent. |
| `vector_group` | string |  | `""` | Vector group. |

## impedance_table

A transformer impedance correction table.

Id: `{number}` · Typed references: `impedance-table:<id>` · Sorted by `number`

| Field | Type | Unit | Default | Meaning |
|---|---|---|---|---|
| `number` | int |  | required | Table number. |
| `points` | list of [float, float, [float]] |  | `[]` | `[ratio_or_angle, factor_re, factor_im]` points in increasing order: tap ratio in pu, or angle in degrees for phase shifters. |

## switched_shunt

A switched shunt.

Id: `{bus}:{label}` · Typed references: `switched-shunt:<id>` · Sorted by `bus`, `label`

| Field | Type | Unit | Default | Meaning |
|---|---|---|---|---|
| `bus` | [bus](#bus) id |  | required | Bus. |
| `label` | string |  | `1` | Shunt identifier, unique at its bus. |
| `mode` | `locked` \| `discrete` \| `continuous` \| `plant_q` \| `vsc_q` \| `shunt_admittance` \| `facts_q` |  | `discrete` | How it switches: `locked`, `discrete`, `continuous`, or discrete to hold a plant's reactive output (`plant_q`), a VSC's (`vsc_q`), another switched shunt's admittance (`shunt_admittance`) or a FACTS device's (`facts_q`). |
| `switching` | `in_order` \| `nearest` |  | `in_order` | Order blocks switch in: `in_order` as listed, or `nearest` admittance first. |
| `status` | on/off |  | `on` | In service (`on`) or out of service (`off`). |
| `vmax_pu` | float | pu or MVAr | `1` | Upper end of the band it holds. |
| `vmin_pu` | float | pu or MVAr | `1` | Lower end of the band it holds. |
| `reg_bus` | [bus](#bus) id |  | none | Bus it regulates, when not its own. |
| `reg_node` | int |  | none | Node of the regulated bus. |
| `rmpct` | float | % | `100` | Share of the reactive power that holds a remote bus. |
| `controller` | string |  | `""` | Name of the VSC line or FACTS device it follows, for the modes that follow one. |
| `b_init_mvar` | float | MVAr | `0` | Switched-in admittance: the starting or last solved value. |
| `blocks` | list of [int, float, [on/off]] |  | `[]` | Blocks as `[count, mvar_per_step]` or `[count, mvar_per_step, status]`; positive MVAr is capacitive. |

## lcc_line

A two-terminal line-commutated DC line.

Id: `{sort(rect_bus,inv_bus)}:{name}` · Typed references: `dcline:<id>` · Sorted by `name`

| Field | Type | Unit | Default | Meaning |
|---|---|---|---|---|
| `name` | string |  | required | Line name, unique among DC lines. |
| `mode` | `blocked` \| `power` \| `current` |  | `blocked` | `blocked`, `power` or `current` control. |
| `r_ohm` | float | ohm | `0` | DC line resistance. |
| `setpoint` | float | MW or A | `0` | Power (MW, negative to hold it at the inverter) or current (A) setpoint. |
| `vsched_kv` | float | kV | `0` | Scheduled DC voltage. |
| `vcmod_kv` | float | kV | `0` | Mode switch DC voltage. |
| `rcomp_ohm` | float | ohm | `0` | Compounding resistance. |
| `margin` | float |  | `0` | Current margin, as a fraction of the setpoint. |
| `metered` | `rectifier` \| `inverter` |  | `inverter` | Which end losses are metered at. |
| `dcvmin_kv` | float | kV | `0` | Smallest compounded DC voltage. |
| `ccc_iterations` | int |  | `20` | Iteration limit for capacitor-commutated converters. |
| `ccc_accel` | float |  | `1` | Acceleration factor for those iterations. |
| `rect_bus` | [bus](#bus) id |  | required | Rectifier AC bus. |
| `rect_bridges` | int |  | `1` | Number of bridges in series. |
| `rect_angle_max_deg` | float | deg | `0` | Largest firing (rectifier) or extinction (inverter) angle. |
| `rect_angle_min_deg` | float | deg | `0` | Smallest firing or extinction angle. |
| `rect_rc_ohm` | float | ohm | `0` | Commutating resistance per bridge. |
| `rect_xc_ohm` | float | ohm | `0` | Commutating reactance per bridge. |
| `rect_ebase_kv` | float | kV | `0` | Primary base AC voltage. |
| `rect_ratio` | float |  | `1` | Transformer ratio. |
| `rect_tap_pu` | float | pu | `1` | Tap setting. |
| `rect_tap_max_pu` | float | pu | `1.5` | Largest tap. |
| `rect_tap_min_pu` | float | pu | `0.51` | Smallest tap. |
| `rect_tap_step_pu` | float | pu | `0.00625` | Tap step. |
| `rect_commutating_bus` | [bus](#bus) id |  | none | Commutating bus, when not the AC bus. |
| `rect_commutating_node` | int |  | none | Node of the commutating bus. |
| `rect_xfmr_from` | [bus](#bus) id |  | none | From bus of a two-winding transformer whose tap this converter controls. |
| `rect_xfmr_to` | [bus](#bus) id |  | none | To bus of that transformer. |
| `rect_xfmr_ckt` | string |  | `1` | Circuit of that transformer. |
| `rect_xcap_ohm` | float | ohm | `0` | Series capacitor reactance (capacitor-commutated converters). |
| `inv_bus` | [bus](#bus) id |  | required | Inverter AC bus. |
| `inv_bridges` | int |  | `1` | Number of bridges in series. |
| `inv_angle_max_deg` | float | deg | `0` | Largest firing (rectifier) or extinction (inverter) angle. |
| `inv_angle_min_deg` | float | deg | `0` | Smallest firing or extinction angle. |
| `inv_rc_ohm` | float | ohm | `0` | Commutating resistance per bridge. |
| `inv_xc_ohm` | float | ohm | `0` | Commutating reactance per bridge. |
| `inv_ebase_kv` | float | kV | `0` | Primary base AC voltage. |
| `inv_ratio` | float |  | `1` | Transformer ratio. |
| `inv_tap_pu` | float | pu | `1` | Tap setting. |
| `inv_tap_max_pu` | float | pu | `1.5` | Largest tap. |
| `inv_tap_min_pu` | float | pu | `0.51` | Smallest tap. |
| `inv_tap_step_pu` | float | pu | `0.00625` | Tap step. |
| `inv_commutating_bus` | [bus](#bus) id |  | none | Commutating bus, when not the AC bus. |
| `inv_commutating_node` | int |  | none | Node of the commutating bus. |
| `inv_xfmr_from` | [bus](#bus) id |  | none | From bus of a two-winding transformer whose tap this converter controls. |
| `inv_xfmr_to` | [bus](#bus) id |  | none | To bus of that transformer. |
| `inv_xfmr_ckt` | string |  | `1` | Circuit of that transformer. |
| `inv_xcap_ohm` | float | ohm | `0` | Series capacitor reactance (capacitor-commutated converters). |

## vsc_line

A two-terminal voltage-source-converter DC line.

Id: `{sort(c1_bus,c2_bus)}:{name}` · Typed references: `dcline:<id>` · Sorted by `name`

| Field | Type | Unit | Default | Meaning |
|---|---|---|---|---|
| `name` | string |  | required | Line name, unique among DC lines. |
| `status` | on/off |  | `on` | In service (`on`) or out of service (`off`). |
| `r_ohm` | float | ohm | `0` | DC line resistance. |
| `owners` | list of [owner id, float] |  | `[]` | Owners and their shares, as `[owner, fraction]` pairs, at most four. Empty means the owner of the first bus, 100 percent. |
| `c1_bus` | [bus](#bus) id |  | required | Converter 1 AC bus. |
| `c1_dc_mode` | `off` \| `voltage` \| `power` |  | `off` | What the converter holds on the DC side: `off`, `voltage` or `power`. |
| `c1_ac_mode` | `voltage` \| `power_factor` |  | `voltage` | What it holds on the AC side: `voltage` or `power_factor`. |
| `c1_dc_set` | float | kV or MW | `0` | DC setpoint: voltage in kV, or power in MW (positive into the DC line). |
| `c1_ac_set` | float | pu or pf | `1` | AC setpoint: voltage in pu, or power factor. |
| `c1_loss_a_kw` | float | kW | `0` | Constant loss. |
| `c1_loss_b_kw_per_a` | float | kW/A | `0` | Loss per ampere of DC current. |
| `c1_loss_min_kw` | float | kW | `0` | Smallest loss. |
| `c1_smax_mva` | float | MVA | `0` | Rating; 0 means none. |
| `c1_imax_a` | float | A | `0` | Current rating; 0 means none. |
| `c1_pwf` | float |  | `1` | Power weighting factor. |
| `c1_qmax_mvar` | float | MVAr | `9999` | Upper reactive limit. |
| `c1_qmin_mvar` | float | MVAr | `-9999` | Lower reactive limit. |
| `c1_reg_bus` | [bus](#bus) id |  | none | Bus whose voltage it holds, when not its own. |
| `c1_rmpct` | float | % | `100` | Share of the reactive power that holds a remote bus. |
| `c2_bus` | [bus](#bus) id |  | required | Converter 2 AC bus. |
| `c2_dc_mode` | `off` \| `voltage` \| `power` |  | `off` | What the converter holds on the DC side: `off`, `voltage` or `power`. |
| `c2_ac_mode` | `voltage` \| `power_factor` |  | `voltage` | What it holds on the AC side: `voltage` or `power_factor`. |
| `c2_dc_set` | float | kV or MW | `0` | DC setpoint: voltage in kV, or power in MW (positive into the DC line). |
| `c2_ac_set` | float | pu or pf | `1` | AC setpoint: voltage in pu, or power factor. |
| `c2_loss_a_kw` | float | kW | `0` | Constant loss. |
| `c2_loss_b_kw_per_a` | float | kW/A | `0` | Loss per ampere of DC current. |
| `c2_loss_min_kw` | float | kW | `0` | Smallest loss. |
| `c2_smax_mva` | float | MVA | `0` | Rating; 0 means none. |
| `c2_imax_a` | float | A | `0` | Current rating; 0 means none. |
| `c2_pwf` | float |  | `1` | Power weighting factor. |
| `c2_qmax_mvar` | float | MVAr | `9999` | Upper reactive limit. |
| `c2_qmin_mvar` | float | MVAr | `-9999` | Lower reactive limit. |
| `c2_reg_bus` | [bus](#bus) id |  | none | Bus whose voltage it holds, when not its own. |
| `c2_rmpct` | float | % | `100` | Share of the reactive power that holds a remote bus. |

## multi_section_line

A line made of several sections in series, grouped for switching.

Id: `{sort(from,to)}:{label}` · Typed references: `multi-section-line:<id>` · Sorted by `from`, `to`, `label`

| Field | Type | Unit | Default | Meaning |
|---|---|---|---|---|
| `from` | [bus](#bus) id |  | required | From bus. |
| `to` | [bus](#bus) id |  | required | To bus. |
| `label` | string |  | `&1` | Group identifier, starting with &. |
| `metered` | `from` \| `to` |  | `from` | Which end losses are metered at. |
| `via` | list of bus id |  | `[]` | Buses between the sections, in order from `from`. |

## facts

A FACTS device: a shunt element, a series element, or both.

Id: `{name}` · Typed references: `facts:<id>` · Sorted by `name`

| Field | Type | Unit | Default | Meaning |
|---|---|---|---|---|
| `name` | string |  | required | Device name. |
| `from` | [bus](#bus) id |  | required | Sending-end bus (shunt element). |
| `to` | [bus](#bus) id |  | none | Terminal-end bus of the series element; none for a shunt-only device. |
| `mode` | `off` \| `normal` \| `shunt_only` \| `constant_impedance` \| `constant_voltage` \| `ipfc_master_pq` \| `ipfc_slave_pq` \| `ipfc_master_voltage` \| `ipfc_slave_voltage` |  | `normal` | Operating mode (PSS/E MODE 0 to 8). |
| `p_set_mw` | float | MW | `0` | Active flow setpoint at the terminal end. |
| `q_set_mvar` | float | MVAr | `0` | Reactive flow setpoint at the terminal end. |
| `vset_pu` | float | pu | `1` | Voltage setpoint at the sending end. |
| `shunt_max_mva` | float | MVA | `9999` | Largest shunt current, as MVA at 1 pu. |
| `series_max_mw` | float | MW | `9999` | Largest bridge active power transfer. |
| `vt_min_pu` | float | pu | `0.9` | Smallest terminal-end voltage. |
| `vt_max_pu` | float | pu | `1.1` | Largest terminal-end voltage. |
| `vs_max_pu` | float | pu | `1` | Largest series voltage. |
| `imax_mva` | float | MVA | `0` | Largest series current, as MVA at 1 pu; 0 means none. |
| `linx_pu` | float | pu | `0.05` | Reactance of the dummy series element used in some modes. |
| `rmpct` | float | % | `100` | Share of the reactive power that holds a remote bus. |
| `owner` | [owner](#owner) id (may be absent) |  | `1` | Owner. |
| `set1` | float |  | `0` | Series impedance (R) or voltage magnitude setpoint, by mode. |
| `set2` | float |  | `0` | Series impedance (X) or voltage angle setpoint, by mode. |
| `series_ref` | `voltage` \| `impedance` |  | `voltage` | What `set1` and `set2` are: series `voltage` or `impedance`. |
| `reg_bus` | [bus](#bus) id |  | none | Bus whose voltage it holds, when not the sending end. |
| `ipfc_partner` | string |  | `""` | Name of the IPFC master device, for slave modes. |
