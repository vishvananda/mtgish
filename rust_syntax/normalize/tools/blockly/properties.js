/* Compact editing of printed characteristics; ability definitions stay intact. */
'use strict';
function createPropertyEditor({types,clone,fromBlock,replaceView,workspace,notice,same,readScalar}) {
  const $=id=>document.getElementById(id),dialog=$('properties-dialog');
  const colors={W:'White',U:'Blue',B:'Black',R:'Red',G:'Green',C:'Colorless',CHOSEN:'Chosen'};
  const letter=color=>Object.keys(colors).find(k=>colors[k]===color);
  function manaText(cost) {
    if(cost===null)return '';
    if(!cost.length)return '0';
    return cost.map(s=>{
      if(typeof s==='string')return '{'+(s==='Snow'?'S':s)+'}';
      if(s.Generic!==undefined)return '{'+s.Generic+'}';
      if(s.Color)return '{'+letter(s.Color)+'}';
      if(s.Hybrid)return '{'+s.Hybrid.map(letter).join('/')+'}';
      if(s.Phyrexian)return '{'+[...s.Phyrexian.map(letter),'P'].join('/')+'}';
      if(s.MonocoloredHybrid)return '{2/'+letter(s.MonocoloredHybrid)+'}';
      throw Error('This mana symbol needs the detailed block editor.');
    }).join('');
  }
  function parseMana(text) {
    const compact=text.trim().toUpperCase().replace(/\s+/g,'');
    if(!compact)return null;
    if(compact==='0')return [];
    const tokens=compact.match(/\{[^{}]+\}|\d+|[WUBRGCSX]/g)||[];
    if(tokens.join('')!==compact)throw Error('Use mana symbols such as 2UU, {W/U}, {2/R}, or {G/P}.');
    return tokens.map(token=>{
      const value=token.replace(/^\{|\}$/g,'');
      if(/^\d+$/.test(value))return {Generic:readScalar({named:'u32'},value)};
      if(value==='X')return 'X';
      if(value==='S')return 'Snow';
      if(colors[value])return {Color:colors[value]};
      const parts=value.split('/'),last=parts.at(-1),colored=parts.slice(0,-1).map(p=>colors[p]);
      if(last==='P'&&parts.length<=3&&colored.every(Boolean))return {Phyrexian:colored};
      if(parts.length===2&&parts[0]==='2'&&colors[last])return {MonocoloredHybrid:colors[last]};
      if(parts.length===2&&parts.every(p=>colors[p]))return {Hybrid:parts.map(p=>colors[p])};
      throw Error('Unknown mana symbol '+token+'.');
    });
  }
  const names=wire=>wire==='AssemblyWorker'?'Assembly-Worker':wire==='Ctan'?"C’tan":wire.replace(/([a-z])([A-Z])/g,'$1 $2');
  const creatures=new Set(types.CreatureType.variants.map(v=>v.wire));
  const normalize=s=>s.toLocaleLowerCase().replace(/[^\p{L}\p{N}]/gu,'');
  const creatureByName=new Map(types.CreatureType.variants.flatMap(v=>[[normalize(v.wire),v.wire],[normalize(names(v.wire)),v.wire]]));
  const common=['Creature','Instant','Sorcery','Artifact','Enchantment','Land','Planeswalker','Battle','Kindred'];
  let editing=null,selected=[];
  function typeChoices(container,values,checked) {
    $(container).replaceChildren(...values.map(type=>{
      const label=document.createElement('label'),input=document.createElement('input');
      input.type='checkbox';input.value=type;input.checked=checked.includes(type);input.name='property-card-type';
      label.append(input,document.createTextNode(names(type)));return label;
    }));
  }
  function selectedTypes(){return [...dialog.querySelectorAll('input[name="property-card-type"]:checked')].map(input=>input.value);}
  function renderCreatures() {
    $('property-creature-chips').replaceChildren(...selected.map(type=>{
      const button=document.createElement('button');button.type='button';button.textContent=names(type)+' ×';button.setAttribute('aria-label','Remove '+names(type));button.onclick=()=>{selected=selected.filter(t=>t!==type);renderCreatures();};return button;
    }));
  }
  function addCreature() {
    const input=$('property-creature'),name=input.value.trim();if(!name)return;
    const type=creatureByName.get(normalize(name));
    if(!type){$('property-error').textContent='Choose a creature type from the suggestions.';input.focus();return false;}
    if(!selected.includes(type))selected.push(type);
    input.value='';$('property-error').textContent='';renderCreatures();return true;
  }
  $('property-creature-options').replaceChildren(...types.CreatureType.variants.map(v=>{const option=document.createElement('option');option.value=names(v.wire);return option;}));
  $('property-add-creature').onclick=addCreature;
  $('property-creature').onkeydown=e=>{if(e.key==='Enter'){e.preventDefault();addCreature();}};
  $('property-has-pt').onchange=()=>{$('property-pt-inputs').hidden=!$('property-has-pt').checked;};
  $('property-cancel').onclick=()=>dialog.close();
  function fill(block) {
    const value=fromBlock(block);editing={id:block.id,value};selected=value.subtypes.filter(t=>creatures.has(t));
    typeChoices('property-common-types',common,value.card_types);
    typeChoices('property-other-types',types.CardType.variants.map(v=>v.wire).filter(t=>!common.includes(t)),value.card_types);
    $('property-rare-types').open=value.card_types.some(t=>!common.includes(t));
    $('property-mana').value=manaText(value.mana_cost);
    $('property-creature').value='';renderCreatures();$('property-error').textContent='';
    $('property-other-subtypes').textContent=value.subtypes.filter(t=>!creatures.has(t)).map(names).join(', ');
    $('property-other-subtypes-row').hidden=!$('property-other-subtypes').textContent;
    $('property-has-pt').checked=value.power_toughness!==null;$('property-pt-inputs').hidden=value.power_toughness===null;
    $('property-power').value=value.power_toughness?.[0]??1;$('property-toughness').value=value.power_toughness?.[1]??1;
    const card=block.getParent();let abilities=[];
    try{const value=fromBlock(card);abilities=(value.faces?.[0]||value).abilities||[];}catch(_){}
    function definesStats(a){
      if(['Scoped','WithPresentation'].includes(a.kind))return definesStats(a.args.ability);
      if(a.kind==='Conditional')return [...a.args.then,...a.args.otherwise].some(definesStats);
      return a.kind==='Static'&&a.args.kind?.CharacteristicDefining&&a.args.changes.some(c=>['SetBasePowerToughness','SetPower','SetToughness'].includes(c.kind));
    }
    const computed=abilities.some(definesStats);
    $('property-computed').hidden=!computed;
    for(const id of ['property-has-pt','property-power','property-toughness'])$(id).disabled=computed;
  }
  function faceChoices(block) {
    const root=workspace().getTopBlocks(false).find(b=>b.state.ty.named==='UnifiedCard');
    if(!root)return [];
    if(root.state.pr==='single')return [root.getInputTargetBlock('F_characteristics')].filter(Boolean);
    const faces=root.getInputTargetBlock('F_faces');
    return faces?Array.from({length:faces.state.count},(_,i)=>faces.getInputTargetBlock('ITEM_'+i)?.getInputTargetBlock('F_characteristics')).filter(Boolean):[];
  }
  function open(block=null) {
    try {
      const faces=faceChoices(),chosen=block||faces[0];if(!chosen){notice('Choose a card face to edit its properties.');return;}
      const options=faces.includes(chosen)?faces:[chosen];
      $('property-face').replaceChildren(...options.map(b=>{const option=document.createElement('option');option.value=b.id;option.textContent=b.getParent()?.getFieldValue('_F_name')||'Card face';return option;}));
      $('property-face').value=chosen.id;$('property-face-row').hidden=options.length<2;
      fill(chosen);if(!dialog.open)dialog.showModal();$('property-mana').focus();
    }catch(e){notice(e.message);}
  }
  $('property-face').onchange=()=>fill(workspace().getBlockById($('property-face').value));
  $('properties-form').onsubmit=e=>{
    e.preventDefault();
    try {
      if($('property-creature').value.trim()&&addCreature()===false)return;
      const block=workspace().getBlockById(editing.id);
      if(!block||!same(fromBlock(block),editing.value))throw Error('The card properties changed; close and reopen this editor.');
      const value=clone(editing.value),typesNow=selectedTypes();
      // Leave imported order and unusual representations exact when untouched.
      value.card_types=[...value.card_types.filter(t=>typesNow.includes(t)),...typesNow.filter(t=>!value.card_types.includes(t))];
      if($('property-mana').value.trim()!==manaText(value.mana_cost))value.mana_cost=parseMana($('property-mana').value);
      const oldCreatures=value.subtypes.filter(t=>creatures.has(t));
      if(!same(oldCreatures,selected))value.subtypes=[...value.subtypes.filter(t=>!creatures.has(t)||selected.includes(t)),...selected.filter(t=>!value.subtypes.includes(t))];
      if(!$('property-has-pt').disabled)value.power_toughness=$('property-has-pt').checked?[readScalar({named:'i32'},$('property-power').value.trim()),readScalar({named:'i32'},$('property-toughness').value.trim())]:null;
      if(!same(value,editing.value))replaceView(block,{},value);
      dialog.close();
    }catch(error){$('property-error').textContent=error.message;}
  };
  return {open,manaText,parseMana};
}
