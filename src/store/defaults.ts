import type {
  BusData,
  GeneratorData,
  LoadData,
  TransformerData,
  CircuitBreakerData,
  DisconnectorData,
  FuseData,
  ConductorData,
  SLDNodeType,
} from "./types";

export const defaultBusData: BusData = {
  label: "BUS",
  voltageKV: 115,
  orientation: "horizontal",
  length: 200,
};

export const defaultGeneratorData: GeneratorData = {
  label: "G1",
  nominalMW: 100,
  voltageKV: 13.8,
  powerFactor: 0.85,
  groundingType: "solidly",
};

export const defaultLoadData: LoadData = {
  label: "LOAD",
  nominalMW: 10,
  nominalMVAR: 3,
  loadType: "static",
};

export const defaultTransformerData: TransformerData = {
  label: "T1",
  ratingMVA: 100,
  primaryKV: 115,
  secondaryKV: 13.8,
  connection: "WyeDelta",
  groundingPrimary: "solidly",
  groundingSecondary: "none",
  impedancePct: 8,
};

export const defaultCircuitBreakerData: CircuitBreakerData = {
  label: "CB",
  state: "closed",
  ratingKV: 115,
  ratingKA: 40,
  normallyOpen: false,
};

export const defaultDisconnectorData: DisconnectorData = {
  label: "DS",
  state: "closed",
  ratingKV: 115,
  type: "gang-operated",
};

export const defaultFuseData: FuseData = {
  label: "FU",
  ratingKV: 34.5,
  ratingA: 100,
  type: "current-limiting",
  blown: false,
};

export const defaultConductorData: ConductorData = {
  voltageKV: 115,
  conductorType: "transmission",
};

export const defaultsByType: Partial<Record<SLDNodeType, object>> = {
  bus: defaultBusData,
  generator: defaultGeneratorData,
  load: defaultLoadData,
  transformer: defaultTransformerData,
  circuitBreaker: defaultCircuitBreakerData,
  disconnector: defaultDisconnectorData,
  fuse: defaultFuseData,
};

export const defaultMetadata = {
  title: "Untitled Diagram",
  revision: "A",
  company: "",
  description: "",
  createdAt: new Date().toISOString(),
  modifiedAt: new Date().toISOString(),
  schemaVersion: "1.0" as const,
};
