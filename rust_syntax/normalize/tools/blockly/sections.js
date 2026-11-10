/* Location is an authoring view, not an extra wire-level ability kind. */
'use strict';
function createCardSections() {
  const definitions=[
    ['resolve','Spell effects',null],['battlefield','Entering / on the battlefield','Battlefield'],
    ['casting','Casting this card',null],['hand','In your hand','Hand'],
    ['graveyard','In your graveyard','Graveyard'],['stack','While this spell is on the stack','Stack'],
    ['everywhere','Characteristics / all zones',null],['exile','In exile','Exile'],
    ['library','In your library','Library'],['command','In the command zone','Command'],
    ['pregame','Before the game',null],['other','Other / multiple locations',null]
  ].map(([id,label,zone])=>({id,label,zone}));
  const get=id=>definitions.find(d=>d.id===id);
  const short=id=>({resolve:'Spell resolution',battlefield:'Battlefield',casting:'Casting',hand:'Hand',graveyard:'Graveyard',stack:'Stack',everywhere:'All zones',exile:'Exile',library:'Library',command:'Command',pregame:'Before the game',other:'Multiple / other'})[id];
  const card=s=>s.pr==='single'||s.ty.named==='Card';
  const zoneId=zone=>definitions.find(d=>d.zone===zone)?.id||'other';
  const castingMechanics=new Set(['Disturb','Emerge','Evoke','Kicker','Multikicker','Buyback','Flashback','Retrace','Escape','Dash','Blitz','Bestow','Mutate','Overload','Delve','Convoke','Improvise','Affinity','Spectacle','Miracle','Surge','Prowl','DeliriumCost','Madness']);
  function selfCast(a) {
    if(a.kind!=='Triggered')return false;
    const event=a.args.event?.Spell;
    return event?.event==='Cast'&&event.spells?.kind==='SameObject'&&event.spells.args?.kind==='Source';
  }
  function classify(a,main='battlefield') {
    if(a.kind==='WithPresentation')return classify(a.args.ability,main);
    if(a.kind==='Scoped') {
      if(a.args.zones.length===1&&a.args.zones[0]==='Stack'&&selfCast(a.args.ability))return 'casting';
      return a.args.zones.length===1?zoneId(a.args.zones[0]):'other';
    }
    if(['Spell','CostBranch','CostOptions','Spree'].includes(a.kind))return 'resolve';
    if(selfCast(a))return 'casting';
    if(a.kind==='Activated'&&a.args.origin?.Cycling)return 'hand';
    if(a.kind==='Activated'&&['Equip','Crew'].includes(a.args.origin))return 'battlefield';
    if(['Casting','Offering','FlashIf','Prototype'].includes(a.kind))return 'casting';
    if(['OpeningHand','DeckRule','DraftRule','Companion','CommanderDeckRule'].includes(a.kind))return 'pregame';
    if(a.kind==='Static'&&a.args.kind?.CharacteristicDefining)return 'everywhere';
    if(a.kind==='Replacement'&&a.args.active==='Everywhere')return 'everywhere';
    if(a.kind==='Keyword'&&a.args.kind==='Flash')return 'casting';
    if(a.kind==='Keyword'&&['Changeling','Devoid'].includes(a.args.kind))return 'everywhere';
    if(a.kind==='Mechanic') {
      const k=a.args.mechanic;
      if(['Madness','Foretell','Plot','Suspend','Haunt','Rebound'].includes(k))return 'other';
      if(castingMechanics.has(k)||['Storm','Cascade','Demonstrate','Replicate'].includes(k))return 'casting';
      if(['Changeling','Devoid'].includes(k))return 'everywhere';
      if(k==='SplitSecond')return 'stack';
      if(['Unearth','Scavenge','Embalm','Eternalize','Encore','EscapeFromGraveyard'].includes(k))return 'graveyard';
      if(['Forecast'].includes(k))return 'hand';
    }
    if(a.kind==='Conditional') {
      const groups=new Set([...a.args.then,...a.args.otherwise].map(a=>classify(a,main)));
      return groups.size===1?[...groups][0]:'other';
    }
    return main==='resolve'?'stack':'battlefield';
  }
  function fixed(a) {
    if(a.kind==='WithPresentation')return fixed(a.args.ability);
    if(a.kind==='Scoped')return fixed(a.args.ability)?classify(a):null;
    if(a.kind==='Activated'&&a.args.origin?.Cycling)return 'hand';
    if(a.kind==='Activated'&&['Equip','Crew'].includes(a.args.origin))return 'battlefield';
    if(['Spell','Casting','OpeningHand','DeckRule','DraftRule','Companion','CommanderDeckRule'].includes(a.kind)||selfCast(a))return classify(a);
    if(a.kind==='Keyword'&&['Flash','Changeling','Devoid'].includes(a.args.kind))return classify(a);
    if(a.kind==='Static'&&a.args.kind?.CharacteristicDefining)return 'everywhere';
    if(a.kind==='Mechanic'&&classify(a)!=='battlefield')return classify(a);
    return null;
  }
  function initial(face) {
    const main=face.characteristics.card_types.some(t=>['Instant','Sorcery'].includes(t))?'resolve':'battlefield';
    const order=face.abilities.map(a=>classify(a,main)),sections=definitions.map(d=>d.id).filter(id=>id===main||order.includes(id));
    sections.sort((a,b)=>(a===main?-1:b===main?1:0));
    return {cardMain:main,cardSections:sections,abilitySectionOrder:order};
  }
  const part=id=>({key:'SECTION_'+id,label:get(id).label,ty:{vec:{named:'Ability'}},flow:'array',socketTy:{named:'Ability'},statement:true,abilitySection:id});
  function parts(state,original) {return card(state)&&state.cardSections?original.flatMap(p=>p.key==='F_abilities'?state.cardSections.map(part):[p]):original;}
  function values(state,value,original) {
    if(!state.cardSections)return original;
    const face=state.pr==='single'?value.faces[0]:value;
    return {...original,...Object.fromEntries(state.cardSections.map(id=>['SECTION_'+id,face.abilities.filter(a=>classify(a,state.cardMain)===id)]))};
  }
  // Preserve the imported interleaving; reordering within a section still edits
  // its ability order. New entries fill free positions, then append.
  function entries(block) {
    const queues=new Map(block.state.cardSections.map(id=>{
      const list=[];for(let b=block.getInputTargetBlock('SECTION_'+id);b;b=b.getNextBlock())list.push({block:b,section:id});
      return [id,list];
    }));
    const out=[];
    for(const id of block.state.abilitySectionOrder||[])if(queues.get(id)?.length)out.push(queues.get(id).shift());
    for(const id of block.state.cardSections)out.push(...queues.get(id));
    return out;
  }
  function wire(value,section,main='battlefield') {
    if(classify(value,main)===section)return value;
    const zone=get(section)?.zone;
    if(!zone)throw Error('This ability does not belong in '+get(section).label+'. Choose its functioning location or a matching ability.');
    if(value.kind==='Scoped')return {...value,args:{...value.args,zones:[zone]}};
    return {kind:'Scoped',args:{zones:[zone],condition:null,ability:value}};
  }
  function home(block) {
    let head=block;
    while(head.getPreviousBlock?.()?.state.ty.named==='Ability')head=head.getPreviousBlock();
    const parent=head.getParent?.();
    if(!parent?.state.cardSections)return null;
    const id=parent.state.cardSections.find(id=>parent.getInputTargetBlock('SECTION_'+id)===head);
    return id?{parent,id}:null;
  }
  return {definitions,get,short,card,classify,fixed,initial,parts,values,entries,wire,home,part};
}
