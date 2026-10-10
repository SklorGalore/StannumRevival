import type { GraphData } from './types';
export function focusedGraph(graph:GraphData,selected:Set<string>): GraphData {
  return {...graph,nodes:graph.nodes.filter(n=>selected.has(n.id)),edges:graph.edges.filter(e=>selected.has(e.source)&&selected.has(e.target))};
}

export function nodeForBus(graph:GraphData,bus:number) {
  return graph.nodes.find(node=>node.bus===bus||node.member_buses?.includes(bus));
}

export function neighborhood(graph:GraphData,bus:number,hops=2): GraphData {
  const equipment=new Map<string,Set<string>>();
  const incident=new Map<string,Set<string>>();
  for (const edge of graph.edges) {
    const nodes=equipment.get(edge.equipment_id)??new Set<string>();
    nodes.add(edge.source);nodes.add(edge.target);equipment.set(edge.equipment_id,nodes);
    for (const node of [edge.source,edge.target]) {
      const ids=incident.get(node)??new Set<string>();ids.add(edge.equipment_id);incident.set(node,ids);
    }
  }
  const origin=nodeForBus(graph,bus);
  const selected=new Set(origin?[origin.id]:[`bus:${bus}`]);
  let frontier=new Set(selected);
  for (let i=0;i<hops&&frontier.size;i++) {
    const next=new Set<string>();
    for (const node of frontier) {
      for (const eq of incident.get(node)??[]) {
        for (const neighbor of equipment.get(eq)??[]) {
          if (!selected.has(neighbor)) {selected.add(neighbor);next.add(neighbor);}
        }
      }
    }
    frontier=next;
  }
  return focusedGraph(graph,selected);
}

export function toCsv(rows:Record<string,unknown>[]):string {
  if (!rows.length) return '';
  const columns=Object.keys(rows[0]);
  const cell=(value:unknown)=>{
    let text=value==null?'':Array.isArray(value)?value.join(' → '):String(value);
    if (typeof value==='string' && /^\s*[=+@-]/.test(text)) text="'"+text;
    return /[",\r\n]/.test(text)?`"${text.replaceAll('"','""')}"`:text;
  };
  return [columns.map(cell).join(','),...rows.map(row=>columns.map(key=>cell(row[key])).join(','))].join('\r\n')+'\r\n';
}
