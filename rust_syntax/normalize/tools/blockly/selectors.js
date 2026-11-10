/* Compact typed selectors retain their complete block trees and binding ports. */
'use strict';
function createSelectorEditor(api) {
  const $=id=>document.getElementById(id),dialog=$('selector-dialog');
  const names=new Set(['ObjectSet','ObjectRef','ObjectFilter','Recipients']);
  const supports=t=>names.has(t?.named);
  const pretty=s=>String(s).replace(/_/g,' ').replace(/([a-z])([A-Z])/g,'$1 $2').toLowerCase();
  const plural=s=>({Creature:'creatures',Permanent:'permanents',Card:'cards',Artifact:'artifacts',Enchantment:'enchantments',Land:'lands',Planeswalker:'planeswalkers',Battle:'battles'})[s]||pretty(s)+'s';
  const you={kind:'One',args:{kind:'AbilityController'}},source={kind:'Source'};
  let editing=null,options=[],drag=null,frame=null,pending=null;
  const title=t=>({ObjectSet:'objects',ObjectRef:'one object',ObjectFilter:'matching conditions',Recipients:'damage recipients'})[t.named];
  function name(b,kind,id) {
    if(!b)return '';
    for(const node of [b,...(b.getDescendants?.(false)||[])])for(const m of Object.values(node.state?.bindings||{}))if(m.kind===kind&&(id===undefined||m.id===Number(id)))return m.name;
    return '';
  }
  function announcement(b) {
    let owner=b;while(owner&&owner.state.ty.named!=='Ability')owner=owner.getParent?.();
    // Read declarations independently: the effect being filled may be incomplete.
    const block=owner?.getInputTargetBlock('F_announcement');return block?api.fromBlock(block):null;
  }
  function targetText(b,slot,index) {
    const label=name(b,'Target',slot);let text='announced target',targets=[];
    try {
      targets=announcement(b)?.targets||[];const target=targets.find(t=>t.slot===slot);
      if(target){text=api.targetSummary(target);if(text.includes('custom target')){const filters=target.filter.kind==='All'?target.filter.args:[target.filter],type=filters.find(f=>f.kind==='CardType');if(type)text=text.replace('custom target','target '+pretty(type.args)+' · conditions');}if(index!==undefined&&(index||target.count?.Exactly?.args!==1))text+=' · item '+(index+1);}
    }catch(_){}
    return text+(label&&(label!=='target'||targets.length>1)?' · '+label:'');
  }
  function filter(v,depth=0) {
    if(!v||depth>5)return 'conditions · details';
    if(v.kind==='Any')return 'any object';
    if(v.kind==='CardType')return pretty(v.args);
    if(v.kind==='ControlledBy'&&api.same(v.args,you))return 'controlled by you';
    if(v.kind==='Tapped')return 'tapped';
    if(v.kind==='SameObject'&&v.args.kind==='Source')return 'this card';
    if(v.kind==='Not')return 'not '+filter(v.args,depth+1);
    if(['All','Either'].includes(v.kind)&&!v.args.length)return v.kind==='All'?'any object (no conditions)':'no matching objects';
    if(v.kind==='All'||v.kind==='Either')return v.args.map(x=>filter(x,depth+1)).join(v.kind==='All'?' + ':' or ');
    return pretty(v.kind)+' · details';
  }
  function query(v) {
    const q=v.args,clauses=q.filter.kind==='All'?q.filter.args:[q.filter];
    const type=clauses.find(f=>f.kind==='CardType'),control=clauses.find(f=>f.kind==='ControlledBy'&&api.same(f.args,you));
    const known=clauses.every(f=>f===type||f===control||f.kind==='Any');
    const noun=plural(type?.args||q.kind);
    const zone=q.scope.Shared;
    if(zone==='Battlefield'&&q.kind==='Permanent'&&known)return 'all '+noun+(control?' you control':'');
    let scope=zone?pretty(zone):q.scope.Players&&api.same(q.scope.Players.players,you)?'your '+pretty(q.scope.Players.zone):'selected zones';
    return (known?'all '+noun+(control?' you control':''):'matching '+plural(q.kind))+' in '+scope+(!known?' · '+filter(q.filter):'');
  }
  function describe(t,v,b,depth=0) {
    if(!v||depth>5)return 'selector · details';
    if(t.named==='ObjectFilter')return filter(v);
    if(t.named==='Recipients') {
      if(v.kind==='Objects')return describe({named:'ObjectSet'},v.args,b,depth+1);
      if(v.kind==='Targets')return targetText(b,v.args);
      return pretty(v.kind)+' · details';
    }
    if(t.named==='ObjectSet') {
      if(v.kind==='One')return describe({named:'ObjectRef'},v.args,b,depth+1);
      if(v.kind==='Zone')return query(v);
      if(v.kind==='Targets')return targetText(b,v.args);
      if(['Selected','Created','Observed'].includes(v.kind))return ({Selected:'chosen objects',Created:'created objects',Observed:'looked-at cards'})[v.kind]+(name(b,'Result',v.args)?' · '+name(b,'Result',v.args):'');
      if(v.kind==='Matching')return describe(t,v.args.objects,b,depth+1)+' matching '+filter(v.args.filter);
      if(v.kind==='Union'&&!v.args.length)return 'no objects';
      if(v.kind==='Union')return v.args.map(x=>describe(t,x,b,depth+1)).join(' or ');
    }
    if(t.named==='ObjectRef') {
      if(v.kind==='Source')return 'this card';
      if(v.kind==='Target')return targetText(b,v.args.slot,v.args.index);
      if(v.kind==='Bound')return 'current object · '+(name(b,'Loop',v.args)||'loop item');
      if(v.kind==='Chosen')return 'chosen object';
      if(v.kind==='AttachedTo')return 'host of '+describe(t,v.args,b,depth+1);
    }
    return pretty(v.kind)+' · details';
  }
  function summary(b) {
    if(!supports(b.state?.ty)||b.state.pr==='literal')return null;
    try{const text=describe(b.state.ty,api.fromBlock(b),b);return text.length>92?text.slice(0,89)+'…':text;}catch(_){return title(b.state.ty)+' · fill missing parts';}
  }
  function expand(b) {
    if(!b||b.isInFlyout)return;
    api.remember();
    for(const child of b.getDescendants(false))if(supports(child.state?.ty))child.setCollapsed(false);
    api.applyPresentation();api.changed();b.select();
  }
  function compact(b){if(!b||b.isInFlyout)return;api.remember();b.setCollapsed(true);api.applyPresentation();api.changed();}
  function decorate(b) {
    if(b.isInFlyout||!supports(b.state?.ty)||b.state.pr==='literal'||!b.isCollapsed())return;
    const field=b.getField('_TEMP_COLLAPSED_FIELD'),el=field?.getSvgRoot();if(!el)return;el.setAttribute('aria-label','Expand selector: '+summary(b));if(el.dataset.selectorChip)return;
    el.dataset.selectorChip='true';el.classList.add('selector-chip-label');el.style.cursor='pointer';el.style.pointerEvents='auto';el.removeAttribute('aria-hidden');el.setAttribute('role','button');el.setAttribute('tabindex','0');el.setAttribute('aria-label','Expand selector: '+summary(b));
    let down;
    el.addEventListener('pointerdown',e=>{down={x:e.clientX,y:e.clientY};});
    el.addEventListener('click',e=>{if(down&&Math.hypot(e.clientX-down.x,e.clientY-down.y)<5){e.stopPropagation();expand(b);}});
    el.addEventListener('keydown',e=>{if(['Enter',' '].includes(e.key)){e.preventDefault();expand(b);}});
    el.appendChild(document.createElementNS('http://www.w3.org/2000/svg','title')).textContent='Click to edit selector blocks; drag to move this whole selector.';
  }
  function starters(ty,parent) {
    const out=[],add=(label,value)=>out.push({label,value});
    const all=filter=>({kind:'Zone',args:{scope:{Shared:'Battlefield'},kind:'Permanent',filter}});
    const creatures={kind:'CardType',args:'Creature'},controlled={kind:'ControlledBy',args:you};
    if(ty.named==='ObjectFilter') {
      add('Any object',{kind:'Any'});for(const type of ['Creature','Artifact','Enchantment','Land','Planeswalker'])add(pretty(type),{kind:'CardType',args:type});
      add('Controlled by you',controlled);add('Tapped',{kind:'Tapped'});add('All of these conditions',{kind:'All',args:[creatures,controlled]});
    }else if(ty.named==='ObjectRef')add('This card',source);
    else {
      const wrap=v=>ty.named==='Recipients'?{kind:'Objects',args:v}:v;
      add('This card',wrap({kind:'One',args:source}));add('All creatures',wrap(all(creatures)));add('Creatures you control',wrap(all({kind:'All',args:[creatures,controlled]})));add('All permanents',wrap(all({kind:'Any'})));add('Permanents you control',wrap(all(controlled)));
      if(ty.named==='ObjectSet')add('Cards in your graveyard',{kind:'Zone',args:{scope:{Players:{zone:'Graveyard',players:you}},kind:'Card',filter:{kind:'Any'}}});
    }
    try {
      for(const target of announcement(parent)?.targets||[]) {
        if(ty.named==='ObjectFilter')continue;
        const object=typeof target.kind==='object'&&target.kind.Object;
        if(!object&&ty.named!=='Recipients')continue;
        const label='Target: '+api.targetSummary(target);
        const value=ty.named==='ObjectRef'?{kind:'Target',args:{slot:target.slot,index:0}}:{kind:'Targets',args:target.slot};add(label,value);
      }
    }catch(_){}
    return out;
  }
  function renderOptions() {
    const q=$('selector-search').value.trim().toLowerCase();
    $('selector-options').replaceChildren(...options.filter(o=>o.label.toLowerCase().includes(q)).map(o=>{
      const button=document.createElement('button');button.type='button';button.textContent=o.label;button.onclick=()=>insert(o.value);return button;
    }));
    $('selector-empty').hidden=$('selector-options').childElementCount>0;
  }
  function current() {
    const parent=api.workspace().getBlockById(editing?.parent),p=parent&&api.parts(parent.state).find(p=>p.key===editing.key),input=parent?.getInput(editing.key);
    if(!p||!input?.connection||JSON.stringify(p.ty)!==JSON.stringify(editing.ty))throw Error('This socket changed. Reopen its selector menu.');
    return {parent,p,input};
  }
  function open(parent,key) {
    if(parent.isInFlyout)return;
    const p=api.parts(parent.state).find(p=>p.key===key);if(!supports(p?.ty))return;
    editing={parent:parent.id,key,ty:p.ty};options=starters(p.ty,parent);
    $('selector-title').textContent='Choose '+title(p.ty);$('selector-search').value='';
    const child=parent.getInputTargetBlock(key);$('selector-current').textContent=child?'Current: '+(summary(child)||describe(p.ty,api.fromBlock(child),child)):'Drop a matching selector into this socket, or choose a starter below.';
    $('selector-edit-blocks').textContent=child?'Edit current selector in blocks':'Build a custom selector in blocks';
    renderOptions();dialog.showModal();$('selector-search').focus();
  }
  function insert(value) {
    try {
      const {parent,p,input}=current(),old=parent.getInputTargetBlock(p.key);let before;try{before=old&&api.fromBlock(old);}catch(_){}
      if(old&&api.same(value,before)){dialog.close();expand(old);return;}
      api.remember();Blockly.Events.setGroup(true);
      try {
        // One undoable replacement retains the original tree in history.
        old?.dispose(false);
        const child=Blockly.serialization.blocks.append(api.blockState(p.ty,api.clone(value)),api.workspace());
        input.connection.connect(child.outputConnection);
        child.select();
      }finally{Blockly.Events.setGroup(false);}
      dialog.close();api.applyPresentation();api.changed();
      if(old)api.notice('Selector replaced. Undo restores the previous selector.');
    }catch(e){api.notice(e.message);}
  }
  $('selector-search').oninput=renderOptions;$('selector-cancel').onclick=()=>dialog.close();
  $('selector-edit-blocks').onclick=()=>{try{const {parent,p}=current();let child=parent.getInputTargetBlock(p.key);if(!child){insert(api.defaultValue(p.ty));child=parent.getInputTargetBlock(p.key);}else dialog.close();expand(child);}catch(e){api.notice(e.message);}};
  const guides=$('selector-guides');
  function visible(b) {
    for(let child=b,parent=b.getParent();parent;child=parent,parent=parent.getParent()){const input=parent.inputList.find(i=>i.connection?.targetBlock()===child);if(input&&(parent.isCollapsed()||!input.isVisible()))return false;if(!input&&parent.getNextBlock()!==child)return false;}
    return true;
  }
  function clearPreview(){pending=null;}
  function endDrag(){if(frame!==null)cancelAnimationFrame(frame);frame=null;drag=null;pending=null;guides.replaceChildren();guides.hidden=true;}
  function drawGuides() {
    if(!drag)return;
    const ws=api.workspace(),bounds=$('workspace').getBoundingClientRect();guides.replaceChildren();
    for(const {parent,input} of drag.sockets) {
      if(parent.isDeadOrDying()||!input.connection)continue;
      const point=Blockly.utils.svgMath.wsToScreenCoordinates(ws,new Blockly.utils.Coordinate(input.connection.x,input.connection.y));
      if(point.x<bounds.left||point.x>bounds.right||point.y<bounds.top||point.y>bounds.bottom)continue;
      const mark=document.createElement('div'),active=pending?.connection===input.connection;
      mark.className='selector-socket-guide'+(active?' selector-socket-active':'');mark.dataset.parent=parent.id;mark.dataset.input=input.name;
      const child=input.connection.targetBlock(),rect=child?.getSvgRoot()?.getBoundingClientRect();
      mark.style.left=(rect?.width?rect.x:point.x-12)+'px';mark.style.top=(rect?.height?rect.y:point.y-16)+'px';
      mark.style.width=(rect?.width?Math.min(rect.width,380):150)+'px';mark.style.height=(rect?.height?Math.min(rect.height,48):30)+'px';
      const label=document.createElement('span');label.textContent=active?(pending.replace?'Replace selector here':'Connect selector here'):'Drop '+title(drag.ty)+' here';mark.append(label);guides.append(mark);
    }
    frame=requestAnimationFrame(drawGuides);
  }
  function beginDrag(block) {
    endDrag();if(!supports(block.state?.ty)||!block.outputConnection)return;
    const ws=api.workspace(),family=new Set(block.getDescendants(false)),sockets=[];
    for(const parent of ws.getAllBlocks(false))if(!family.has(parent)&&!parent.isCollapsed()&&visible(parent))for(const input of parent.inputList) {
      if(input.isVisible()&&input.connection?.type===Blockly.ConnectionType.INPUT_VALUE&&ws.connectionChecker.canConnect(block.outputConnection,input.connection,true,Infinity))sockets.push({parent,input});
    }
    drag={block,ty:block.state.ty,sockets};guides.hidden=false;drawGuides();
  }
  function preview(connection,replace){if(drag)pending={connection,replace};}
  return {supports,title,summary,describe,expand,compact,decorate,open,starters,beginDrag,endDrag,preview,clearPreview};
}
