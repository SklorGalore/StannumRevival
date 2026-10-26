function mpc = two_area
%TWO_AREA  Two asynchronous copies of MATPOWER's 9-bus case and an offshore
%   wind island, for Stannum's VSC DC grid tests and demo. Pair it with
%   two_area.dcgrid.json, which links the three AC grids with two DC grids.
%
%   Buses 101-109 and 201-209 are case9 (area 2 has 10% more load); bus 301
%   is a 160 MW offshore wind plant that is the island's reference, and bus
%   302 its 220 kV collector, where the offshore converter connects.
%
%   case9 is based on data from p. 70 of: Chow, J. H., editor. Time-Scale
%   Modeling of Dynamic Networks with Applications to Power Systems.
%   Springer-Verlag, 1982 (from R.P. Schulz, A.E. Turner and D.N. Ewart,
%   "Long Term Power System Dynamics," EPRI Report 90-7-0, 1974), as
%   distributed with MATPOWER; see MATPOWER-LICENSE.

%% MATPOWER Case Format : Version 2
mpc.version = '2';
mpc.baseMVA = 100;

%% bus data
%	bus_i	type	Pd	Qd	Gs	Bs	area	Vm	Va	baseKV	zone	Vmax	Vmin
mpc.bus = [
	101	3	0	0	0	0	1	1	0	345	1	1.1	0.9;
	102	2	0	0	0	0	1	1	0	345	1	1.1	0.9;
	103	2	0	0	0	0	1	1	0	345	1	1.1	0.9;
	104	1	0	0	0	0	1	1	0	345	1	1.1	0.9;
	105	1	90	30	0	0	1	1	0	345	1	1.1	0.9;
	106	1	0	0	0	0	1	1	0	345	1	1.1	0.9;
	107	1	100	35	0	0	1	1	0	345	1	1.1	0.9;
	108	1	0	0	0	0	1	1	0	345	1	1.1	0.9;
	109	1	125	50	0	0	1	1	0	345	1	1.1	0.9;
	201	3	0	0	0	0	2	1	0	345	1	1.1	0.9;
	202	2	0	0	0	0	2	1	0	345	1	1.1	0.9;
	203	2	0	0	0	0	2	1	0	345	1	1.1	0.9;
	204	1	0	0	0	0	2	1	0	345	1	1.1	0.9;
	205	1	99	33	0	0	2	1	0	345	1	1.1	0.9;
	206	1	0	0	0	0	2	1	0	345	1	1.1	0.9;
	207	1	110	38.5	0	0	2	1	0	345	1	1.1	0.9;
	208	1	0	0	0	0	2	1	0	345	1	1.1	0.9;
	209	1	137.5	55	0	0	2	1	0	345	1	1.1	0.9;
	301	3	0	0	0	0	3	1	0	220	1	1.1	0.9;
	302	1	0	0	0	0	3	1	0	220	1	1.1	0.9;
];

%% generator data
%	bus	Pg	Qg	Qmax	Qmin	Vg	mBase	status	Pmax	Pmin
mpc.gen = [
	101	72.3	27.03	300	-300	1.04	100	1	300	10;
	102	163	6.54	300	-300	1.025	100	1	300	10;
	103	85	-10.95	300	-300	1.025	100	1	300	10;
	201	72.3	27.03	300	-300	1.04	100	1	300	10;
	202	163	6.54	300	-300	1.025	100	1	300	10;
	203	85	-10.95	300	-300	1.025	100	1	300	10;
	301	160	0	80	-80	1.02	200	1	200	0;
];

%% branch data
%	fbus	tbus	r	x	b	rateA	rateB	rateC	ratio	angle	status	angmin	angmax
mpc.branch = [
	101	104	0	0.0576	0	250	250	250	0	0	1	-360	360;
	104	105	0.017	0.092	0.158	250	250	250	0	0	1	-360	360;
	105	106	0.039	0.17	0.358	150	150	150	0	0	1	-360	360;
	103	106	0	0.0586	0	300	300	300	0	0	1	-360	360;
	106	107	0.0119	0.1008	0.209	150	150	150	0	0	1	-360	360;
	107	108	0.0085	0.072	0.149	250	250	250	0	0	1	-360	360;
	108	102	0	0.0625	0	250	250	250	0	0	1	-360	360;
	108	109	0.032	0.161	0.306	250	250	250	0	0	1	-360	360;
	109	104	0.01	0.085	0.176	250	250	250	0	0	1	-360	360;
	201	204	0	0.0576	0	250	250	250	0	0	1	-360	360;
	204	205	0.017	0.092	0.158	250	250	250	0	0	1	-360	360;
	205	206	0.039	0.17	0.358	150	150	150	0	0	1	-360	360;
	203	206	0	0.0586	0	300	300	300	0	0	1	-360	360;
	206	207	0.0119	0.1008	0.209	150	150	150	0	0	1	-360	360;
	207	208	0.0085	0.072	0.149	250	250	250	0	0	1	-360	360;
	208	202	0	0.0625	0	250	250	250	0	0	1	-360	360;
	208	209	0.032	0.161	0.306	250	250	250	0	0	1	-360	360;
	209	204	0.01	0.085	0.176	250	250	250	0	0	1	-360	360;
	301	302	0.002	0.04	0.1	250	250	250	0	0	1	-360	360;
];
