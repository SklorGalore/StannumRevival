// Ported from nodal's web/field-colors.ts.
import {BUS_FIELDS,EQUIPMENT_FIELDS,fieldOptions,fieldValues,type FieldDefinition,type FieldTarget,type FieldValue} from './field-view';
import type {Snapshot} from './types';

export type ColorScheme={
  field:string;
  palette:string;
  colors:Record<string,string>;
  low:string;
  high:string;
  min:number|null;
  max:number|null;
  missing:string;
  mixed:string;
};
export type ColorSettings={bus:ColorScheme;equipment:ColorScheme};
export type ColorPresentation={
  nodeColors:Record<string,string>;
  edgeColors:Record<string,string>;
  legends:Array<{
    target:FieldTarget;
    label:string;
    entries:Array<{label:string;color:string}>;
    gradient?:{low:string;high:string;min:number;max:number};
  }>;
};

export const COLOR_PALETTES=[
  {id:'categorical-8',label:'Accessible 8',colors:['#0072b2','#e69f00','#009e73','#d55e00','#56b4e9','#cc79a7','#f0e442','#333333']},
  {id:'categorical-10',label:'Tableau 10',colors:['#4e79a7','#f28e2b','#e15759','#76b7b2','#59a14f','#edc949','#af7aa1','#ff9da7','#9c755f','#bab0ab']},
];

const STORAGE_KEY='nodal-field-colors';
const HEX_COLOR=/^#[\da-f]{6}$/i;
const forbiddenKeys=new Set(['__proto__','constructor','prototype']);
const fields:Record<FieldTarget,FieldDefinition[]>={bus:BUS_FIELDS,equipment:EQUIPMENT_FIELDS};
const paletteIds=new Set(COLOR_PALETTES.map(palette=>palette.id));

export function defaultColorSettings():ColorSettings {
  const scheme=():ColorScheme=>({field:'',palette:'categorical-8',colors:{},low:'#f1f5f9',high:'#0f766e',min:null,max:null,missing:'#cbd5e1',mixed:'#8b5cf6'});
  return {bus:scheme(),equipment:scheme()};
}

function validColor(value:unknown):value is string {
  return typeof value==='string'&&HEX_COLOR.test(value);
}

function validField(target:FieldTarget,value:unknown):string {
  if(typeof value!=='string'||value==='') return '';
  return fields[target].some(field=>field.key===value)?value:'';
}

function sanitizeScheme(target:FieldTarget,value:unknown):ColorScheme {
  const fallback=defaultColorSettings()[target];
  if(!value||typeof value!=='object'||Array.isArray(value)) return fallback;
  const candidate=value as Partial<ColorScheme>;
  const colors:Record<string,string>={};
  if(candidate.colors&&typeof candidate.colors==='object'&&!Array.isArray(candidate.colors)) {
    for(const [key,color] of Object.entries(candidate.colors)) {
      if(!forbiddenKeys.has(key)&&validColor(color)) colors[key]=color;
    }
  }
  const min=typeof candidate.min==='number'&&Number.isFinite(candidate.min)?candidate.min:null;
  const max=typeof candidate.max==='number'&&Number.isFinite(candidate.max)?candidate.max:null;
  const boundsAreReversed=min!==null&&max!==null&&min>max;
  return {
    field:validField(target,candidate.field),
    palette:typeof candidate.palette==='string'&&paletteIds.has(candidate.palette)?candidate.palette:fallback.palette,
    colors,
    low:validColor(candidate.low)?candidate.low:fallback.low,
    high:validColor(candidate.high)?candidate.high:fallback.high,
    min:boundsAreReversed?null:min,
    max:boundsAreReversed?null:max,
    missing:validColor(candidate.missing)?candidate.missing:fallback.missing,
    mixed:validColor(candidate.mixed)?candidate.mixed:fallback.mixed,
  };
}

export function loadColorSettings(storage?:Pick<Storage,'getItem'>):ColorSettings {
  try {
    const source=storage??(typeof localStorage==='undefined'?undefined:localStorage);
    const stored=source?.getItem(STORAGE_KEY);
    if(!stored) return defaultColorSettings();
    const value:unknown=JSON.parse(stored);
    if(!value||typeof value!=='object'||Array.isArray(value)) return defaultColorSettings();
    const candidate=value as Partial<ColorSettings>;
    const loaded={bus:sanitizeScheme('bus',candidate.bus),equipment:sanitizeScheme('equipment',candidate.equipment)};
    for(const target of ['bus','equipment'] as const) {
      if(fieldDefinition(target,loaded[target].field)?.type==='text') loaded[target].colors={};
    }
    return loaded;
  } catch {
    return defaultColorSettings();
  }
}

export function saveColorSettings(settings:ColorSettings,storage?:Pick<Storage,'setItem'>):void {
  try {
    const source=storage??(typeof localStorage==='undefined'?undefined:localStorage);
    if(!source) return;
    const safe={bus:sanitizeScheme('bus',settings.bus),equipment:sanitizeScheme('equipment',settings.equipment)};
    for(const target of ['bus','equipment'] as const) {
      if(fieldDefinition(target,safe[target].field)?.type==='text') safe[target].colors={};
    }
    source.setItem(STORAGE_KEY,JSON.stringify(safe));
  } catch {
    // Browser storage may be disabled or unavailable; coloring still works in memory.
  }
}

function fieldDefinition(target:FieldTarget,key:string) {
  return fields[target].find(field=>field.key===key);
}

function textValue(value:FieldValue):string {
  return String(value);
}

function domainLabel(value:FieldValue):string {
  return value===null?'Missing':String(value);
}

function compareValues(a:FieldValue,b:FieldValue):number {
  if(a===null) return b===null?0:1;
  if(b===null) return -1;
  if(typeof a==='number'&&typeof b==='number') return a-b;
  return String(a).localeCompare(String(b),undefined,{numeric:true,sensitivity:'base'})||String(a).localeCompare(String(b));
}

function unique(values:FieldValue[]):FieldValue[] {
  const seen=new Set<string>();
  return values.filter(value=>{
    const key=value===null?'null':`${typeof value}:${String(value)}`;
    if(seen.has(key)) return false;
    seen.add(key);
    return true;
  }).sort(compareValues);
}

function hexChannel(value:number):string {
  return Math.round(value).toString(16).padStart(2,'0');
}

function blend(low:string,high:string,amount:number):string {
  const t=Math.min(1,Math.max(0,amount));
  const lo=low.slice(1).match(/.{2}/g)!.map(value=>parseInt(value,16));
  const hi=high.slice(1).match(/.{2}/g)!.map(value=>parseInt(value,16));
  return `#${lo.map((value,index)=>hexChannel(value+(hi[index]-value)*t)).join('')}`;
}

function colorForValue(value:FieldValue,options:{scheme:ColorScheme;type:'number'|'text'|'category';domain:FieldValue[];min:number;max:number}):string {
  const {scheme,type,domain,min,max}=options;
  if(value===null) return scheme.missing;
  const custom=scheme.colors[textValue(value)];
  if(validColor(custom)) return custom;
  if(type==='number'&&typeof value==='number') {
    const span=max-min;
    const amount=span===0?(value<min?0:value>max?1:.5):(value-min)/span;
    return blend(scheme.low,scheme.high,amount);
  }
  const palette=COLOR_PALETTES.find(item=>item.id===scheme.palette)??COLOR_PALETTES[0];
  const index=Math.max(0,domain.findIndex(item=>item!==null&&textValue(item)===textValue(value)));
  return palette.colors[index%palette.colors.length];
}

function resolveValues(snapshot:Snapshot,target:FieldTarget,id:number|string,field:string):FieldValue[] {
  return fieldValues(snapshot,target,id,field);
}

export function createColorPresentation(snapshot:Snapshot,settings:ColorSettings):ColorPresentation {
  const result:ColorPresentation={nodeColors:{},edgeColors:{},legends:[]};
  for(const target of ['bus','equipment'] as const) {
    const scheme=sanitizeScheme(target,settings[target]);
    const definition=fieldDefinition(target,scheme.field);
    if(!definition) continue;
    const rawDomain=fieldOptions(snapshot,target,scheme.field);
    const domain=unique(rawDomain);
    const numeric=definition.type==='number';
    const numericValues=domain.filter((value):value is number=>typeof value==='number'&&Number.isFinite(value));
    let domainMin=0;
    let domainMax=0;
    if(numericValues.length) {
      domainMin=numericValues[0];
      domainMax=numericValues[0];
      for(const value of numericValues) {
        if(value<domainMin) domainMin=value;
        if(value>domainMax) domainMax=value;
      }
    }
    let min=scheme.min??domainMin;
    let max=scheme.max??domainMax;
    if(min>max) {
      if(scheme.min!==null&&scheme.max===null) max=min;
      else if(scheme.max!==null&&scheme.min===null) min=max;
      else {min=domainMin;max=domainMax;}
    }
    const entries=numeric?[]:domain.map(value=>({label:domainLabel(value),color:colorForValue(value,{scheme,type:definition.type,domain,min,max})}));
    result.legends.push({
      target,
      label:definition.label,
      entries,
      ...(numeric?{gradient:{low:scheme.low,high:scheme.high,min,max}}:{}),
    });
    if(target==='bus') {
      for(const node of snapshot.graph.nodes) {
        if(node.bus===null) continue;
        const values=unique(resolveValues(snapshot,'bus',node.bus,scheme.field));
        const color=values.length>1
          ?scheme.mixed
          :colorForValue(values[0]??null,{scheme,type:definition.type,domain,min,max});
        result.nodeColors[node.id]=color;
      }
    } else {
      for(const edge of snapshot.graph.edges) {
        const values=unique(resolveValues(snapshot,'equipment',edge.equipment_id,scheme.field));
        const color=values.length>1
          ?scheme.mixed
          :colorForValue(values[0]??null,{scheme,type:definition.type,domain,min,max});
        result.edgeColors[edge.id]=color;
      }
    }
  }
  return result;
}
