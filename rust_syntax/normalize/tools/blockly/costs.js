/* Common costs have a compact form; unfamiliar components remain exact. */
'use strict';
function createCostEditor({namedAbilities,clone,same,readScalar,fromBlock,replaceView,workspace,notice,manaText,parseMana,loyaltyText,parseLoyalty}) {
  const $=id=>document.getElementById(id),dialog=$('cost-dialog');
  const constant=n=>({kind:'Constant',args:n}),self={kind:'One',args:{kind:'Source'}};
  let editing=null,rows=[];
  function selection(row,tap=false) {
    const filters=[{kind:'ControlledBy',args:{kind:'One',args:{kind:'AbilityController'}}}];
    if(row.type!=='Permanent')filters.push(...row.type.split('+').map(type=>({kind:'CardType',args:type})));
    if(row.other)filters.push({kind:'Not',args:{kind:'SameObject',args:{kind:'Source'}}});
    if(tap)filters.push({kind:'Not',args:{kind:'Tapped'}});
    return {objects:{kind:'Zone',args:{scope:{Shared:'Battlefield'},kind:'Permanent',filter:{kind:'All',args:filters}}},choose:{Exactly:constant(readScalar({named:'i64'},String(row.count).trim()))}};
  }
  function build(row) {
    const n=()=>{const value=readScalar({named:'i64'},String(row.count).trim());if(value<0)throw Error('Cost amounts cannot be negative.');return value;};
    if(row.kind==='self')return {Sacrifice:{objects:clone(self),choose:null}};
    if(row.kind==='sacrifice'){n();return {Sacrifice:selection(row)}};
    if(row.kind==='tap'){n();return {ChangeTapState:{selections:[selection(row,true)],tapped:true,symbol:false}}};
    if(row.kind==='loyalty')return {Loyalty:parseLoyalty(String(row.count),true)};
    if(row.kind==='life')return {Life:constant(n())};
    if(row.kind==='discard')return {Discard:{count:constant(n()),filter:{kind:'Any'},random:false}};
    return clone(row.original);
  }
  function decode(part,index) {
    const base={index,original:clone(part),count:1,type:'Creature',other:false};
    if(same(part,{Sacrifice:{objects:self,choose:null}}))return {...base,kind:'self'};
    if(part.Loyalty&&loyaltyText(part.Loyalty,true)!==undefined)return {...base,kind:'loyalty',count:loyaltyText(part.Loyalty,true)};
    if(part.Life?.kind==='Constant')return {...base,kind:'life',count:part.Life.args};
    if(part.Discard?.count.kind==='Constant'&&part.Discard.filter.kind==='Any'&&!part.Discard.random)return {...base,kind:'discard',count:part.Discard.count.args};
    const tap=part.ChangeTapState,sel=part.Sacrifice||(tap?.tapped&&!tap.symbol&&tap.selections.length===1?tap.selections[0]:null);
    if(sel?.choose?.Exactly?.kind==='Constant'&&sel.objects.kind==='Zone') {
      const q=sel.objects.args,filters=q.filter.kind==='All'?q.filter.args:[q.filter];
      if(q.scope.Shared==='Battlefield'&&q.kind==='Permanent') {
        const control={kind:'ControlledBy',args:{kind:'One',args:{kind:'AbilityController'}}},other={kind:'Not',args:{kind:'SameObject',args:{kind:'Source'}}},untapped={kind:'Not',args:{kind:'Tapped'}};
        const cardTypes=filters.filter(f=>f.kind==='CardType').map(f=>f.args);
        const type=cardTypes.length?cardTypes.join('+'):'Permanent';
        if(['Creature','Artifact','Enchantment','Land','Permanent','Artifact+Creature'].includes(type)&&filters.some(f=>same(f,control))&&(!tap||filters.some(f=>same(f,untapped)))&&filters.every(f=>f.kind==='CardType'||same(f,control)||same(f,other)||!!tap&&same(f,untapped)))return {...base,kind:tap?'tap':'sacrifice',count:sel.choose.Exactly.args,type,other:filters.some(f=>same(f,other))};
      }
    }
    return {...base,kind:'custom'};
  }
  const kinds=[['self','Sacrifice this card'],['sacrifice','Sacrifice'],['tap','Tap'],['life','Pay life'],['discard','Discard'],['loyalty','Change loyalty (+/−)']];
  function select(options,value,change,label) {
    const input=document.createElement('select');input.setAttribute('aria-label',label);
    input.replaceChildren(...options.map(([wire,label])=>{const option=document.createElement('option');option.value=wire;option.textContent=label;return option;}));input.value=value;input.onchange=()=>change(input.value);return input;
  }
  function render() {
    $('cost-extra-rows').replaceChildren(...rows.map((row,i)=>{
      const line=document.createElement('div');line.className='cost-row';
      if(row.kind==='custom'){const label=document.createElement('span');label.textContent='Other cost: '+(typeof row.original==='string'?row.original:Object.keys(row.original)[0]);line.append(label);}
      else {
        line.append(select(kinds,row.kind,v=>{row.kind=v;if(v==='tap')row.other=true;render();},'Additional cost '+(i+1)));
        if(row.kind!=='self'){
          const count=document.createElement('input');count.value=row.count;count.inputMode=row.kind==='loyalty'?'text':'numeric';count.setAttribute('aria-label','Amount for additional cost '+(i+1));count.oninput=()=>row.count=count.value;line.append(count);
        }
        if(row.kind==='loyalty'){const hint=document.createElement('small');hint.textContent='Loyalty change: +1, 0, −2, or −X';line.append(hint);}
        if(['sacrifice','tap'].includes(row.kind)) {
          line.append(select(['Creature','Artifact','Enchantment','Land','Permanent','Artifact+Creature'].map(t=>[t,t==='Artifact+Creature'?'artifact creatures':t.toLowerCase()+'s']),row.type,v=>row.type=v,'Permanents for additional cost '+(i+1)));
          const other=document.createElement('label'),box=document.createElement('input');box.type='checkbox';box.checked=row.other;box.onchange=()=>row.other=box.checked;other.append(box,document.createTextNode('Other than this card'));line.append(other);
          const hint=document.createElement('small');hint.textContent=row.kind==='tap'?'Untapped permanents you control':'Permanents you control';line.append(hint);
        }
      }
      const remove=document.createElement('button');remove.type='button';remove.textContent='Remove';remove.setAttribute('aria-label','Remove additional cost '+(i+1));remove.onclick=()=>{rows.splice(i,1);render();};line.append(remove);return line;
    }));
  }
  $('cost-add').onclick=()=>{rows.push({kind:$('cost-new-kind').value,index:null,count:1,type:'Creature',other:$('cost-new-kind').value==='tap'});render();};
  $('cost-cancel').onclick=()=>dialog.close();
  $('cost-show-blocks').onclick=()=>{const b=workspace().getBlockById(editing.id);dialog.close();if(b){for(let p=b;p;p=p.getParent())p.setCollapsed(false);b.select();workspace().centerOnBlock(b.id);}};
  function open(block) {
    try {
      const value=fromBlock(block),mana=value.parts.map((p,i)=>p.Mana!==undefined?i:-1).filter(i=>i>=0),tap=value.parts.map((p,i)=>p==='TapSymbol'?i:-1).filter(i=>i>=0);
      editing={id:block.id,value,mana:mana.length===1?mana[0]:-1,tap:tap.length===1?tap[0]:-1};
      let owner=block.getParent();while(owner&&owner.state.ty.named!=='Ability')owner=owner.getParent();
      let ability;try{ability=namedAbilities.unscoped(fromBlock(owner));}catch(_){}
      const cycling=!!ability?.args?.origin?.Cycling;
      editing.locked=new Set(value.parts.map((part,i)=>cycling&&same(part,namedAbilities.discard)?i:-1).filter(i=>i>=0));
      editing.crew=ability?.args?.origin==='Crew'?value.parts.findIndex(part=>namedAbilities.crewPower(part)!==undefined):-1;
      if(editing.crew>=0)editing.locked.add(editing.crew);
      const pureCrew=editing.crew>=0&&value.parts.length===1;
      $('cost-crew-fields').hidden=editing.crew<0;$('cost-general-fields').hidden=pureCrew;
      $('cost-cycling-note').hidden=!cycling;
      $('cost-title').textContent=namedAbilities.name(ability)||'Ability cost';
      if(editing.crew>=0)$('cost-crew-power').value=namedAbilities.crewPower(value.parts[editing.crew]);
      rows=value.parts.map(decode).filter(row=>!editing.locked.has(row.index)&&row.index!==editing.mana&&row.index!==editing.tap);rows.forEach(row=>row.initial={kind:row.kind,count:row.count,type:row.type,other:row.other});
      $('cost-mana').value=editing.mana<0?'':manaText(value.parts[editing.mana].Mana);$('cost-mana').disabled=mana.length>1;
      $('cost-tap').checked=editing.tap>=0;$('cost-tap').disabled=tap.length>1;
      $('cost-error').textContent='';render();dialog.showModal();$('cost-mana').focus();
    }catch(e){notice(e.message);}
  }
  function summary(cost) {
    return cost.parts.map(p=>namedAbilities.crewPower(p)!==undefined?'tap creatures · power '+namedAbilities.crewPower(p)+'+':p.Loyalty?(loyaltyText(p.Loyalty,true)??'formula')+' loyalty':p==='TapSymbol'?'{T}':p.Mana!==undefined?manaText(p.Mana):p.Sacrifice&&same(p.Sacrifice.objects,self)?'sacrifice this':p.Life?.kind==='Constant'?'pay '+p.Life.args+' life':p.Sacrifice?'sacrifice':p.ChangeTapState?.tapped?'tap permanents':p.Discard?'discard':typeof p==='string'?p:Object.keys(p)[0]).join(', ')||'No cost';
  }
  $('cost-form').onsubmit=e=>{
    e.preventDefault();
    try {
      const block=workspace().getBlockById(editing.id);if(!block||!same(fromBlock(block),editing.value))throw Error('The cost changed; close and reopen this editor.');
      const values=new Map(rows.filter(r=>r.index!==null).map(row=>[row.index,row.initial&&same(row.initial,{kind:row.kind,count:row.count,type:row.type,other:row.other})?row.original:build(row)]));
      for(const i of editing.locked)values.set(i,clone(editing.value.parts[i]));
      if(editing.crew>=0){const power=readScalar({named:'i64'},$('cost-crew-power').value.trim());if(power<0)throw Error('Crew power cannot be negative.');values.get(editing.crew).Constrained.constraints[0].TotalProperty.amount={kind:'Constant',args:power};}
      const manaChanged=!$('cost-mana').disabled&&$('cost-mana').value.trim()!==(editing.mana<0?'':manaText(editing.value.parts[editing.mana].Mana));
      const mana=manaChanged?parseMana($('cost-mana').value):null;
      if(editing.mana>=0)values.set(editing.mana,manaChanged?(mana===null?null:{Mana:mana}):editing.value.parts[editing.mana]);
      if(editing.tap>=0)values.set(editing.tap,$('cost-tap').checked?'TapSymbol':null);
      const parts=[],mapping=new Map();
      editing.value.parts.forEach((_,i)=>{if(values.get(i)){mapping.set(i,parts.length);parts.push(values.get(i));}});
      // Append new components so existing receipt component paths stay stable.
      if(editing.mana<0&&manaChanged&&mana!==null)parts.push({Mana:mana});
      if(editing.tap<0&&$('cost-tap').checked)parts.push('TapSymbol');
      rows.filter(r=>r.index===null).forEach(row=>parts.push(build(row)));
      let owner=block.getParent();while(owner&&owner.state.ty.named!=='Ability')owner=owner.getParent();
      const renumbered=editing.value.parts.some((_,i)=>mapping.get(i)!==i);
      const activation=owner?.state.variant==='Activated'&&owner.getInputTargetBlock('F_cost')===block;
      // Casting histories can be read by a sibling spell ability; nested costs
      // have longer component paths. Preserve those links conservatively too.
      const referenceRoot=activation?owner:block.getRootBlock();
      function checkReferences(value){if(!value||typeof value!=='object')return;if(value.kind==='CostObjects'&&value.args.component.length&&(activation?mapping.get(Number(value.args.component[0]))!==Number(value.args.component[0]):renumbered))throw Error('An effect refers to these cost components. Keep their positions, or edit the cost and its references in the blocks.');Object.values(value).forEach(checkReferences);}
      if(referenceRoot&&renumbered)checkReferences(fromBlock(referenceRoot));
      if(!same(parts,editing.value.parts))replaceView(block,{}, {parts});dialog.close();
    }catch(error){$('cost-error').textContent=error.message;}
  };
  return {open,summary};
}
