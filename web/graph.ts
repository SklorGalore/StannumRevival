import cytoscape, {type Core} from 'cytoscape';
import {graphStyles,type GraphSelection} from './graph-presentation';
import {nodeForBus} from './view-model';
import type {Snapshot,GraphData,Path} from './types';

export class NetworkGraph {
  private cy:Core|null=null;
  private graph:GraphData|null=null;
  private selection:GraphSelection={bus:null,equipment:null};

  destroy() {
    this.cy?.removeAllListeners();
    this.cy?.destroy();
    this.cy=null;
    this.graph=null;
  }

  updateTheme():void {
    this.cy?.style(graphStyles(typeof document!=='undefined'&&document.documentElement.dataset.theme==='dark'));
  }

  fit() {this.cy?.fit(undefined,35);}

  setSelection(selection:GraphSelection):void {
    this.selection=selection;
    const cy=this.cy;
    if(!cy||!this.graph) return;
    cy.elements().removeClass('selected');
    if(selection.bus!==null) {
      const node=nodeForBus(this.graph,selection.bus);
      if(node) cy.getElementById(node.id).addClass('selected');
    }
    if(selection.equipment!==null) {
      cy.edges().filter(edge=>edge.data('equipment')===selection.equipment).addClass('selected');
      cy.nodes().filter(node=>node.data('equipment')===selection.equipment).addClass('selected');
    }
    this.refreshLabels();
  }

  render(container:HTMLElement|undefined,graph:GraphData,snapshot:Snapshot,path:Path|null,onInspect:(bus:number|null,equipment:string|null)=>void) {
    this.destroy();
    this.graph=graph;
    const bridges=new Set(snapshot.analysis.bridges.map(e=>e.id));
    const cuts=new Set(snapshot.analysis.articulation_buses);
    const syntheticCuts=new Set(snapshot.analysis.synthetic_articulations);
    const pathNodes=new Set(path?.node_ids??[]);
    const pathEdges=new Set(path?.connection_ids??[]);
    const equipment=new Map(snapshot.case.equipment.map(e=>[e.id,e]));
    this.cy=cytoscape({container,
      headless:!container,
      styleEnabled:true,
      elements:[
        ...graph.nodes.map(n=>({data:{id:n.id,label:n.bus===null?'T':n.member_buses.length>1?`${n.bus} · ${n.member_buses.length}`:String(n.bus),bus:n.bus,equipment:n.equipment_id},classes:[n.bus===null?'junction':'bus',n.bus!==null&&cuts.has(n.bus)||syntheticCuts.has(n.id)?'articulation':'',pathNodes.has(n.id)?'path':''].filter(Boolean).join(' ')})),
        ...graph.edges.map(e=>({data:{id:e.id,source:e.source,target:e.target,equipment:e.equipment_id},classes:[equipment.get(e.equipment_id)?.kind??'line',bridges.has(e.id)?'bridge':'',pathEdges.has(e.id)?'path':''].filter(Boolean).join(' ')})),
      ],
      style:graphStyles(typeof document!=='undefined'&&document.documentElement.dataset.theme==='dark'),
      layout:container?{name:graph.nodes.length>180?'circle':'cose',animate:false,padding:40,randomize:false}:{name:'preset'},
      minZoom:.08,maxZoom:4,wheelSensitivity:.25,
    });
    const cy=this.cy;
    cy.on('tap','node',event=>{const n=event.target;onInspect(n.data('bus')??null,n.data('equipment')??null);});
    cy.on('tap','edge',event=>onInspect(null,event.target.data('equipment')));
    cy.on('mouseover','node, edge',event=>event.target.addClass('hovered'));
    cy.on('mouseout','node, edge',event=>event.target.removeClass('hovered'));
    cy.on('zoom',()=>this.refreshLabels());
    this.setSelection(this.selection);
  }

  private refreshLabels() {
    const cy=this.cy;
    if(!cy) return;
    const hide=cy.zoom()<.6;
    cy.nodes().forEach(node=>{
      const important=node.hasClass('selected')||node.hasClass('path')||node.hasClass('bridge')||node.hasClass('articulation');
      node.toggleClass('label-hidden',hide&&!important);
    });
  }
}
