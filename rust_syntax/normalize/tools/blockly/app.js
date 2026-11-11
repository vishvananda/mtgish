/* Card blocks: schema-driven Blockly <-> exact Rust IR, with local Wasm validation. */
'use strict';
(async () => {
  const payload = JSON.parse(document.getElementById('demo-payload').textContent);
  let types=payload.schema.types;
  const schema = payload.schema, authoring = schema.authoring, $ = id => document.getElementById(id);
  const clone = v => structuredClone(v), typeKey = t => JSON.stringify(t);
  const sections=createCardSections();
  const uiLabels={Counter:'Counter spell or ability',};
  const palette={abilities:'#477d70',effects:'#b46b4f',flow:'#9b7a42',choices:'#8762a8',events:'#508d88',targets:'#5a7f9e',values:'#788544',properties:'#657b62',advanced:'#86739a'};
  const pretty = text => text.replace(/_/g,' ').replace(/([a-z0-9])([A-Z])/g,'$1 $2');
  const variantLabel = (type,v) => (type==='CastingRule'?({Permission:'Cast from another zone',AdditionalCost:'Additional casting cost',AlternativeCost:'Alternative casting cost',ReduceCost:'This card costs less to cast',Require:'Casting restriction',Flash:'Cast with flash'})[v.wire]:null) || (type==='Ability'?({Spell:'When this spell resolves',Triggered:'When an event happens',Activated:'Activated ability',Static:'Continuous effect',Casting:'Casting rule',Replacement:'Replace an event',Scoped:'Ability in selected locations',Mechanic:'Keyword mechanic'})[v.wire]:null) || (type==='Operation'?(uiLabels[v.wire]||flowView?.labels[v.wire]||authoring.actions[v.wire]?.label):type==='Instruction'?(flowView?.labels[v.wire]||authoring.instructions?.[v.wire]?.label):type==='KeywordAction'?authoring.keyword_actions?.[v.wire]?.label:type==='ObjectKeywordAction'?authoring.object_keyword_actions?.[v.wire]?.label:null) || valueView?.label(type,v.wire) || pretty(authoring.references[type]?.[v.wire]||v.name);
  const typeName = t => t.named || (t.vec ? `List<${typeName(t.vec)}>` : t.option ? `Optional<${typeName(t.option)}>` : `(${t.tuple.map(typeName).join(', ')})`);
  const descriptor = t => t.named && types[t.named];
  const newtype = t => {const d=descriptor(t);return d?.kind==='struct' && d.fields.kind==='tuple' && d.fields.items.length===1 ? d.fields.items[0]:null;};
  const scalar = t => !!t.named && (!types[t.named] || (newtype(t) && scalar(newtype(t))) || (types[t.named].kind === 'enum' && !types[t.named].adjacent && types[t.named].variants.every(v => v.fields.kind === 'unit')));
  const excluded = v => /^(Draft|Named$)/.test(v.name) || ['Primitive','Form'].includes(v.name);
  const preferred = {Ability:'Spell',Instruction:'Effect',Operation:'Draw',Expr:'Constant',PlayerRef:'AbilityController',PlayerSet:'One',ObjectRef:'Source',ObjectSet:'One',ManaSymbol:'Generic',Keyword:'Flying',Count:'Exactly',Destination:'Exile',MovementDestination:'Exile',ConjureDestination:'Exile',TargetKind:'Object',ObjectKind:'Permanent'};
  const pickVariant = (d,name) => d.variants.find(v => v.wire === preferred[name] || v.name === preferred[name]) || d.variants.find(v => v.fields.kind === 'unit' && !excluded(v)) || d.variants.find(v => !excluded(v)) || d.variants[0];
  // JSON numbers outside JavaScript's safe range are retained as BigInt, never rounded.
  function parseExact(text) {
    const safe = text.replace(/"(?:\\.|[^"\\])*"|-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?/g, token => {
      if (token[0] !== '"' && /^-?\d+$/.test(token) && !Number.isSafeInteger(Number(token))) return `{"$exact_integer":${JSON.stringify(token)}}`;
      return token;
    });
    return JSON.parse(safe, (_,v) => v && typeof v === 'object' && Object.keys(v).length === 1 && typeof v.$exact_integer === 'string' ? BigInt(v.$exact_integer) : v);
  }
  function encode(v, indent = 0) {
    if (typeof v === 'bigint') return v.toString();
    if (v === null || typeof v !== 'object') return JSON.stringify(v);
    const pad = '  '.repeat(indent), next = pad + '  ';
    const parts = Array.isArray(v) ? v.map(x => encode(x,indent+1)) : Object.entries(v).map(([k,x]) => `${JSON.stringify(k)}: ${encode(x,indent+1)}`);
    if (!parts.length) return Array.isArray(v) ? '[]' : '{}';
    return `${Array.isArray(v)?'[':'{'}\n${next}${parts.join(',\n'+next)}\n${pad}${Array.isArray(v)?']':'}'}`;
  }
  function dump(value) {
    if($('dump-format').value==='complete')return encode(value);
    const readable=clone(value);
    for(const face of readable.faces||[])for(const [key,v] of Object.entries(face.characteristics||{}))
      if(v===null || (Array.isArray(v)&&v.length===0) || (v&&typeof v==='object'&&!Array.isArray(v)&&Object.keys(v).length===0))delete face.characteristics[key];
    return encode(readable);
  }
  function fieldValue(t,value) {
    if(newtype(t))return fieldValue(newtype(t),value);
    if (t.named === 'bool') return value ? 'TRUE':'FALSE';
    return String(value);
  }
  function readScalar(t,text) {
    if(newtype(t))return readScalar(newtype(t),text);
    if (t.named === 'String') return text;
    if (t.named === 'bool') return text === true || text === 'TRUE';
    if (types[t.named]) {
      if (!types[t.named].variants.some(v => v.wire === text)) throw Error(`Unknown ${t.named}: ${text}`);
      return text;
    }
    if (!/^-?\d+$/.test(text)) throw Error(`${t.named} requires an integer, not ${JSON.stringify(text)}`);
    const n = BigInt(text), match = /^([ui])(8|16|32|64)$/.exec(t.named);
    if (match) {
      const bits = BigInt(match[2]), signed = match[1] === 'i';
      const min = signed ? -(1n << (bits-1n)) : 0n, max = (1n << (signed ? bits-1n : bits))-1n;
      if (n < min || n > max) throw Error(`${text} is outside ${t.named}`);
    }
    return Number.isSafeInteger(Number(n)) ? Number(n) : n;
  }
  class CardDropdown extends Blockly.FieldDropdown {
    isClickableInFlyout(){return true;}
  }
  function field(t,value) {
    if(newtype(t))return field(newtype(t),value);
    if (t.named === 'bool') return new Blockly.FieldCheckbox(fieldValue(t,value));
    if (types[t.named]) return new CardDropdown(types[t.named].variants.map(v => [pretty(v.name),v.wire]));
    return new Blockly.FieldTextInput(String(value));
  }
  function defaultValue(t, path = []) {
    if (path.length > 24 || path.filter(n => n === typeKey(t)).length > 2) throw Error(`Choose an explicit child for ${typeName(t)}`);
    path = [...path,typeKey(t)];
    if (t.vec) return [];
    if (t.option) return null;
    if (t.tuple) return t.tuple.map(t => defaultValue(t,path));
    const d = descriptor(t);
    if (!d) return t.named === 'String' ? '' : t.named === 'bool' ? false : 0;
    if (d.kind === 'struct') {
      if (t.named === 'ResultId') return 4294967295;
      if (t.named === 'TargetSpec') return {...fieldsDefault(d.fields,path,t.named),slot:0,...clone(targetPresets.find(([label])=>label==='target creature')[1])};
      return fieldsDefault(d.fields,path,t.named);
    }
    const v = pickVariant(d,t.named), args = fieldsDefault(v.fields,path,`${t.named}::${v.wire}`);
    return packEnum(d,v,args);
  }
  function fieldsDefault(f,path,owner) {
    if (f.kind === 'unit') return null;
    if (f.kind === 'record') return Object.fromEntries(f.fields.map(x => [x.name,authoring.defaults[owner]?.[x.name]!==undefined?clone(authoring.defaults[owner][x.name]):defaultValue(x.ty,path)]));
    const a = f.items.map(t => defaultValue(t,path)); return a.length === 1 ? a[0] : a;
  }
  function packEnum(d,v,args) {
    return d.adjacent ? (v.fields.kind === 'unit' ? {kind:v.wire} : {kind:v.wire,args}) : (v.fields.kind === 'unit' ? v.wire : {[v.wire]:args});
  }
  function variantFor(t,value) {
    const d=descriptor(t); if (!d || d.kind !== 'enum') return null;
    const wire = d.adjacent ? value.kind : typeof value === 'string' ? value : Object.keys(value)[0];
    const v=d.variants.find(v => v.wire === wire); if (!v) throw Error(`Unknown ${t.named} variant ${wire}`);
    return v;
  }
  const SELF={kind:'AbilityController'};
  const choices={
    PlayerSet:[['you',{kind:'One',args:SELF}],['each player',{kind:'All'}],['your opponents',{kind:'OpponentsOf',args:SELF}]],
    PlayerRef:[['you',SELF],['the active player',{kind:'Active'}]],
    ObjectRef:[['this card',{kind:'Source'}]],
    Recipients:[['chosen target',{kind:'Targets',args:0}],['chosen object target',{kind:'Objects',args:{kind:'Targets',args:0}}],['chosen player target',{kind:'Players',args:{kind:'Targets',args:0}}],['you',{kind:'Players',args:{kind:'One',args:SELF}}]],
    EventSpec:[
      ['this enters the battlefield',{ZoneChange:{objects:{kind:'SameObject',args:{kind:'Source'}},from:null,to:'Battlefield'}}],
      ['this card is cast',{Spell:{event:'Cast',players:{kind:'One',args:SELF},spells:{kind:'SameObject',args:{kind:'Source'}}}}],
      ['this dies',{ZoneChange:{objects:{kind:'SameObject',args:{kind:'Source'}},from:'Battlefield',to:'Graveyard'}}],
      ['this attacks',{Object:{event:'Attacks',objects:{kind:'SameObject',args:{kind:'Source'}}}}],
      ['you cast a spell',{Spell:{event:'Cast',players:{kind:'One',args:SELF},spells:{kind:'Any'}}}],
      ['your upkeep begins',{Turn:{point:'UpkeepBegins',players:{kind:'One',args:SELF}}}]
    ],
  };
  const same=(a,b)=>{
    if(a===b)return true;
    if(a===null||b===null||typeof a!=='object'||typeof b!=='object')return false;
    if(Array.isArray(a)!==Array.isArray(b))return false;
    const keys=Object.keys(a);return keys.length===Object.keys(b).length&&keys.every(k=>Object.hasOwn(b,k)&&same(a[k],b[k]));
  };
  const projections=createAuthoringProjections({types,clone,same});
  const namedAbilities=createNamedAbilities({abilityValue,actionValue,clone,same});
  const choiceView=createChoiceView({clone,defaultValue,fromBlock,parts,replaceView,remember,changed,applyPresentation,notice,addToFlow,bindings:()=>bindingEditor,workspace:()=>workspace,childValues,assemble});
  const flowView=createFlowView({actions:authoring.actions,parts,remember,addToFlow,instruction:name=>{const v=types.Instruction.variants.find(v=>v.wire===name);return packEnum(types.Instruction,v,fieldsDefault(v.fields,[],`Instruction::${name}`));},block:(name,value)=>flyoutBlock('Instruction',value),effect:(name,values)=>flyoutBlock('Instruction',actionValue(name,values))});
  const valueView=createValueView({policy:schema.authoring.values,parts,block:flyoutBlock,count:()=>flyoutBlock('Expr',{kind:'CountObjects',args:selectorEditor.starters({named:'ObjectSet'},null).find(o=>o.label==='Creatures you control').value})});
  const targetPresets=authoring.targets.map(t=>[t.label,t.fields]);
  const permanentTarget=filter=>({kind:{Object:{kind:'Permanent',zones:['Battlefield']}},filter,players:null});
  targetPresets.push(['target permanent',permanentTarget({kind:'Any'})],...['Artifact','Enchantment','Land'].map(type=>['target '+type.toLowerCase(),permanentTarget({kind:'CardType',args:type})]),['target spell',{kind:{Object:{kind:'Spell',zones:['Stack']}},filter:{kind:'Any'},players:null}],['target creature you control',permanentTarget({kind:'AllOf',args:[{kind:'CardType',args:'Creature'},{kind:'ControlledBy',args:{kind:'One',args:SELF}}]})],['target opponent',{kind:'Player',filter:{kind:'Any'},players:{kind:'OpponentsOf',args:SELF}}]);
  targetPresets.push(['target ability',{kind:{Object:{kind:'Ability',zones:['Stack']}},filter:{kind:'Any'},players:null}],['target spell or ability',{kind:{Object:{kind:'Any',zones:['Stack']}},filter:{kind:'Any'},players:null}]);
  const targetPreset = v => targetPresets.findIndex(([,p])=>Object.keys(p).every(k=>same(p[k],v[k])));
  function targetSummary(v) {
    const i=targetPreset(v),base=i<0?'custom target':targetPresets[i][0];
    const count=v.count?.Exactly?.kind==='Constant'?v.count.Exactly.args:null;
    const prefix=v.count==='AnyNumber'?'any number · ':v.count?.UpTo?.kind==='Constant'?'up to '+v.count.UpTo.args+' × ':count===1?'':count!==null?count+' × ':'variable count · ';
    return `${prefix}${base}${v.constraints.length||v.distribute!==null||v.chooser!==null?' · with conditions':''}`;
  }
  function mapTargetDomain(t,value,slot,preset,root=false) {
    // Nested ability definitions introduce their own target-slot scope.
    if(t.named==='Ability'&&!root)return value;
    if(t.named==='Mode'&&!root&&value.announcement.targets.some(t=>t.slot===slot))return value;
    if(t.named==='Operation'&&!root&&value.args?.announcement?.targets?.some(t=>t.slot===slot))return value;
    if(t.named==='TargetSpec'&&value.slot===slot)return {...value,...clone(preset)};
    if(t.named==='Recipients') {
      const direct=value.kind==='Targets'?value.args:['Objects','Players'].includes(value.kind)&&value.args.kind==='Targets'?value.args.args:null;
      if(direct===slot)return preset.kind==='DamageRecipient'?{kind:'Targets',args:slot}:preset.kind==='Player'?{kind:'Players',args:{kind:'Targets',args:slot}}:{kind:'Objects',args:{kind:'Targets',args:slot}};
    }
    if(scalar(t))return value;
    if(t.vec)return value.map(v=>mapTargetDomain(t.vec,v,slot,preset));
    if(t.option)return value===null?null:mapTargetDomain(t.option,value,slot,preset);
    const d=descriptor(t),v=variantFor(t,value),f=t.tuple?{kind:'tuple',items:t.tuple}:v?v.fields:d.fields;
    const args=v?(d.adjacent?value.args:value[v.wire]):value;
    let mapped=null;
    if(f.kind==='record')mapped=Object.fromEntries(f.fields.map(f=>[f.name,mapTargetDomain(f.ty,args[f.name],slot,preset)]));
    if(f.kind==='tuple'){const items=f.items.map((ty,i)=>mapTargetDomain(ty,f.items.length===1?args:args[i],slot,preset));mapped=items.length===1?items[0]:items;}
    return v?packEnum(d,v,mapped):mapped;
  }
  function resetFlyoutBlock(b,value) {
    const fresh=starterState(blockState(b.state.ty,value)),old=b.state;
    Blockly.Events.disable();try{
      for(const input of b.inputList)input.connection?.targetBlock()?.dispose(false);
      b.loadExtraState({...fresh.extraState,statement:old.statement,freshTarget:old.freshTarget,autoResult:old.autoResult});
      for(const [name,value] of Object.entries(fresh.fields||{}))b.setFieldValue(value,name);
      for(const [key,entry] of Object.entries(fresh.inputs||{})){
        const connection=b.getInput(key).connection;
        if(entry.shadow)connection.setShadowState(entry.shadow);
        if(entry.block){const child=Blockly.serialization.blocks.append(entry.block,b.workspace);for(const node of child.getDescendants(false))node.isInFlyout=true;connection.connect(child.outputConnection||child.previousConnection);}
      }
      b.setCollapsed(!!fresh.collapsed);b.render();
    }finally{Blockly.Events.enable();}
    applyPresentation();
  }
  function chooseFlyoutVariant(b,name) {
    const s=b.state,type=s.pr==='keyword'?'Keyword':s.ty.named,d=types[type],variant=d?.variants?.find(v=>v.wire===name);
    const value=s.pr==='effect'?actionValue(name):s.pr==='keyword'?{kind:'Keyword',args:packEnum(d,variant,fieldsDefault(variant.fields,[],type+'::'+name))}:type==='Ability'?abilityValue(name):type==='Instruction'?flowView.starter(name):valueView.starter(type,name)||packEnum(d,variant,fieldsDefault(variant.fields,[],type+'::'+name));
    resetFlyoutBlock(b,scopeValue(s,value));
  }
  function chooseTarget(b,index) {
    if(b.isInFlyout){
      if(index==='CUSTOM'){b.state.shown=[...new Set([...(b.state.shown||[]),'F_kind','F_filter','F_players'])];applyPresentation();return;}
      const preset=targetPresets[Number(index)];if(preset)resetFlyoutBlock(b,{...fromBlock(b),...clone(preset[1])});return;
    }
    if(index==='CUSTOM') {
      b.state.shown=[...new Set([...(b.state.shown||[]),'F_kind','F_filter','F_players'])];
      applyPresentation();changed();return;
    }
    const preset=targetPresets[Number(index)];if(!preset)return;
    const owner=choiceView.scope(b);
    if(owner) {
      const slot=Number(b.getFieldValue('_F_slot'));
      // Keep recipient references in the same scope consistent with the new entity kind.
      let value;try{value=fromBlock(owner);}catch{/* An unfinished body can still edit its announcement. */}
      if(value){
        if(owner.state.pr==='effect')value.args.operation=mapTargetDomain({named:'Operation'},value.args.operation,slot,preset[1],true);
        else Object.assign(value,mapTargetDomain(owner.state.ty,value,slot,preset[1],true));
        replaceView(owner,{},value);return;
      }
    }
    Blockly.Events.setGroup(true);
    try {
      // Change only the target domain; keep its slot, count, distribution and constraints.
      for(const [key,value] of Object.entries(preset[1])) {
        const name='F_'+key,p=parts(b.state).find(p=>p.key===name);
        b.getInputTargetBlock(name)?.dispose(false);
        const block=Blockly.serialization.blocks.append(blockState(p.ty,value),workspace);
        b.getInput(name).connection.connect(block.outputConnection);
      }
      b.state.shown=(b.state.shown||[]).filter(k=>!['F_kind','F_filter','F_players'].includes(k));
    }finally{Blockly.Events.setGroup(false);}
    applyPresentation();changed();
  }
  const viewVariant = state => state.pr==='effect'?types.Operation.variants.find(v=>v.wire===state.op):state.pr==='keyword'?types.Keyword.variants.find(v=>v.wire===state.keyword):descriptor(state.ty)?.variants?.find(v=>v.wire===state.variant);
  function fieldParts(f,prefix='F_') {
    if(f.kind==='unit')return [];
    if(f.kind==='record')return f.fields.map(f=>({key:prefix+f.name,label:pretty(f.name),ty:f.ty}));
    return f.items.map((ty,i)=>({key:`${prefix}ARG_${i}`,label:f.items.length===1?'value':String(i+1),ty}));
  }
  function parts(state) {
    const t=state.ty,d=descriptor(t);let result;
    if(state.pr==='literal') {
      result=[{key:'VALUE',label:t.named==='EventSpec'?'When':'',ty:t,inline:state.literal}];
    } else if(state.pr==='count')result=[{key:'COUNTED',label:'',ty:{named:'ObjectSet'}}];
    else if(state.pr==='query')result=[{key:'Q_CARD',label:'',ty:{named:'String'},options:projections.cards},{key:'Q_CONTROL',label:'',ty:{named:'String'},options:[['any controller','Any'],['you control','You']]}];
    else if(state.pr==='single') {
      // The single-card projection keeps its hidden Card tag as a literal.
      // General records use the schema's typed Layout field; preserving this
      // projection field also lets old saved cards decode before migration.
      result=[{key:'ROOT_schema',label:'schema version',ty:{named:'u32'}},{key:'ROOT_layout',label:'layout',ty:{named:'String'}},{key:'ROOT_data',label:'layout data',ty:{vec:{named:'Field'}}},
        ...fieldParts(types.Card.fields)];
    } else if(state.pr==='effect') {
      result=[{key:'RESULT',label:'result ID',ty:{named:'ResultId'}},...fieldParts(viewVariant(state).fields,'O_')];
      if(['Draw','GainLife','LoseLife','DealDamage'].includes(state.op))result.sort((a,b)=>(a.key==='O_amount'?-2:a.key==='RESULT'?2:0)-(b.key==='O_amount'?-2:b.key==='RESULT'?2:0));
    } else if(state.pr==='keyword') result=fieldParts(viewVariant(state).fields,'K_');
    else if(scalar(t))result=[{key:'VALUE',label:t.named,ty:t}];
    else if(t.vec)result=Array.from({length:state.count},(_,i)=>({key:`ITEM_${i}`,label:`${i+1}`,ty:t.vec,statement:!scalar(t.vec)}));
    else if(t.option)result=state.present?[{key:'SOME',label:'value',ty:t.option}]:[];
    else {
      const f=t.tuple?{kind:'tuple',items:t.tuple}:d.kind==='struct'?d.fields:viewVariant(state).fields;
      result=fieldParts(f,f.kind==='tuple'?'':'F_');
    }
    return valueView.parts(state,flowView.parts(state,choiceView.parts(state,sections.parts(state,result)))).map(p=>{
      const inline=state.inline?.[p.key];
      if(inline && (state.ui<3 || inline==='pt') && !state.noInline?.includes(p.key))p={...p,inline,editTy:inline==='constant'?{named:'i64'}:p.ty};
      if(state.ui>=2 && !p.inline) {
        if(p.ty.named==='Program')p={...p,flow:'program',socketTy:{named:'Instruction'},statement:true};
        else if(p.ty.vec && ['Ability','Instruction'].includes(p.ty.vec.named))p={...p,flow:'array',socketTy:p.ty.vec,statement:true};
      }
      if(state.negativePrevention && p.key.endsWith('preventable'))p={...p,inline:'unpreventable',label:"can't be prevented"};
      if(state.ty.named==='Ability'&&state.variant==='Activated'&&p.key==='F_origin')p.label='mechanic identity';
      if(state.ty.named==='Ability'&&state.variant==='Triggered'&&p.key==='F_event')p.label=p.inline==='choice'?'When':'';
      if(state.ty.named==='Ability'&&state.variant==='Scoped'&&p.key==='F_condition')p.label='only while';
      if(state.ty.named==='Ability'&&state.variant==='Scoped'&&p.key==='F_ability')p.label='';
      if(p.label==='characteristics')p.label='properties';
      if(p.label==='announcement')p.label=state.ty.named==='Ability'&&state.variant==='Spell'?'Cast-time choices':'Targets & modes';
      if(p.flow&&!p.abilitySection&&!state.flowOption&&p.socketTy?.named!=='Program')p.label=p.ty.named==='Program'?(p.label==='otherwise'?'otherwise':'do'):['TargetSpec','Mode'].includes(p.ty.vec?.named)?'':p.ty.vec.named==='Ability'?'abilities':'do';
      if(state.pr==='mode'&&p.key==='F_body')p.label='effects';
      if(state.ty.named==='Ability'&&state.variant==='Spell'&&p.key==='F_body')p.label='effects';
      if(state.pr==='effect'&&state.op==='Counter'){
        if(['O_objects','O_object'].includes(p.key))p.label='';
        if(p.key==='O_destination')p.label='put countered spell in';
        if(p.key==='O_destination_when')p.label='use this destination only if';
      }
      if(state.pr==='effect') {
        if(p.key==='O_amount') {p.label='';p.suffix='';}
        if(p.key==='O_players')p.label='for';
        if(p.key==='O_recipients')p.label='to';
        if(p.key==='O_source')p.label='from';
      }
      return p;
    });
  }
  function stateFor(t,value) {
    if(t.vec)return {ty:t,count:value.length,ui:3};
    if(t.option)return {ty:t,present:value!==null,ui:3};
    let sourceScope;
    if(t.named==='Ability'&&value.kind==='Scoped'&&value.args.condition===null&&value.args.ability.kind!=='Scoped'){sourceScope=clone(value.args.zones);value=value.args.ability;}
    const variant=variantFor(t,value),s={ty:t,ui:3,compactFlow:true,negativePrevention:true,...(variant?{variant:variant.wire}:{}),...(sourceScope?{sourceScope}:{})};
    if(t.named==='Expr'&&value.kind==='CountObjects')s.pr='count';
    if(t.named==='ObjectSet'&&projections.query(value)){s.pr='query';s.queryRaw=clone(value);}
    if(t.named==='Expr'&&value.kind==='Constant'){s.pr='literal';s.literal='constant';}
    else if(choices[t.named]?.some(([,v])=>same(v,value))){s.pr='literal';s.literal='choice';}
    if(t.named==='UnifiedCard' && value.faces.length===1)s.pr='single';
    if(t.named==='Instruction' && value.kind==='Effect'){s.pr='effect';s.op=value.args.operation.kind;}
    if(t.named==='Ability' && value.kind==='Keyword'){s.pr='keyword';s.keyword=value.args.kind;}
    if(t.named==='Ability'&&value.kind==='Activated')s.activationName=namedAbilities.name(value);
    if(sections.card(s))Object.assign(s,sections.initial(s.pr==='single'?value.faces[0]:value));
    return valueView.state(t,value,choiceView.state(t,value,s));
  }
  function fieldValues(f,a,prefix='F_') {
    if(f.kind==='unit')return {};
    if(f.kind==='record')return Object.fromEntries(f.fields.map(f=>[prefix+f.name,a[f.name]]));
    return Object.fromEntries(f.items.map((_,i)=>[`${prefix}ARG_${i}`,f.items.length===1?a:a[i]]));
  }
  function childValues(t,value,state) {
    if(state.sourceScope)value=value.args.ability;
    if(state.valueList!==undefined)return valueView.listValues(state,value);
    if(state.pr==='literal')return {VALUE:value};
    if(state.pr==='count')return {COUNTED:value.args};
    if(state.pr==='query')return projections.query(value);
    if(state.pr==='single')return sections.values(state,value,{ROOT_schema:value.schema_version,ROOT_layout:value.layout,ROOT_data:value.data,...fieldValues(types.Card.fields,value.faces[0])});
    if(state.pr==='effect')return {RESULT:value.args.result,...fieldValues(viewVariant(state).fields,value.args.operation.args,'O_')};
    if(state.pr==='keyword')return fieldValues(viewVariant(state).fields,value.args.args,'K_');
    if(scalar(t))return {VALUE:value};
    if(t.vec)return Object.fromEntries(value.map((v,i)=>[`ITEM_${i}`,v]));
    if(t.option)return value===null?{}:{SOME:value};
    const d=descriptor(t),v=variantFor(t,value),f=t.tuple?{kind:'tuple',items:t.tuple}:v?v.fields:d.fields;
    return choiceView.values(state,value,sections.values(state,value,fieldValues(f,v?(d.adjacent?value.args:value[v.wire]):value,f.kind==='tuple'?'':'F_')));
  }
  function chainState(ty,values,outline) {
    let head=null,last=null;
    for(const value of values) {
      const next=toBlock(ty,value,outline);next.extraState.statement=true;
      if(ty.named==='Program')next.extraState.flowOption=true;
      if(last)last.next={block:next};else head=next;last=next;
    }
    return head;
  }
  function toBlock(t,value,outline=false,override={}) {
    const peeled=projections.peel(t,value);if(peeled)return toBlock(peeled.ty,peeled.value,outline,override);
    const state=stateFor(t,value),values=childValues(t,value,state);
    state.inline={};
    for(const p of parts(state)) {
      const v=values[p.key];
      if(p.ty.option?.tuple?.length===2&&p.ty.option.tuple.every(t=>t.named==='i32')&&Array.isArray(v))state.inline[p.key]='pt';
      else if(p.ty.named==='Expr' && v?.kind==='Constant')state.inline[p.key]='constant';
      else if(choices[p.ty.named]?.some(([,c])=>same(c,v)))state.inline[p.key]='choice';
    }
    Object.assign(state,override);
    const result={type:'ir_value',extraState:state,fields:{},inputs:{}};
    for(const p of parts(state)) {
      const v=values[p.key];if(v===undefined)throw Error(`Missing ${typeName(t)}.${p.label}`);
      if(p.inline==='pt'){result.fields[p.key+'_P']=String(v[0]);result.fields[p.key+'_T']=String(v[1]);}
      else if(p.inline==='constant')result.fields[p.key]=String(v.args);
      else if(p.inline==='choice')result.fields[p.key]=String(choices[p.ty.named].findIndex(([,c])=>same(c,v)));
      else if(p.inline==='unpreventable')result.fields[p.key]=fieldValue(p.ty,!v);
      else if(scalar(p.ty))result.fields[p.key]=fieldValue(p.ty,v);
      else if(p.flow) {const chain=chainState(p.socketTy,p.flow==='program'?v.instructions:v,outline);if(chain)result.inputs[p.key]={block:chain};}
      else {
        const child=toBlock(p.ty,v,outline);
        if(p.statement){child.extraState.statement=true;result.inputs[p.key]={block:child};}
        else if(p.ty.named==='Expr'||choices[p.ty.named]) {
          const literal=child.extraState.pr==='literal';
          const fallback=p.ty.named==='Expr'?{kind:'Constant',args:0}:clone(choices[p.ty.named][0][1]);
          result.inputs[p.key]=literal?{shadow:child}:{shadow:toBlock(p.ty,fallback),block:child};
        } else result.inputs[p.key]={block:child};
      }
    }
    const variant=viewVariant(state);
    if(variant && !scalar(t)&&!state.activationName&&!['literal','count','query'].includes(state.pr)&&!(state.pr==='effect'&&['Keyword','KeywordEach'].includes(state.op)))result.fields.VARIANT=variant.wire;
    if(selectorEditor?.supports(t)&&state.pr!=='literal')result.collapsed=true;
    if((outline||['Characteristics','Cost'].includes(t.named)) && ['Characteristics','Cost','EntrySpec','MovementTerms','BattlefieldArrival','ExileArrival','CardArrival'].includes(t.named))result.collapsed=true;
    return result;
  }
  function colour(t,state={},block=null) {
    const n=typeName(t);
    if(state.pr==='effect'||n==='Operation')return authoring.actions[state.op||state.variant]?.control?palette.flow:palette.effects;
    if(/^(Instruction|Program)$/.test(n))return palette.flow;
    if(/^(KeywordAction|ObjectKeywordAction|CopyException)$/.test(n))return palette.effects;
    if(/^Mode/.test(n))return palette.choices;
    if(n==='Announcement')return palette.choices;
    if(/Ability|Keyword|Casting|Cost/.test(n))return palette.abilities;
    if(/Card|Characteristics|ManaSymbol/.test(n))return palette.properties;
    if(/Event/.test(n))return palette.events;
    if(/Object|Target|Recipient|Filter|Selection/.test(n))return palette.targets;
    if(/Expr|NumberTest|Condition|Player|Duration|Count|ResultId|LoopBinding|Property|Information|Rounding|Comparison|Aggregation|i32|u32|i64/.test(n))return palette.values;
    return palette.advanced;
  }
  const icon = s => 'data:image/svg+xml;charset=utf-8,'+encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" width="22" height="22"><rect x="1" y="1" width="20" height="20" rx="5" fill="white" fill-opacity=".22"/><text x="11" y="16" text-anchor="middle" fill="white" font-size="19">${s}</text></svg>`);
  function button(symbol,label,fn) { return new Blockly.FieldImage(icon(symbol),22,22,label,fn); }
  let workspace, suppress=false, revision=0, timer, lastValid=null, lastSubmittedIR=null, workerReady=false, preserveSavedDraft=false;
  let history=[],historyIndex=-1;
  let detailMode=false,codeDirty=false,lastHuman=null,bindingEditor=null,propertyEditor=null,costEditor=null,selectorEditor=null;
  function scalarType(p){return p.inline==='pt'?{named:'i32'}:p.inline==='constant'?{named:'i64'}:p.inline==='choice'?{named:'String'}:scalar(p.ty)?p.ty:null;}
  function pickerValue(b,p,index) {const choice=choices[p.ty.named]?.[index];if(!choice)throw Error('Choose a valid '+p.label);const pick=b.state.picks?.[p.key];return clone(pick?.index===index?pick.value:choice[1]);}
  function fakeBlock(s) {return {type:s.type,state:s.extraState,isEnabled:()=>!s.disabled&&!s.disabledReasons?.length,getFieldValue:n=>s.fields?.[n],setFieldValue:(v,n)=>{(s.fields??={})[n]=v;},getInputTargetBlock:n=>{const child=s.inputs?.[n]?.block||s.inputs?.[n]?.shadow;return child?fakeBlock(child):null;},getNextBlock:()=>s.next?.block?fakeBlock(s.next.block):null};}
  function valueField(p,value){if(p.ty.named==='Comparison')return new CardDropdown([['=','Equal'],['≠','NotEqual'],['<','Less'],['≤','LessOrEqual'],['>','Greater'],['≥','GreaterOrEqual'],['same parity','SameParity'],['different parity','DifferentParity']]);if(p.options)return new CardDropdown(p.options);return p.inline==='choice'?new CardDropdown(choices[p.ty.named].map(([label],i)=>[label,String(i)])):field(scalarType(p),value);}
  function isMetadataEmpty(v) {
    if(v===null || v===false || v==='DestinationDefault')return true;
    if(Array.isArray(v))return v.length===0;
    return !!v && typeof v==='object' && Object.values(v).every(isMetadataEmpty);
  }
  const defaultStructures=new Set(['Announcement','TriggerLimits','ActivationLimits','EntrySpec','MovementTerms','BattlefieldArrival','ExileArrival','CardArrival','ModeCount','ModeRules','ModeMechanism']);
  function resetValue(b,p) {
    const valueReset=valueView.reset(b.state,p);if(valueReset!==undefined)return valueReset;

    if(b.state.ty.named==='Instruction'&&b.state.variant==='ChooseOne'&&p.key==='F_options')return undefined;
    if((b.state.pr==='announcement'&&p.key==='F_targets')||(b.state.pr==='modes'&&p.key==='F_options'))return undefined;
    if(b.state.pr==='announcement'&&p.key==='F_modes')return null;
    if(p.abilitySection||p.key==='RESULT'||['ROOT_schema','ROOT_layout','F_schema_version','F_slot'].includes(p.key))return undefined;
    const owner=b.state.pr==='single'?(p.key.startsWith('ROOT_')?'UnifiedCard':'Card'):b.state.pr==='effect'?`Operation::${b.state.op}`:b.state.pr==='keyword'?`Keyword::${b.state.keyword}`:b.state.variant?`${b.state.ty.named}::${b.state.variant}`:b.state.ty.named;
    const key=p.key.replace(/^(ROOT_|F_|O_|K_)/,'');
    if(authoring.defaults[owner]?.[key]!==undefined)return clone(authoring.defaults[owner][key]);
    if(p.ty.option)return null;
    if(p.ty.vec)return [];
    if(defaultStructures.has(p.ty.named))return defaultValue(p.ty);
    if(p.flow&&p.label==='otherwise')return {instructions:[]};
    return undefined;
  }
  function removable(b,p) {
    const reset=resetValue(b,p);if(reset===undefined)return false;
    if(b.state.shown?.includes(p.key))return true;
    try{return !same(childValues(b.state.ty,fromBlock(b),b.state)[p.key],reset);}catch(_){return true;}
  }
  function removeOption(b,key) {
    if(b.isInFlyout)return;
    if(b.state.pr==='announcement'&&key==='F_modes'){choiceView.removeModes(b);return;}
    const p=parts(b.state).find(p=>p.key===key);if(p?.ty.named==='Announcement'){choiceView.clearAnnouncement(b,key);return;}const reset=p&&resetValue(b,p);if(reset===undefined)return;
    const values=childValues(b.state.ty,fromBlock(b),b.state);values[key]=reset;
    replaceView(b,{shown:(b.state.shown||[]).filter(k=>k!==key)},assemble(b.state,values));
  }
  function hiddenReason(b,p) {
    if(!detailMode&&b.state.ty.named==='Ability'&&b.state.variant==='Activated'&&p.key==='F_origin')return true;
    if(b.state.ty.named==='Ability'&&b.state.variant==='Scoped'&&p.key==='F_zones'&&sections.home(b))return true;
    if(b.state.shown?.includes(p.key))return false;
    if(p.key==='RESULT'||(b.state.ty.named==='Instruction'&&b.state.variant==='Effect'&&p.key==='F_result'))return true;
    const choiceHidden=choiceView.hidden(b,p);if(choiceHidden!==null)return choiceHidden;
    if(detailMode)return false;
    const valueReset=valueView.reset(b.state,p);if(valueReset!==undefined)try{return same(fromBlock(b.getInputTargetBlock(p.key)),valueReset);}catch(_){}
    if(b.state.pr==='effect'&&b.state.op==='Counter'&&p.key==='O_destination')try{return same(fromBlock(b.getInputTargetBlock(p.key)),resetValue(b,p));}catch(_){}
    if(b.state.ty.named==='Ability'&&b.state.variant==='Activated'){
      if(p.key==='F_origin')return true;
      if(!b.state.showNamedRules)try{if(namedAbilities.standard(fromBlock(b))?.hidden.includes(p.key))return true;}catch(_){}
    }
    if(b.state.ty.named==='Ability'&&b.state.variant==='Scoped'&&p.key==='F_zones'&&sections.get(sections.home(b)?.id)?.zone)return true;
    if(b.state.ty.named==='Ability'&&b.state.variant==='Static'&&p.key==='F_kind')try{if(fromBlock(b.getInputTargetBlock(p.key))==='Ordinary')return true;}catch(_){}
    if(bindingEditor&&!bindingEditor.showNumeric(b,p))return true;
    if(b.state.ty.named==='TargetSpec'&&['F_kind','F_filter','F_players'].includes(p.key)) {
      try{if(targetPreset(fromBlock(b))>=0)return true;}catch(_){}
    }
    if(['ROOT_schema','ROOT_layout','F_schema_version','RESULT'].includes(p.key))return true;
    if(b.state.ty.named==='TargetSpec'&&p.key==='F_slot')return true;
    if(p.flow)return p.label==='otherwise'&&!b.getInputTargetBlock(p.key);
    const owner=b.state.pr==='single'?(p.key.startsWith('ROOT_')?'UnifiedCard':'Card'):b.state.pr==='effect'?`Operation::${b.state.op}`:b.state.pr==='keyword'?`Keyword::${b.state.keyword}`:b.state.variant?`${b.state.ty.named}::${b.state.variant}`:b.state.ty.named;
    const key=p.key.replace(/^(ROOT_|F_|O_|K_)/,''),defined=authoring.defaults[owner]?.[key];
    if(defined!==undefined)try {
      let value;
      if(p.inline==='constant')value={kind:'Constant',args:readScalar({named:'i64'},b.getFieldValue('_'+p.key))};
      else if(p.inline==='choice')value=choices[p.ty.named][Number(b.getFieldValue('_'+p.key))][1];
      else if(scalar(p.ty)){value=readScalar(p.ty,b.getFieldValue('_'+p.key));if(p.inline==='unpreventable')value=!value;}
      else if(!p.inline)value=fromBlock(b.getInputTargetBlock(p.key));
      if(value!==undefined)return same(value,defined);
    }catch(_){}

    const st=scalarType(p);
    if(st) {
      if(st.named==='bool') {
        const value=b.getFieldValue('_'+p.key)==='TRUE';
        return false;
      }
      return false;
    }
    const child=b.getInputTargetBlock(p.key);if(!child)return false;
    if(b.state.ty.named==='TargetSpec'&&['F_kind','F_count'].includes(p.key)){try{if(same(fromBlock(child),defaultValue({named:'TargetSpec'})[p.key.slice(2)]))return true;}catch(_){}}
    if(child.state.ty.vec)return child.state.count===0;
    if(child.state.ty.option)return !child.state.present;
    if(defaultStructures.has(child.state.ty.named)){try{return isMetadataEmpty(fromBlock(child));}catch(_){return false;}}
    return false;
  }
  function functioningOptions(b) {
    try {
      const value=fromBlock(b),fixed=sections.fixed(value);
      return sections.definitions.filter(d=>fixed?d.id===fixed:!!d.zone||d.id===sections.classify(value)).map(d=>[sections.short(d.id),d.id]);
    }catch(_){return sections.definitions.map(d=>[sections.short(d.id),d.id]);}
  }
  function optionsFor(b) {
    const out=[['⋯','NONE']];
    try{const count=choiceView.countOption(b);if(count)out.push(count);}catch(_){}
    if(sections.home(b)&&b.state.ty.named==='Ability')try{if(!sections.fixed(fromBlock(b)))out.push(['Edit functioning locations…','LOCATIONS']);}catch(_){}
    if(b.state.activationName&&!detailMode)out.push([b.state.showNamedRules?'Hide standard rules':'Show rules','NAMED_RULES']);
    if(bindingEditor?.canPromote(b))out.push(['Name result','NAME_RESULT']);
    for(const p of parts(b.state)) {
      if(choiceView.managed(b.state,p))continue;
      if(p.key==='F_body'&&b.state.ty.named==='Ability'&&hiddenReason(b,p))continue;
      if(b.state.variant==='Scoped'&&p.key==='F_zones'&&sections.home(b))continue;
      if(!detailMode&&b.state.ty.named==='Ability'&&b.state.variant==='Activated'&&p.key==='F_origin')continue;
      if(!detailMode&&!b.state.showNamedRules&&b.state.activationName)try{if(namedAbilities.standard(fromBlock(b))?.hidden.includes(p.key))continue;}catch(_){}
      if(!detailMode && (p.key.startsWith('ROOT_')||['F_schema_version','RESULT'].includes(p.key)))continue;
      if(!hiddenReason(b,p)&&removable(b,p))out.push(['Remove '+(p.label||pretty(p.key.replace(/^(F_|O_|K_)/,''))),'REMOVE:'+p.key]);
      if(hiddenReason(b,p))out.push([p.ty.named==='Announcement'?'Add a target':p.ty.vec?'Add '+p.label.replace(/ies$/,'y').replace(/s$/,''):p.flow?'Add otherwise':pretty(p.label||p.key),'SHOW:'+p.key]);
      if(b.state.ui<3 && p.inline && p.inline!=='unpreventable')out.push(['Use a block for '+pretty(p.key.replace(/^(F_|O_|K_)/,'')),'BLOCK:'+p.key]);
    }
    return out;
  }
  function collapsedSummary(b) {
    const name=b.state.ty.named;
    if(selectorEditor?.supports(b.state.ty)){const label=selectorEditor.summary(b);return label?label+' ▸':null;}
    if(!['Characteristics','Announcement','TargetSpec','Cost'].includes(name))return null;
    const v=fromBlock(b);
    if(name==='Cost')return costEditor?.summary(v)||'Cost';
    if(name==='Characteristics')return [...v.supertypes,...v.card_types,...v.subtypes,v.power_toughness?.join('/'),v.loyalty!==null?'Starting loyalty '+(propertyEditor?.loyaltyText(v.loyalty)??'(formula)'):null,propertyEditor?.manaText(v.mana_cost)||''].filter(Boolean).join(' · ')||'Properties';
    if(name==='TargetSpec'){const named=b.state.bindings?.F_slot?.name;return targetSummary(v)+(named?' · '+named:'');}
    return [...v.targets.map(targetSummary),v.modes?'mode choices':''].filter(Boolean).join(' · ')||'No targets';
  }
  function routeAbilities() {
    let moved=false;
    for(const card of workspace.getAllBlocks(false).filter(b=>b.state?.cardSections)) {
      try {
        const properties=fromBlock(card.getInputTargetBlock('F_characteristics'));
        const main=properties.card_types.some(t=>['Instant','Sorcery'].includes(t))?'resolve':'battlefield';
        if(main!==card.state.cardMain){
          card.state.cardMain=main;
          if(!card.state.cardSections.includes(main))card.state.cardSections.unshift(main);
          card.rebuild();moved=true;
        }
      }catch(_){}
      for(const {block,section} of sections.entries(card)) {
        try{const value=fromBlock(block),target=sections.fixed(value)||(section==='resolve'&&sections.classify(value,card.state.cardMain)!=='resolve'?sections.classify(value,card.state.cardMain):null);if(target&&target!==section&&moveAbility(block,target))moved=true;}catch(_){}
      }
    }
    return moved;
  }
  let paletteLayoutRevision=0;
  const paintedFlyoutBlocks=new WeakSet();
  function applyPresentation() {
    if(!workspace)return;
    const all=[...workspace.getAllBlocks(false),...(workspace.getFlyout()?.getWorkspace().getAllBlocks(false)||[])];
    for(const b of all) {
      if(!b.state)continue;
      const blockColour=colour(b.state.ty,b.state,b);if(b.getColour()!==blockColour)b.setColour(blockColour);
      if(b.isCollapsed()) {
        try {
          const summary=collapsedSummary(b);
          // Summary text is presentation, not a card edit. Avoid validation/save loops.
          Blockly.Events.disable();try{if(summary)b.getField('_TEMP_COLLAPSED_FIELD')?.setValue(summary);}finally{Blockly.Events.enable();}
        }catch(_){}
        selectorEditor?.decorate(b);
        continue;
      }
      choiceView.refresh(b);flowView.refresh(b);
      const selector=b.getField('TARGET_PRESET');
      if(selector)try{
        const index=targetPreset(fromBlock(b));
        Blockly.Events.disable();try{selector.setValue(index<0?'CUSTOM':String(index));}finally{Blockly.Events.enable();}
      }catch(_){}
      let child=b,hiddenAncestor=false;
      for(let parent=b.getParent();parent;child=parent,parent=parent.getParent()){
        const input=parent.inputList.find(i=>i.connection?.targetBlock()===child);
        if(input && (!input.isVisible() || parent.isCollapsed())){hiddenAncestor=true;break;}
      }
      if(hiddenAncestor)continue;
      if(b.state.ty.named==='Ability'&&b.state.variant==='Activated')try{
        const info=namedAbilities.standard(fromBlock(b)),name=info?.name||null;
        if((b.state.activationName||null)!==name){b.state.activationName=name;b.rebuild();}
        b.getField('NAMED_ABILITY')?.setValue(name||'Activated ability');
        b.getField('NAMED_HINT_TEXT')?.setValue(info?.hint||'');
      }catch(_){}
      const home=sections.home(b);
      const location=b.getField('LOCATION');
      if(location){location.setVisible(!!home&&b.state.variant!=='Spell');b.getField('LOCATION_LABEL')?.setVisible(!!home&&b.state.variant!=='Spell');if(home){Blockly.Events.disable();try{location.setValue(home.id);}finally{Blockly.Events.enable();}}}
      const from=b.getField('CAST_FROM');if(from){let value;try{value=fromBlock(b);if(value.kind==='Scoped')value=value.args.ability;}catch(_){}const permission=value?.kind==='Casting'?value.args.Permission:null;from.setValue(value?.kind==='Replacement'&&value.args.active==='SourceEntering'?'As this enters · replacement':permission?'From '+(permission.from.Players?.zone||'another zone').toLowerCase():value?.kind==='Mechanic'&&value.args.mechanic==='Disturb'?'From graveyard · transformed':'');}
      b.getField('COST_ZERO')?.setValue(b.state.present&&b.getInputTargetBlock('SOME')?.state.count===0?'0':'');
      let rerender=valueView.refresh(b,detailMode);
      for(const p of parts(b.state)) {
        const input=b.getInput(p.key),hidden=hiddenReason(b,p);
        if(input && input.isVisible()===hidden){input.setVisible(!hidden);rerender=true;}
        b.getField('FILL_'+p.key)?.setVisible(!hidden && (p.flow || !b.getInputTargetBlock(p.key)));
        b.getField('REMOVE_'+p.key)?.setVisible(!hidden&&removable(b,p));
      }
      const menu=b.getField('MORE');if(menu)menu.setVisible(!b.isInFlyout&&optionsFor(b).length>1);
      b.getField('TYPE_LABEL')?.setVisible(detailMode);
      if(rerender && b.rendered)b.render();
    }
    const flyoutWorkspace=workspace.getFlyout()?.getWorkspace();
    // Hidden inputs can keep a new flyout child's fields from receiving their
    // first paint. Paint visible children before their parents are measured.
    for(const b of (flyoutWorkspace?.getAllBlocks(false)||[]).reverse())if(!paintedFlyoutBlocks.has(b)){
      let child=b,visible=true;
      for(let parent=b.getParent();parent;child=parent,parent=parent.getParent())if(parent.isCollapsed()||parent.inputList.some(i=>i.connection?.targetBlock()===child&&!i.isVisible())){visible=false;break;}
      if(visible){b.render();paintedFlyoutBlocks.add(b);}
    }
    const layoutRevision=++paletteLayoutRevision;
    return Blockly.renderManagement.finishQueuedRenders().then(()=>{
      if(layoutRevision!==paletteLayoutRevision||workspace.isDragging())return;
      const flyout=workspace.getFlyout();if(!flyout?.isVisible())return;
      // Blockly measures the initial, expanded blocks before optional fields
      // hide. Its reflow updates width but leaves vertical positions intact.
      Blockly.Events.disable();try{
        let y=flyout.MARGIN;
        for(const item of flyout.getContents()){
          const element=item.getElement(),rect=element.getBoundingRectangle(),dy=y-rect.top;
          if(Math.abs(dy)>.01)element.moveBy(0,dy);
          y+=rect.getHeight();
        }
        flyout.reflow();flyout.getWorkspace().resizeContents();
      }finally{Blockly.Events.enable();}
    });
  }
  function replaceView(b,patch,value=fromBlock(b)) {
    const old=Blockly.serialization.blocks.save(b),next=old.next;
    const fresh=blockState(b.state.ty,value,b.isCollapsed(),{...b.state,...patch});
    bindingEditor?.transfer(b,fakeBlock(fresh));
    fresh.id=b.id;fresh.x=old.x;fresh.y=old.y;if(next)fresh.next=next;
    const saved=Blockly.serialization.workspaces.save(workspace);
    function find(holder) {
      if(holder.block?.id===b.id){holder.block={...fresh};return true;}
      const n=holder.block;if(!n)return false;
      return Object.values(n.inputs||{}).some(find)||(n.next?find(n.next):false);
    }
    for(let i=0;i<saved.blocks.blocks.length;i++){
      if(saved.blocks.blocks[i].id===b.id){saved.blocks.blocks[i]=fresh;break;}
      const n=saved.blocks.blocks[i];if(Object.values(n.inputs||{}).some(find)||(n.next&&find(n.next)))break;
    }
    remember();install(saved,false);
  }
  function revealOption(b,key) {
    if(b.isInFlyout)return;
    const p=parts(b.state).find(p=>p.key===key);if(!p)return;
    if(b.state.pr==='announcement'&&key==='F_modes'){choiceView.addModes(b);return;}
    if(p.flow&&p.socketTy?.named==='TargetSpec'){choiceView.addTarget(b);return;}
    remember();
    b.state.shown=[...new Set([...(b.state.shown||[]),key])];
    const child=b.getInputTargetBlock(key);
    child?.setCollapsed(false);
    if(p.ty.named==='Announcement' && child)choiceView.addTarget(child);
    else if(child?.state.ty.vec)child.resize(1);
    else if(child?.state.ty.option && !child.state.present)child.resize(0);
    else if(scalarType(p)?.named==='bool')b.setFieldValue(b.getFieldValue('_'+key)==='TRUE'?'FALSE':'TRUE','_'+key);
    applyPresentation();b.render();changed();
  }
  function addSection(b,id) {
    if(b.isInFlyout||!sections.get(id)||b.state.cardSections.includes(id))return;
    remember();b.state.cardSections.push(id);b.rebuild();applyPresentation();changed();
  }
  function removeSection(b,id) {
    if(b.isInFlyout)return;
    remember();
    b.getInputTargetBlock('SECTION_'+id)?.dispose(false);
    if(id!==b.state.cardMain)b.state.cardSections=b.state.cardSections.filter(section=>section!==id);
    b.rebuild();applyPresentation();changed();
  }
  function setFunctionZones(b,zones) {
    if(b.state.variant==='Scoped'&&!b.state.sourceScope){
      b.getInputTargetBlock('F_zones')?.dispose(false);
      const list=Blockly.serialization.blocks.append(blockState({vec:{named:'EventZone'}},zones),workspace);
      b.getInput('F_zones').connection.connect(list.outputConnection);
    }else b.state.sourceScope=clone(zones);
  }
  function editLocations(b) {
    const home=sections.home(b);if(!home)return;
    const value=sections.wire(fromBlock(b),home.id,home.parent.state.cardMain);
    const zones=value.kind==='Scoped'?value.args.zones:[sections.get(home.id).zone].filter(Boolean);
    Blockly.dialog.prompt('Functioning locations, separated by commas: Battlefield, Hand, Graveyard, Stack, Exile, Library, Command',zones.join(', '),text=>{
      if(text===null)return;
      const allowed=sections.definitions.map(d=>d.zone).filter(Boolean),wanted=text.split(',').map(x=>allowed.find(z=>z.toLowerCase()===x.trim().toLowerCase()));
      if(!wanted.length||wanted.some(z=>!z)||new Set(wanted).size!==wanted.length){notice('Choose one or more distinct locations from the list.');return;}
      remember();suppress=true;
      try{setFunctionZones(b,wanted);const target=sections.classify(fromBlock(b),home.parent.state.cardMain);moveAbility(b,target);}finally{suppress=false;}
      changed();
    });
  }
  function moveAbility(b,id) {
    const home=sections.home(b);if(!home||home.id===id||!sections.get(id))return false;
    try{sections.wire(fromBlock(b),id,home.parent.state.cardMain);}catch(e){notice(e.message);applyPresentation();return false;}
    if(!suppress)remember();
    const zone=sections.get(id).zone;
    if(zone&&(b.state.sourceScope||b.state.variant==='Scoped'))setFunctionZones(b,[zone]);
    if(!home.parent.state.cardSections.includes(id)){home.parent.state.cardSections.push(id);home.parent.rebuild();}
    b.unplug(true);let last=home.parent.getInputTargetBlock('SECTION_'+id);
    if(last){while(last.getNextBlock())last=last.getNextBlock();last.nextConnection.connect(b.previousConnection);}
    else home.parent.getInput('SECTION_'+id).connection.connect(b.previousConnection);
    changed();return true;
  }
  function sectionStarter(id) {
    if(id==='resolve')return abilityValue('Spell');
    if(id==='casting')return {kind:'Casting',args:{AdditionalCost:{cost:{parts:[{Mana:[{Generic:1}]}]},optional:true,repetition:'Once'}}};
    if(id==='everywhere')return {kind:'Static',args:{kind:{CharacteristicDefining:{keyword:null,contributes_to_color_identity:true}},affected:{Objects:{kind:'One',args:{kind:'Source'}}},changes:[{kind:'SetBasePowerToughness',args:{power:{kind:'Constant',args:1},toughness:{kind:'Constant',args:1}}}]}};
    if(id==='pregame')return {kind:'OpeningHand',args:{announcement:{targets:[],modes:null},body:{instructions:[]}}};
    if(id==='other')return {kind:'Scoped',args:{zones:['Battlefield','Graveyard'],condition:null,ability:abilityValue('Activated')}};
    return abilityValue('Activated');
  }
  function addToFlow(parent,p,value) {
    if(parent.isInFlyout)return;
    const t=p.socketTy,state=blockState(t,value??(p.abilitySection?sectionStarter(p.abilitySection):defaultValue(t)));state.extraState.statement=true;if(t.named==='Program')state.extraState.flowOption=true;if(state.extraState.pr==='effect')state.extraState.autoResult=true;
    const b=Blockly.serialization.blocks.append(state,workspace);
    let last=parent.getInputTargetBlock(p.key);
    if(!last)parent.getInput(p.key).connection.connect(b.previousConnection);
    else {while(last.getNextBlock())last=last.getNextBlock();last.nextConnection.connect(b.previousConnection);}
    b.select();changed();
  }
  function insertDefault(parent,p) {
    if(parent.isInFlyout)return;
    try {
      if(p.abilitySection==='resolve') {
        for(let ability=parent.getInputTargetBlock(p.key);ability;ability=ability.getNextBlock())if(ability.state.variant==='Spell'){
          addToFlow(ability,parts(ability.state).find(p=>p.key==='F_body'),actionValue('Draw',{players:{kind:'One',args:SELF},amount:{kind:'Constant',args:1}}));return;
        }
      }
      if(p.flow){if(p.socketTy?.named==='TargetSpec')choiceView.addTarget(parent);else if(p.socketTy?.named==='Mode')choiceView.addMode(parent);else addToFlow(parent,p);return;}
      const value=valueView.defaultItem(parent.state)??defaultValue(p.ty);
      if(p.ty.named==='TargetSpec'&&parent.state.ty.vec?.named==='TargetSpec'){const slots=parent.getChildren(false).filter(b=>b.state.ty.named==='TargetSpec').map(b=>Number(b.getFieldValue('_F_slot')));value.slot=slots.length?Math.max(...slots)+1:0;}
      const state=blockState(p.ty,value);if(p.statement)state.extraState.statement=true;
      const b=Blockly.serialization.blocks.append(state,workspace);
      parent.getInput(p.key).connection.connect(p.statement?b.previousConnection:b.outputConnection);b.select();
    }catch(e){notice(e.message);}
  }
  function caption(state) {
    if(state.pr==='single')return 'Card';
    if(state.ty.named==='Characteristics')return 'Properties';
    if(state.ty.named==='FaceDownAs')return 'Face-down creature';
    if(state.ty.named==='TargetSpec')return 'Target';
    if(state.ty.named==='Announcement')return 'Targets & modes';
    return ({TargetSlot:'Target',ResultId:'Recorded result',LoopBinding:'Loop value',SelectionId:'Selection'})[state.ty.named]||typeName(state.ty);
  }
  Blockly.Blocks.ir_value={
    init() {
      const baseToString=this.toString;
      this.toString=function(...args){try{const summary=collapsedSummary(this);if(summary)return summary;}catch(_){}return baseToString.apply(this,args);};
      this.state={ty:{named:'String'}};this.setOutput(true,typeKey(this.state.ty));this.setInputsInline(false);
      this.appendDummyInput('HEADER').appendField('Value');
      this.setOnChange(e=>{
        if(e.type===Blockly.Events.BLOCK_CHANGE && e.blockId===this.id && e.name==='TARGET_PRESET') {
          const selected=e.newValue;
          setTimeout(()=>{if(this.workspace?.getBlockById(this.id))chooseTarget(this,selected);},0);
        }
        if(e.type===Blockly.Events.BLOCK_CHANGE && e.blockId===this.id && e.name==='VARIANT') {
          const v=this.getFieldValue('VARIANT'),key=this.state.pr==='effect'?'op':this.state.pr==='keyword'?'keyword':'variant';
          if(v && v!==this.state[key]){
            if(this.isInFlyout){chooseFlyoutVariant(this,v);return;}
            const destination=['MovementDestination','ConjureDestination','LibraryPosition'].includes(this.state.ty.named);
            // Empty arrivals are structural defaults. Keep nonempty children
            // recoverable if the new zone cannot accept their type.
            const emptyArrivals=destination?this.getChildren(false).filter(b=>{
              if(!['BattlefieldArrival','ExileArrival','CardArrival'].includes(b.state?.ty.named))return false;
              try{return same(fromBlock(b),defaultValue(b.state.ty));}catch(_){return false;}
            }):[];
            this.state[key]=v;this.rebuild();
            if(destination){
              for(const b of emptyArrivals)if(!b.getParent())b.dispose(false);
              for(const p of parts(this.state))if(!scalarType(p)&&!p.flow&&!this.getInputTargetBlock(p.key))insertDefault(this,p);
            }
          }
        }
      });
    },
    saveExtraState(){return clone(this.state);},
    loadExtraState(state) {
      if(state?.valueList!==undefined&&(!Number.isInteger(state.valueList)||state.valueList<0||state.valueList>2048))throw Error('Invalid value list or a list longer than 2,048 items.');
      if(!state?.ty || !Number.isInteger(state.count??0) || (state.count??0)<0 || (state.count??0)>2048)throw Error('Invalid block state or a list longer than 2,048 items.');
      this.state=clone(state);this.rebuild();
    },
    rebuild() {
      const attached={},shadows={},values={};
      Blockly.Events.disable();
      try {
        for(const input of this.inputList){
          const child=input.connection?.targetBlock();
          if(input.connection)shadows[input.name]=child?input.connection.stashShadowState().shadowState:input.connection.getShadowState();
          if(child){
            if(child.isShadow()){
              shadows[input.name]=Blockly.serialization.blocks.save(child,{addCoordinates:false,addInputBlocks:true,addNextBlocks:false});
              child.dispose(false);
            }else{attached[input.name]=child;child.unplug(false);}
          }
          for(const f of input.fieldRow)if(f.name)values[f.name]=f.getValue();
        }
        for(const i of [...this.inputList])this.removeInput(i.name);
        const t=this.state.ty,d=descriptor(t),header=this.state.pr==='literal'?null:this.appendDummyInput('HEADER'),variant=viewVariant(this.state);
        if(this.state.statement){this.setOutput(false);this.setPreviousStatement(true,projections.outputs(t).map(typeKey));this.setNextStatement(['Instruction','Ability','TargetSpec','Mode','Program','Operation'].includes(t.named),typeKey(t));}
        else {this.setNextStatement(false);this.setPreviousStatement(false);this.setOutput(true,projections.outputs(t).map(typeKey));}
        this.setColour(colour(t,this.state,this));
        if(this.state.flowOption)header.appendField('Option','OPTION_TITLE').appendField(button('−','Remove this option',()=>choiceView.removeRow(this)),'REMOVE_OPTION');
        else if(this.state.pr==='announcement')header.appendField('Targets','CHOICES_TITLE').appendField(button('+','Add target group',()=>choiceView.addTarget(this)),'ADD_TARGET').appendField(button('≡','Add modes',()=>choiceView.addModes(this)),'ADD_MODES');
        else if(this.state.pr==='modes'){
          header.appendField(this.state.impliedModes?'Required':'Choose');
          if(this.state.simpleModeCount)header.appendField(new CardDropdown([['exactly','Exactly'],['up to','UpTo'],['between','Between']]),'_M_STYLE').appendField(new Blockly.FieldTextInput(values._M_MIN||'1'),'_M_MIN').appendField('and','MODE_AND').appendField(new Blockly.FieldTextInput(values._M_MAX||'1'),'_M_MAX').appendField('modes');
          else header.appendField('modes');
          header.appendField(button('+','Add mode',()=>choiceView.addMode(this)),'ADD_MODE');
        }
        else if(this.state.pr==='mode')header.appendField('Mode','MODE_TITLE').appendField(button('−','Remove this mode',()=>choiceView.removeRow(this)),'REMOVE_MODE');
        else if(this.state.pr==='count')header.appendField('number of');
        else if(this.state.pr==='query')header.appendField('all');
        else if(this.state.activationName)header.appendField(this.state.activationName,'NAMED_ABILITY');
        else if(this.state.pr==='literal') { /* The editable field itself is the literal block. */ }
        else if(t.named==='TargetSpec'){
          header.appendField('Choose');
          if(this.state.simpleTargetCount)header.appendField(new CardDropdown([['exactly','Exactly'],['up to','UpTo'],['any number','AnyNumber'],['at least','AtLeast']]),'_T_STYLE').appendField(new Blockly.FieldTextInput(values._T_NUMBER||'1'),'_T_NUMBER');
          header.appendField(new CardDropdown([...targetPresets.map(([label],i)=>[label,String(i)]),['custom target…','CUSTOM']]),'TARGET_PRESET').appendField(button('−','Remove this target group',()=>choiceView.removeRow(this)),'REMOVE_TARGET');
        }
        else if(this.state.valueList!==undefined){header.appendField(new CardDropdown(variantChoices(this.state,d).map(v=>[variantLabel(t.named,v),v.wire])),'VARIANT').appendField(button('+','Add another value or condition',()=>this.resize(1)),'ADD_VALUE').appendField(button('−','Remove the last value or condition',()=>this.resize(-1)),'REMOVE_VALUE').appendField('empty list','EMPTY_VALUES');this.setFieldValue(variant.wire,'VARIANT');}
        else if(t.vec){header.appendField(t.vec.named==='TargetSpec'?'Targets':typeName(t.vec)+' items');header.appendField(button('+','Add an item',()=>this.resize(1)));if(this.state.count)header.appendField(button('−','Remove last item',()=>this.resize(-1)));}
        else if(t.option){header.appendField(t.option.vec?.named==='ManaSymbol'?(this.state.present?'Mana cost':'No mana cost'):(this.state.present?'Value':'Not set'));if(t.option.vec?.named==='ManaSymbol')header.appendField('','COST_ZERO');header.appendField(button(this.state.present?'−':'+','Toggle optional value',()=>this.resize(0)));}
        else if(this.state.pr==='effect'&&['Keyword','KeywordEach'].includes(this.state.op))header.appendField(this.state.op==='Keyword'?'Keyword action':'Keyword action on objects');
        else if(variant && !scalar(t)) {
          const vd=this.state.pr==='effect'?types.Operation:this.state.pr==='keyword'?types.Keyword:d;
          if(t.named==='EventSpec')header.appendField('When');
          header.appendField(new CardDropdown(t.named==='Ability'&&variant.wire==='Spell'?[['When this spell resolves','Spell']]:variantChoices(this.state,vd).map(v=>[variantLabel(this.state.pr==='effect'?'Operation':t.named,v),v.wire])),'VARIANT');this.setFieldValue(variant.wire,'VARIANT');
          header.appendField('· '+t.named,'TYPE_LABEL');
        }else header.appendField(caption(this.state));
        if(header&&t.named==='Instruction'&&this.state.variant==='ChooseOne')header.appendField(button('+','Add option',()=>flowView.addOption(this)),'ADD_OPTION');
        if(header&&selectorEditor?.supports(t))header.appendField(button('▴','Compact this selector',()=>selectorEditor.compact(this)),'COMPACT_SELECTOR');
        if(header&&this.state.activationName)this.appendDummyInput('NAMED_HINT').appendField('','NAMED_HINT_TEXT');
        if(header&&(t.named==='Mode'||t.named==='Ability'&&['Spell','Activated','Triggered'].includes(this.state.variant)))header.appendField(button('⋄','Edit targets or modes',()=>choiceView.reveal(this)),'EDIT_CHOICES');
        if(header&&t.named==='Ability')header.appendField('Functions:','LOCATION_LABEL').appendField(new CardDropdown(()=>functioningOptions(this),v=>{if(Blockly.Events.isEnabled()&&sections.home(this)&&sections.home(this).id!==v)setTimeout(()=>moveAbility(this,v),0);return v;}),'LOCATION').appendField('','CAST_FROM');
        if(header&&this.state.cardSections)header.appendField(new CardDropdown(()=>[['Add another section…','NONE'],...sections.definitions.filter(d=>!this.state.cardSections.includes(d.id)).map(d=>[d.label,d.id])],v=>{if(v!=='NONE')setTimeout(()=>addSection(this,v),0);return null;}),'ADD_SECTION');
        if(header)header.appendField(new CardDropdown(()=>optionsFor(this),v=>{
          if(v==='NONE')return null;
          setTimeout(()=>{if(v==='COUNT_BLOCKS'||v==='COUNT_SIMPLE')choiceView.editCount(this,v==='COUNT_SIMPLE');else if(v==='NAMED_RULES'){remember();this.state.showNamedRules=!this.state.showNamedRules;applyPresentation();changed();}else if(v==='NAME_RESULT')bindingEditor.promote(this);else if(v==='LOCATIONS')editLocations(this);else if(v.startsWith('SHOW:'))revealOption(this,v.slice(5));else if(v.startsWith('REMOVE:'))removeOption(this,v.slice(7));else if(v.startsWith('BLOCK:'))replaceView(this,{noInline:[...(this.state.noInline||[]),v.slice(6)]});},0);return null;
        }),'MORE');
        for(const p of parts(this.state)) {
          if(choiceView.managed(this.state,p))continue;
          if(p.inline==='pt') {
            const input=this.appendDummyInput(p.key).appendField('power / toughness');
            for(const [i,suffix] of ['P','T'].entries()){if(i)input.appendField('/');input.appendField(new Blockly.FieldTextInput(values['_'+p.key+'_'+suffix]||'0'),'_'+p.key+'_'+suffix);}
          }else if(scalarType(p)) {
            const v=values['_'+p.key]??(p.inline==='choice'?'0':fieldValue(scalarType(p),defaultValue(scalarType(p))));
            const input=this.appendDummyInput(p.key);if(p.label)input.appendField(p.label);input.appendField(valueField(p,v),'_'+p.key);this.getField('_'+p.key).setValue(v);if(p.suffix)input.appendField(p.suffix);
          }else {
            const ty=p.socketTy||p.ty;
            const input=(p.statement?this.appendStatementInput(p.key):this.appendValueInput(p.key)).setCheck(typeKey(ty));
            if(p.abilitySection){
              const title=this.appendDummyInput('TITLE_'+p.key).appendField(p.label)
                .appendField(button('+',p.abilitySection==='resolve'?'Add a spell effect':'Add an ability here',()=>insertDefault(this,p)),'FILL_'+p.key)
                .appendField(button('−','Clear this section (undo restores it)',()=>removeSection(this,p.abilitySection)));
              this.moveInputBefore(title.name,p.key);
            }else {
              input.appendField(p.label);
              if(['Characteristics','Cost'].includes(p.ty.named))input.appendField(new Blockly.FieldImage('data:image/svg+xml;charset=utf-8,'+encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="56" height="24"><rect width="56" height="24" rx="5" fill="white" fill-opacity=".25"/><text x="28" y="16" text-anchor="middle" fill="white" font-family="sans-serif" font-size="12">Edit…</text></svg>'),56,24,p.ty.named==='Cost'?'Edit cost':'Edit card properties',()=>p.ty.named==='Cost'?costEditor.open(this.getInputTargetBlock(p.key)):propertyEditor.open(this.getInputTargetBlock(p.key))),'EDIT_PROPERTIES');
              else if(p.flow&&['TargetSpec','Mode','Program'].includes(p.socketTy?.named)){ /* Header owns the explicit add-row control. */ }
              else if(selectorEditor?.supports(p.ty)&&!p.statement)input.appendField(new Blockly.FieldImage('data:image/svg+xml;charset=utf-8,'+encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="88" height="24"><rect width="88" height="24" rx="12" fill="#254b65"/><text x="44" y="16" text-anchor="middle" fill="white" font-family="sans-serif" font-size="12">Choose…</text></svg>'),88,24,'Choose '+selectorEditor.title(p.ty,this,p.key),()=>selectorEditor.open(this,p.key)),'SELECT_'+p.key);
              else input.appendField(button('+',p.flow?'Add a block':`Fill ${p.label}`,()=>insertDefault(this,p)),'FILL_'+p.key);
            }
            const shadow=shadows[p.key];
            if(shadow&&projections.compatible(shadow.extraState.ty,ty))this.getInput(p.key).connection.setShadowState(shadow);
            const old=attached[p.key];
            if(old && projections.compatible(old.state.ty,ty))this.getInput(p.key).connection.connect(p.statement?old.previousConnection:old.outputConnection);
          }
        }
        for(const p of parts(this.state))if(resetValue(this,p)!==undefined)this.getInput(p.key)?.appendField(button('−','Remove '+(p.label||'this option')+' (undo restores it)',()=>removeOption(this,p.key)),'REMOVE_'+p.key);
        this.setTooltip(`${typeName(t)}${variant?'::'+variant.name:''}\n${variant?.doc||''}`);
        valueView.decorate(this);
        this.setInputsInline(valueView.compact(this.state)||flowView.compact(this.state)||['literal','effect','count','query','modes'].includes(this.state.pr)||t.named==='TargetSpec'||this.state.pr==='keyword'||scalar(t)||(d?.kind==='struct'&&d.fields.kind==='tuple'&&d.fields.items.every(scalar)));
      }finally{Blockly.Events.enable();}
      if(this.rendered)this.render();
    },
    resize(delta) {
      if(this.isInFlyout)return;
      if((this.state.ty.vec||this.state.valueList!==undefined)&&delta>0&&(this.state.valueList??this.state.count)>=2048){notice('This demo allows up to 2,048 items in one list.');return;}
      Blockly.Events.setGroup(true);
      try {
        if(this.state.valueList!==undefined){if(delta<0)this.getInputTargetBlock(`ITEM_${this.state.valueList-1}`)?.dispose(false);this.state.valueList=Math.max(0,this.state.valueList+delta);}
        else if(this.state.ty.vec){if(delta<0)this.getInputTargetBlock(`ITEM_${this.state.count-1}`)?.dispose(false);this.state.count=Math.max(0,this.state.count+delta);}
        else {if(this.state.present)this.getInputTargetBlock('SOME')?.dispose(false);this.state.present=!this.state.present;}
        this.rebuild();const p=parts(this.state).at(-1);if(p&&!scalarType(p)&&!this.getInputTargetBlock(p.key))insertDefault(this,p);
      }finally{Blockly.Events.setGroup(false);}
      changed();
    }
  };
  function chainValues(head) {
    const values=[],seen=new Set();
    for(let b=head;b;b=b.getNextBlock?.()) {if(seen.has(b))throw Error('A block stack contains a cycle.');seen.add(b);values.push(fromBlock(b));}
    return values;
  }
  function fieldsResult(f,values,prefix='F_') {
    if(f.kind==='unit')return null;
    if(f.kind==='record')return Object.fromEntries(f.fields.map(f=>[f.name,values[prefix+f.name]]));
    const a=f.items.map((_,i)=>values[`${prefix}ARG_${i}`]);return a.length===1?a[0]:a;
  }
  function fromBlock(b,expected=null) {
    if(!b || b.type!=='ir_value')throw Error('Fill the missing block before exporting.');
    if(!b.isEnabled())throw Error('Re-enable or remove disabled blocks before exporting IR.');
    const state=b.state,t=state.ty,d=descriptor(t),values={};
    for(const p of parts(state)) {
      if(p.inline==='pt')values[p.key]=[readScalar({named:'i32'},b.getFieldValue('_'+p.key+'_P')),readScalar({named:'i32'},b.getFieldValue('_'+p.key+'_T'))];
      else if(p.inline==='constant')values[p.key]={kind:'Constant',args:readScalar({named:'i64'},b.getFieldValue('_'+p.key))};
      else if(p.inline==='choice') {
        values[p.key]=pickerValue(b,p,Number(b.getFieldValue('_'+p.key)));
      } else if(p.inline==='unpreventable')values[p.key]=!readScalar(p.ty,b.getFieldValue('_'+p.key));
      else if(scalar(p.ty))values[p.key]=readScalar(p.ty,b.getFieldValue('_'+p.key));
      else if(p.flow) {const list=chainValues(b.getInputTargetBlock(p.key));values[p.key]=p.flow==='program'?{instructions:list}:list;}
      else {
        const child=b.getInputTargetBlock(p.key);if(!child)throw Error(selectorEditor.stackContext(b,p.key)?'Choose a spell or ability to counter.':`${typeName(t)}: fill the ${p.label} socket.`);
        values[p.key]=fromBlock(child,p.ty);
      }
    }
    if(state.cardSections)values.F_abilities=sections.entries(b).map(e=>sections.wire(fromBlock(e.block),e.section,state.cardMain));
    const value=assemble(state,values);return expected?projections.coerce(t,expected,value):value;
  }
  function scopeValue(state,value){return state.sourceScope?{kind:'Scoped',args:{zones:clone(state.sourceScope),condition:null,ability:value}}:value;}
  function assemble(state,values) {
    const t=state.ty,d=descriptor(t);
    if(state.valueList!==undefined)return valueView.listValue(state,values);
    if(state.pr==='literal')return values.VALUE;
    if(state.pr==='count')return {kind:'CountObjects',args:values.COUNTED};
    if(state.pr==='query')return projections.queryValue(state,values);
    if(state.pr==='single')return {schema_version:values.ROOT_schema,layout:values.ROOT_layout,data:values.ROOT_data,faces:[fieldsResult(types.Card.fields,values)]};
    if(state.pr==='effect')return {kind:'Effect',args:{result:values.RESULT,operation:packEnum(types.Operation,viewVariant(state),fieldsResult(viewVariant(state).fields,values,'O_'))}};
    if(state.pr==='keyword')return scopeValue(state,{kind:'Keyword',args:packEnum(types.Keyword,viewVariant(state),fieldsResult(viewVariant(state).fields,values,'K_'))});
    if(scalar(t))return values.VALUE;
    if(t.vec) {
      if(['Instruction','Ability'].includes(t.vec.named))return parts(state).flatMap(p=>values[p.key]);
      return parts(state).map(p=>values[p.key]);
    }
    if(t.option)return state.present?values.SOME:null;
    const v=viewVariant(state),f=t.tuple?{kind:'tuple',items:t.tuple}:v?v.fields:d.fields;
    choiceView.fields(state,values);
    const a=fieldsResult(f,values,f.kind==='tuple'?'':'F_');return scopeValue(state,v?packEnum(d,v,a):a);
  }
  // Blockly serializes field names separately from state; use those exact names.
  function fixFieldNames(s) {
    s.fields=Object.fromEntries(Object.entries(s.fields).map(([k,v])=>[k==='VARIANT'?k:'_'+k,v]));
    Object.values(s.inputs||{}).forEach(i=>{if(i.block)fixFieldNames(i.block);if(i.shadow)fixFieldNames(i.shadow);});if(s.next?.block)fixFieldNames(s.next.block);return s;
  }
  const blockState=(t,v,outline=false,override={})=>fixFieldNames(toBlock(t,v,outline,override));
  function actionValue(name,values={}) {
    const v=types.Operation.variants.find(v=>v.wire===name),args=fieldsDefault(v.fields,[],`Operation::${name}`);
    if(v.fields.kind==='record')Object.assign(args,values);
    else if(Object.keys(values).length)throw Error('This action has no named fields');
    return {kind:'Effect',args:{result:4294967295,operation:packEnum(types.Operation,v,args)}};
  }
  const keywordFamilies=['KeywordAction','ObjectKeywordAction'];
  const modeFamilies=['ModeMechanism','ModeRepetition','ModeAvailability','ModeTargetPolicy','ModeAcceptance'];
  const keywordMeta=(type,v)=>(type==='KeywordAction'?authoring.keyword_actions:authoring.object_keyword_actions)[v.wire];
  function keywordValue(type,v) {
    return actionValue(type==='KeywordAction'?'Keyword':'KeywordEach',{action:packEnum(types[type],v,fieldsDefault(v.fields,[],`${type}::${v.wire}`))});
  }
  function abilityValue(name) {
    const v=types.Ability.variants.find(v=>v.wire===name),args=fieldsDefault(v.fields,[],`Ability::${name}`);
    if(name==='Triggered')args.event=clone(choices.EventSpec[0][1]);
    return packEnum(types.Ability,v,args);
  }
  function starterState(state){
    if(state.extraState.pr==='effect'&&state.extraState.op==='Counter')delete state.inputs.O_objects;
    return state;
  }
  function flyoutBlock(type,value) {
    const state=starterState(blockState({named:type},value));
    if(['Ability','Instruction','CopyException','TargetSpec','Mode'].includes(type))state.extraState.statement=true;
    if(type==='TargetSpec')state.extraState.freshTarget=true;
    if(state.extraState.pr==='effect')state.extraState.autoResult=true;
    return {kind:'block',...state};
  }
  function variantBlock(type,name) {
    const d=types[type],v=d.variants.find(v=>v.wire===name);
    const args=fieldsDefault(v.fields,[],`${type}::${name}`);
    return flyoutBlock(type,type==='Instruction'?flowView.starter(name):valueView.starter(type,name)||packEnum(d,v,args));
  }
  function effectGroup(name) {
    const category=authoring.actions[name]?.category;
    if(['Cards & libraries','Objects & movement','Counters & changes'].includes(category)||name==='Wish')return 'Cards & permanents';
    if(category==='Damage & life'||['AddMana','PayCost','PayAnyAmount','PayRepeatedly','AnyPlayerMayPay','PlayersPayRepeatedly'].includes(name))return 'Life, damage & mana';
    if(category==='Choices & values')return 'Choices & chance';
    return 'Spells & rules';
  }
  function variantChoices(state,descriptor) {
    const values=valueView.variants(state,descriptor);if(values)return values;
    const flow=flowView.variants(state,descriptor);if(flow)return flow;
    if(state.pr==='effect'){
      if(['Keyword','KeywordEach'].includes(state.op))return descriptor.variants.filter(v=>v.wire===state.op);
      const meta=authoring.actions[state.op];
      if(meta&&!meta.control){const options=descriptor.variants.filter(v=>!excluded(v)&&!authoring.actions[v.wire].control&&!['Keyword','KeywordEach'].includes(v.wire)&&effectGroup(v.wire)===effectGroup(state.op));return options.length?options:descriptor.variants.filter(v=>v.wire===state.op);}
    }
    return descriptor.variants;
  }
  function actionGroups() {
    const groups=[['Cards & permanents','Draw'],['Life, damage & mana','GainLife'],['Spells & rules','Counter'],['Choices & chance','FlipCoin']];
    return [
      ...groups.flatMap(([text,name])=>[{kind:'label',text},flyoutBlock('Instruction',actionValue(name,{...(name==='Draw'||name==='GainLife'?{players:{kind:'One',args:SELF},amount:{kind:'Constant',args:1}}:{})}))]),
      {kind:'label',text:'Keyword actions'},
      {kind:'label',text:'Named Magic actions · Investigate creates a Clue token'},
      keywordValue('KeywordAction',types.KeywordAction.variants.find(v=>v.wire==='Investigate')),
      keywordValue('ObjectKeywordAction',types.ObjectKeywordAction.variants.find(v=>v.wire==='Transform'))
    ].map(item=>item.kind==='Effect'?flyoutBlock('Instruction',item):item);
  }
  function controlGroups(){return flowView.palette();}
  selectorEditor=createSelectorEditor({same,clone,fromBlock,parts,blockState,defaultValue,targetSummary,createTarget:(parent,kind)=>{const owner=choiceView.scope(parent);if(!owner)throw Error('Connect this action inside an ability before adding its target.');const announcement=owner.getInputTargetBlock('F_announcement')||owner.getInputTargetBlock('O_announcement');if(!announcement)throw Error('This ability cannot announce a target. Choose a stack query or an advanced selector.');return choiceView.addTarget(announcement,{kind:{Object:{kind,zones:['Stack']}},filter:{kind:'Any'},players:null});},workspace:()=>workspace,remember,changed,applyPresentation,notice});
  class SelectorConnectionPreviewer extends Blockly.InsertionMarkerPreviewer {
    constructor(block){super(block);selectorEditor.beginDrag(block);}
    previewConnection(dragged,connection){super.previewConnection(dragged,connection);selectorEditor.preview(connection,false);}
    previewReplacement(dragged,connection,replaced){super.previewReplacement(dragged,connection,replaced);selectorEditor.preview(connection,true);}
    hidePreview(){super.hidePreview();selectorEditor.clearPreview();}
    dispose(){super.dispose();selectorEditor.endDrag();}
  }
  const toolboxDefinition={kind:'categoryToolbox',contents:[
    {kind:'category',name:'Abilities',colour:palette.abilities,contents:[
      {kind:'label',text:'Add these to a location section'},
      ...['Triggered','Activated','Static','Replacement'].map(n=>flyoutBlock('Ability',abilityValue(n))),
      flyoutBlock('Ability',{kind:'Replacement',args:{active:'SourceEntering',event:{Enter:{objects:{kind:'SameObject',args:{kind:'Source'}}}},body:{instructions:[]}}}),
      {kind:'label',text:'Named abilities · battlefield'},
      flyoutBlock('Ability',namedAbilities.equip()),flyoutBlock('Ability',namedAbilities.crew()),
      {kind:'label',text:'Named abilities · hand'},
      flyoutBlock('Ability',namedAbilities.cycling()),
      {kind:'label',text:'Simple keywords'},
      flyoutBlock('Ability',{kind:'Keyword',args:{kind:'Flying'}})
    ]},
    {kind:'category',name:'Effects',colour:palette.effects,contents:[
      {kind:'label',text:'Drop effects into spell resolution or an ability'},
      ...actionGroups()
    ]},
    {kind:'category',name:'Flow',colour:palette.flow,contents:controlGroups()},
    {kind:'category',name:'Casting & locations',colour:palette.abilities,contents:[
      {kind:'label',text:'Casting rules and costs'},
      flyoutBlock('Ability',sectionStarter('casting')),
      flyoutBlock('Ability',{kind:'Keyword',args:{kind:'Flash'}}),
      flyoutBlock('Ability',{kind:'Casting',args:{Permission:{origin:'Flashback',player:{kind:'OwnerOf',args:{kind:'Source'}},from:{Players:{zone:'Graveyard',players:{kind:'One',args:{kind:'OwnerOf',args:{kind:'Source'}}}}},require:{kind:'AnyOf',args:[{kind:'CardType',args:'Instant'},{kind:'CardType',args:'Sorcery'}]},cost:{parts:[{Mana:[{Generic:1}]}]},leave_stack:'Exile'}}}),
      flyoutBlock('Ability',{kind:'Mechanic',args:{mechanic:'Disturb',cost:{parts:[{Mana:[{Generic:1}]}]},amount:null,filter:null}}),
      {kind:'label',text:'When this card is cast · a separate trigger'},
      flyoutBlock('Ability',{kind:'Scoped',args:{zones:['Stack'],condition:null,ability:{...abilityValue('Triggered'),args:{...abilityValue('Triggered').args,event:{Spell:{event:'Cast',players:{kind:'One',args:SELF},spells:{kind:'SameObject',args:{kind:'Source'}}}}}}}}),
      {kind:'label',text:'In your graveyard · activate or continuously affect'},
      ...['Activated','Static'].map(kind=>flyoutBlock('Ability',{kind:'Scoped',args:{zones:['Graveyard'],condition:null,ability:abilityValue(kind)}})),
      {kind:'label',text:'Define characteristics in every zone'},
      flyoutBlock('Ability',sectionStarter('everywhere'))
    ]},
    {kind:'category',name:'Choices & modes',colour:palette.choices,contents:[
      {kind:'label',text:'Use an ability’s ⋄ button to add modes'},
      {kind:'label',text:'Add a separate mode to its modes section'},
      flyoutBlock('Mode',{announcement:{targets:[],modes:null},body:{instructions:[]},weight:1,requires:null}),
      {kind:'label',text:'Selection method and additional rules'},
      ...types.ModeMechanism.variants.map(v=>variantBlock('ModeMechanism',v.wire)),
      ...['ModeRepetition','ModeAvailability','ModeTargetPolicy','ModeAcceptance'].flatMap(t=>types[t].variants.map(v=>variantBlock(t,v.wire)))
    ]},
    {kind:'category',name:'Events',colour:palette.events,contents:[
      flyoutBlock('EventSpec',choices.EventSpec[0][1])
    ]},
    {kind:'category',name:'Targets & selections',colour:palette.targets,contents:[
      flyoutBlock('TargetSpec',defaultValue({named:'TargetSpec'})),
      ...['Target','Source'].map(n=>variantBlock('ObjectRef',n)),
      variantBlock('ObjectSet','Targets'),
      ...selectorEditor.starters({named:'ObjectSet'},null).filter(o=>o.value.kind==='Zone').map(o=>flyoutBlock('ObjectSet',o.value))
    ]},
    {kind:'category',name:'Values & conditions',colour:palette.values,contents:valueView.palette()},
    {kind:'category',name:'Advanced',colour:palette.advanced,contents:
      [...['Expr','NumberTest','Condition','PlayerRef','PlayerSet','ObjectFilter','ObjectRef','ObjectSet','CostPart','Duration','EventSpec'].map(type=>({kind:'category',name:type,colour:colour({named:type}),contents:types[type].variants.filter(v=>!excluded(v)).flatMap(v=>{try{return [variantBlock(type,v.wire)];}catch(_){return [];}})})),{kind:'category',name:'Specialized flow',colour:palette.flow,contents:[flyoutBlock('Instruction',actionValue('Together'))]},{kind:'category',name:'Copy modifiers',colour:palette.effects,contents:[variantBlock('CopyException',types.CopyException.variants[0].wire)]}]}
  ]};
  const theme=Blockly.Theme.defineTheme('cardBlocks',{base:Blockly.Themes.Classic,fontStyle:{family:'system-ui, sans-serif',size:11},componentStyles:{workspaceBackgroundColour:'#f4f6f1',scrollbarColour:'#a8b5a4',scrollbarOpacity:.6,insertionMarkerColour:'#52794c',insertionMarkerOpacity:.4}});
  workspace=Blockly.inject('workspace',{renderer:'zelos',plugins:{connectionPreviewer:SelectorConnectionPreviewer},theme,toolbox:toolboxDefinition,trashcan:false,sounds:false,zoom:{controls:false,wheel:true,startScale:.75,maxScale:1.6,minScale:.12,scaleSpeed:1.16},move:{scrollbars:true,drag:true,wheel:true},grid:{spacing:22,length:1,colour:'#d6dfd1',snap:false}});
  workspace.getFlyout().getWorkspace().addChangeListener(e=>{if(!e.isUiEvent&&e.type===Blockly.Events.BLOCK_CHANGE)setTimeout(applyPresentation,0);});
  propertyEditor=createPropertyEditor({types,clone,fromBlock,replaceView,workspace:()=>workspace,notice,same,readScalar});
  costEditor=createCostEditor({namedAbilities,clone,same,readScalar,fromBlock,replaceView,workspace:()=>workspace,notice,manaText:propertyEditor.manaText,parseMana:propertyEditor.parseMana,loyaltyText:propertyEditor.loyaltyText,parseLoyalty:propertyEditor.parseLoyalty});
  $('edit-properties').onclick=()=>propertyEditor.open();
  new ResizeObserver(()=>Blockly.svgResize(workspace)).observe($('workspace'));
  function blockSearchKey(block){
    const s=block.extraState;
    if(s.pr==='effect'&&!['Keyword','KeywordEach'].includes(s.op))return 'Operation:'+s.op;
    if(s.ty.named==='EventSpec'&&s.pr==='literal')return 'EventSpec:common';
    return encode(fromBlock(fakeBlock(block)));
  }
  $('block-search').oninput=()=>{
    const q=$('block-search').value.trim().toLocaleLowerCase();
    if(!q){workspace.updateToolbox(toolboxDefinition);return;}
    const contents=[],seen=new Set();
    function visit(items,path=[]) {
      for(const item of items) {
        if(item.kind==='category'){visit(item.contents,[...path,item.name]);continue;}
        if(item.kind!=='block')continue;
        try {
          const state=item.extraState,key=blockSearchKey(item);
          const variant=viewVariant(state),type=state.pr==='effect'?'Operation':state.ty.named;
          const label=(selectorEditor.supports(state.ty)?selectorEditor.summary(fakeBlock(item)):null)||state.activationName|| (variant?variantLabel(type,variant):typeName(state.ty));
          const selectorOptions=(state.ty.named==='TargetSpec'?targetPresets:state.ty.named==='EventSpec'?choices.EventSpec:[]).map(([label])=>label).join(' ');
          if(!`${label} ${selectorOptions} ${variant?.doc||''} ${path.join(' ')} ${key} ${pretty(key)}`.toLocaleLowerCase().includes(q)||seen.has(key))continue;
          seen.add(key);contents.push(item);
        }catch(_){}
      }
    }
    visit(toolboxDefinition.contents);
    for(const v of types.Instruction.variants.filter(v=>v.wire!=='Effect'))try{
      if(!`${v.wire} ${v.name} ${flowView.labels[v.wire]||''} ${authoring.instructions[v.wire]?.label||''} ${pretty(v.name)} ${v.doc||''}`.toLowerCase().includes(q))continue;
      const block=variantBlock('Instruction',v.wire),key=blockSearchKey(block);if(!seen.has(key)){seen.add(key);contents.push(block);}
    }catch(_){}
    // A topic has one effect starter; searches can still start at any named effect.
    for(const v of types.Operation.variants.filter(v=>!excluded(v)&&!['Keyword','KeywordEach'].includes(v.wire)))try{
      const meta=authoring.actions[v.wire];
      if(!`${v.wire} ${v.name} ${uiLabels[v.wire]||flowView.labels[v.wire]||''} ${meta.label} ${pretty(v.name)} ${v.doc||''}`.toLocaleLowerCase().includes(q))continue;
      const block=flyoutBlock('Instruction',actionValue(v.wire)),key=blockSearchKey(block);
      if(!seen.has(key)){seen.add(key);contents.push(block);}
    }catch(_){}
    for(const v of types.CopyException.variants)try{
      if(!`${pretty(v.name)} ${v.doc||''}`.toLocaleLowerCase().includes(q))continue;
      const block=variantBlock('CopyException',v.wire),key=encode(fromBlock(fakeBlock(block)));
      if(!seen.has(key)){seen.add(key);contents.push(block);}
    }catch(_){}
    // The palette uses editable keyword-action families; searches still offer
    // the requested named action with its exact typed operands.
    for(const type of keywordFamilies)for(const v of types[type].variants)try{
      const meta=keywordMeta(type,v);
      if(!`${meta.label} ${v.name} ${v.doc||''}`.toLocaleLowerCase().includes(q))continue;
      const block=flyoutBlock('Instruction',keywordValue(type,v)),key=encode(fromBlock(fakeBlock(block)));
      if(!seen.has(key)){seen.add(key);contents.push(block);}
    }catch(_){}
    // One palette block, but every keyword name remains searchable.
    for(const v of types.Keyword.variants)if(`${v.name} ${pretty(v.name)}`.toLocaleLowerCase().includes(q))try {
      const block=flyoutBlock('Ability',{kind:'Keyword',args:packEnum(types.Keyword,v,fieldsDefault(v.fields,[],`Keyword::${v.wire}`))}),key=encode(fromBlock(fakeBlock(block)));
      if(!seen.has(key)){seen.add(key);contents.push(block);}
    }catch(_){}
    workspace.updateToolbox({kind:'categoryToolbox',contents:[{kind:'category',name:'Matches',colour:palette.effects,contents:contents.length?contents:[{kind:'label',text:'No matching blocks'}]}]});
    workspace.getToolbox().selectItemByPosition(0);
  };

  // Complete workspace snapshots make structural edits (lists and constructor
  // switches) atomic, including any detached children and their field values.
  function remember() {
    const state=JSON.stringify(Blockly.serialization.workspaces.save(workspace));
    if(history[historyIndex]!==state){history=history.slice(0,historyIndex+1);history.push(state);if(history.length>40)history.shift();historyIndex=history.length-1;}
    workspace.clearUndo();
  }
  workspace.undo=redo=>{
    remember();const next=historyIndex+(redo?1:-1);
    if(next<0 || next>=history.length)return;
    historyIndex=next;install(JSON.parse(history[next]),false);
    // Normalize incidental serialization changes made while restoring a frame.
    // They must not truncate the redo history as a new user edit.
    history[historyIndex]=JSON.stringify(Blockly.serialization.workspaces.save(workspace));
  };
  function notice(text) { $('notice').textContent=text; $('notice').style.display='block'; clearTimeout(notice.timer); notice.timer=setTimeout(()=>$('notice').style.display='none',4500); }
  function invalid(text,waiting=false) {
    lastValid=null; $('export').disabled=true; $('validation').className='badge '+(waiting?'':'invalid');
    $('validation').textContent=waiting?'Checking in Rust / Wasm…':'Needs attention'; $('validation-detail').textContent=text;
  }
  const workerCode = `${VALIDATOR_GLUE}\nself.onmessage=async e=>{const m=e.data;try{
    if(m.init){const stream=new Blob([Uint8Array.from(atob(m.init),c=>c.charCodeAt(0))]).stream().pipeThrough(new DecompressionStream('gzip'));wasm_bindgen.initSync({module:new Uint8Array(await new Response(stream).arrayBuffer())});self.postMessage({ready:true});}
    else if(m.batch)self.postMessage({id:m.id,batch:m.batch.map(ir=>{const r=JSON.parse(wasm_bindgen.validate_ir(ir));return {valid:r.valid,error:r.error};})});
    else if(m.describeBindings!==undefined)self.postMessage({id:m.id,result:JSON.parse(wasm_bindgen.describe_card_bindings(m.describeBindings))});
    else if(m.formatDocument!==undefined)self.postMessage({id:m.id,result:JSON.parse(wasm_bindgen.format_card_document(m.formatDocument))});
    else if(m.formatCode!==undefined)self.postMessage({id:m.id,result:JSON.parse(wasm_bindgen.format_card_code(m.formatCode))});
    else if(m.migrateIR!==undefined)self.postMessage({id:m.id,result:JSON.parse(wasm_bindgen.migrate_ir(m.migrateIR))});
    else if(m.parseCode!==undefined)self.postMessage({id:m.id,result:JSON.parse(wasm_bindgen.parse_card_code(m.parseCode))});
    else self.postMessage({id:m.id,result:JSON.parse(wasm_bindgen.validate_ir(m.ir))});
  }catch(e){self.postMessage({id:m.id,error:String(e)});}};`;
  let startedResolve,startedReject;
  const workerStarted=new Promise((resolve,reject)=>{startedResolve=resolve;startedReject=reject;});workerStarted.catch(()=>{});
  const worker=new Worker(URL.createObjectURL(new Blob([workerCode],{type:'application/javascript'})));
  const requests=new Map();let nextRequest=-1;
  function validateTexts(batch){return new Promise((resolve,reject)=>{const id=nextRequest--;requests.set(id,{resolve,reject});worker.postMessage({id,batch});});}
  worker.onmessage=e=>{
    if(requests.has(e.data.id)){const p=requests.get(e.data.id);requests.delete(e.data.id);e.data.error?p.reject(Error(e.data.error)):p.resolve(e.data.batch??e.data.result);return;}
    if(e.data.ready){workerReady=true;startedResolve();changed();return;}
    if(!workerReady&&e.data.error&&e.data.id===undefined){startedReject(Error(e.data.error));invalid(e.data.error);return;}

  };
  worker.onerror=e=>{startedReject(Error(e.message));invalid('The WebAssembly worker stopped: '+e.message);for(const p of requests.values())p.reject(Error(e.message));requests.clear();};
  worker.postMessage({init:payload.wasm});
  async function codeRequest(kind,text){await workerStarted;return new Promise((resolve,reject)=>{const id=nextRequest--;requests.set(id,{resolve,reject});worker.postMessage({id,[kind]:text});});}
  bindingEditor=createBindingEditor({sectionEntries:sections.entries,types,parts,scalar,descriptor,variantFor,viewVariant,childValues,fromBlock,pickerValue,
    unwrapConnection:projections.unwrap,workspace:()=>workspace,generated,revision:()=>revision,request:codeRequest,parse:parseExact,encode,notice,remember,changed,migrationFinished:()=>{preserveSavedDraft=false;}});
  async function refreshCode(){
    if(!lastValid||codeDirty||lastHuman===null)return;
    $('human-editor').value=lastHuman;$('code-status').textContent='Checked by Rust · edit and apply to update blocks';
  }
  function showDump(){
    const human=$('dump-format').value==='human';$('ir').classList.toggle('hidden',human);$('human-editor').classList.toggle('hidden',!human);$('code-actions').classList.toggle('hidden',!human);
    try{$('ir').textContent=dump(generated());if(human&&workerReady)refreshCode().catch(e=>notice(e.message));}catch(_){}
  }
  $('human-editor').oninput=()=>{codeDirty=true;$('code-status').textContent='Unapplied edits · Apply code to validate and update blocks';};
  $('apply-code').onclick=async()=>{
    const text=$('human-editor').value,start=revision;
    try {
      const r=await codeRequest('parseCode',text);
      if(text!==$('human-editor').value||revision!==start){notice('Workspace or code changed while checking; apply again.');return;}
      if(!r.valid){$('code-status').textContent=r.error;return;}
      preserveSavedDraft=false;codeDirty=false;loadIR(parseExact(r.ir),true,parseExact(r.document));notice('Applied validated card code');
    }catch(e){$('code-status').textContent=e.message;}
  };
  $('export-code').onclick=async()=>{
    if(codeDirty){notice('Apply code edits before exporting.');return;}
    if(!lastValid)return;const ir=lastValid;
    try{const r=await codeRequest('formatDocument',encode(bindingEditor.document()));if(r.valid&&lastValid===ir)download(($('card-title').textContent||'card').replace(/[^\p{L}\p{N}_-]+/gu,'_')+'.card',r.code);else if(!r.valid)notice(r.error);}catch(e){notice(e.message);}
  };

  function generated() {
    const roots=workspace.getTopBlocks(false);
    if(roots.length!==1 || roots[0].state.ty.named!=='UnifiedCard') throw Error('Keep exactly one card record, with all other blocks connected. Delete or attach the loose blocks.');
    return fromBlock(roots[0]);
  }
  function validCurrent() {try{return lastValid&&encode(generated())===lastSubmittedIR?lastValid:null;}catch(_){return null;}}
  function allocateFreshResults() {
    const blocks=workspace.getAllBlocks(false),used=new Set();
    for(const b of blocks){
      if(b.state?.pr==='effect'&&!b.state.autoResult)used.add(Number(b.getFieldValue('_RESULT')));
      else if(b.state?.ty.named==='Instruction'&&b.state.variant==='Effect')used.add(Number(b.getFieldValue('_F_result')));
    }
    for(const b of blocks)if(b.state?.pr==='effect'&&b.state.autoResult) {
      let n=0;while(used.has(n))++n;used.add(n);b.state.autoResult=false;b.setFieldValue(String(n),'_RESULT');
    }
  }
  function changed() {
    if(suppress) return;
    ++revision; clearTimeout(timer); invalid('Changes are checked with the native Rust validation rules.',true);
    timer=setTimeout(()=>{
      let routed=false;suppress=true;try{routed=routeAbilities();}finally{suppress=false;}
      if(routed){changed();return;}
      allocateFreshResults();
      applyPresentation();
      remember();
      try {
        const value=generated(),ir=encode(value); $('ir').textContent=dump(value);
        $('card-title').textContent=value.faces.map(f=>f.name).join(' // ')||'New card';
        $('workspace-meta').textContent=`Saved locally${detailMode?' · schema '+schema.version:''} · drag blocks from the left to build`;
        try {if(!preserveSavedDraft)localStorage.setItem('mtgish.blockly.v1',JSON.stringify(savedWorkspace()));else $('workspace-meta').textContent='Original draft preserved · current workspace is not autosaved';}catch(_){$('workspace-meta').textContent+=' · local save unavailable';}
        if(workerReady) {
          const start=revision;lastSubmittedIR=ir;
          bindingEditor.check(ir,start).then(result=>{
            if(!result||start!==revision)return;
            lastValid=ir;lastHuman=result.code;
            // Filling automatic identity metadata belongs to this edit's snapshot.
            if(historyIndex>=0)history[historyIndex]=JSON.stringify(Blockly.serialization.workspaces.save(workspace));
            try{if(!preserveSavedDraft)localStorage.setItem('mtgish.blockly.v1',JSON.stringify(savedWorkspace()));}catch(_){}
            applyPresentation();if($('dump-format').value==='human')refreshCode();
            $('export').disabled=false;$('validation').className='badge valid';$('validation').textContent='✓ Valid Rust IR';
            $('validation-detail').textContent=`Schema ${schema.version} · ${value.faces.length} face${value.faces.length===1?'':'s'} · names and references checked in Rust / WebAssembly`;
          }).catch(e=>{if(start===revision)invalid(e.message);});
        }
      } catch(e) { $('ir').textContent=''; invalid(e.message); }
    },120);
  }
  function install(state,resetHistory=true) {
    const before=Blockly.serialization.workspaces.save(workspace);
    suppress=true; Blockly.Events.disable(); invalid('Loading workspace…',true);
    try {
      Blockly.serialization.workspaces.load(state,workspace);
      workspace.clearUndo();
    } catch(e) {
      Blockly.serialization.workspaces.load(before,workspace);
      throw e;
    } finally {Blockly.Events.enable();suppress=false;changed();}
    if(resetHistory){history=[];historyIndex=-1;remember();}
    applyPresentation();workspace.render();
    Blockly.renderManagement.finishQueuedRenders().then(()=>workspace.zoomToFit());
  }
  function loadIR(value,outline=true,document=null,retainedKeys=[]) {
    const block=blockState({named:'UnifiedCard'},value,outline);
    if(document)bindingEditor.attach(fakeBlock(block),document,retainedKeys);
    bindingEditor.reset();
    install({blocks:{languageVersion:0,blocks:[{...block,x:40,y:40}]}});
  }
  function savedWorkspace() { return {format:'mtgish.blockly.workspace.v1',presentation:6,schema:schema.version,model:payload.model,workspace:Blockly.serialization.workspaces.save(workspace)}; }
  function decodeWorkspace(value,model=schema,captureNames=false) {
    const nodes=value.workspace?.blocks?.blocks;
    if(nodes?.length!==1)throw Error('Keep exactly one complete card record before migration.');
    // Decoding is synchronous and uses fake blocks only. Restore the editor's
    // current descriptor table before loading or rendering any real blocks.
    const current=types;
    try {
      types=model.types;
      const root=fakeBlock(nodes[0]),ir=fromBlock(root);
      return captureNames?{ir,names:bindingEditor.captureDocument(root,ir,(value.presentation??1)<6?1:2)}:ir;
    }finally{types=current;}
  }
  async function migrateWorkspace(value) {
    const sequence=loadSequence,before=JSON.stringify(Blockly.serialization.workspaces.save(workspace));
    localStorage.setItem('mtgish.blockly.before-schema-upgrade',JSON.stringify(value));
    const legacy=payload.legacy_models[String(value.schema)];
    if(!legacy)throw Error('Unsupported saved-workspace schema '+value.schema);
    if((value.presentation??1)<6)localStorage.setItem('mtgish.blockly.before-authoring-v6',JSON.stringify(value));
    const decoded=decodeWorkspace(value,legacy,true),r=decoded.names?await codeRequest('formatDocument',encode(decoded.names.document)):await codeRequest('migrateIR',encode(decoded.ir));
    if(!r.valid)throw Error(r.error);
    if(sequence!==loadSequence||before!==JSON.stringify(Blockly.serialization.workspaces.save(workspace)))throw Error('The workspace changed while upgrading.');
    if(decoded.names){const doc=parseExact(r.document);loadIR(doc.card,true,doc,decoded.names.keys);}
    else loadIR(parseExact(r.ir));preserveSavedDraft=false;
  }
  function restore(value) {
    if(value.schema!==schema.version)throw Error('This workspace requires a schema upgrade; use Import.');
    const presentation=value.presentation??1;
    if(value.workspace?.blocks?.blocks?.length===1 && (presentation!==6||value.model!==payload.model)) {
      try {const ir=decodeWorkspace(value),fresh=blockState({named:'UnifiedCard'},ir);bindingEditor.transfer(fakeBlock(value.workspace.blocks.blocks[0]),fakeBlock(fresh));localStorage.setItem('mtgish.blockly.before-authoring-v6',JSON.stringify(value));if(presentation<5)localStorage.setItem('mtgish.blockly.before-authoring-v5',JSON.stringify(value));if(presentation<4)localStorage.setItem('mtgish.blockly.before-authoring-v4',JSON.stringify(value));bindingEditor.reset();if(presentation<6){preserveSavedDraft=true;bindingEditor.legacy();}install({blocks:{languageVersion:0,blocks:[{...fresh,x:40,y:40}]}});return;}catch(_){}
    }
    install(value.workspace);
  }
  workspace.addChangeListener(e=>{
    choiceView.moved(e);
    if(e.isUiEvent && e.type!==Blockly.Events.SELECTED)setTimeout(applyPresentation,0);
    if(e.type===Blockly.Events.SELECTED){ const b=workspace.getBlockById(e.newElementId); if(b){ const d=descriptor(b.state.ty),v=d?.variants?.find(v=>v.wire===b.state.variant); $('selection').replaceChildren(); const title=document.createElement('strong');title.textContent=typeName(b.state.ty)+(v?'::'+v.name:'');$('selection').append(title,document.createTextNode('  '+(v?.doc||'Connect a value of this type to a matching socket.'))); } }
    if(e.name?.startsWith('NAME:'))return;
    if(!e.isUiEvent && e.type!==Blockly.Events.FINISHED_LOADING) {if(e.type===Blockly.Events.BLOCK_CREATE)bindingEditor?.created(e.ids||[]);changed();}
  });
  const bytes=s=>Uint8Array.from(atob(s),c=>c.charCodeAt(0)), shards=new Map();
  async function shard(n) {
    if(!shards.has(n)) shards.set(n,(async()=>{
      const stream=new Blob([bytes(payload.shards[n])]).stream().pipeThrough(new DecompressionStream('gzip'));
      return parseExact(await new Response(stream).text());
    })());
    return shards.get(n);
  }
  let loadSequence=0;
  async function loadCard(entry) {
    const seq=++loadSequence;
    try {const records=await shard(entry.shard); if(seq!==loadSequence)return; loadIR(records[entry.index]); notice('Loaded '+entry.name);}catch(e){notice('Could not load card: '+e.message);}
  }
  function searchCards() {
    const q=$('search').value.trim().toLocaleLowerCase(),matches=payload.cards.filter(c=>c.name.toLocaleLowerCase().includes(q));
    const favourites=['Lightning Bolt','Llanowar Elves','Cloudshift','Counterspell','Restoration Angel','Basalt Golem','Conjurer\'s Closet'];
    const shown=q ? [...matches].sort((a,b)=>(a.name.toLocaleLowerCase()===q?-1:b.name.toLocaleLowerCase()===q?1:0)).slice(0,50) : favourites.map(n=>payload.cards.find(c=>c.name===n)).filter(Boolean);
    $('card-count').textContent=q ? `${matches.length.toLocaleString()} matches · showing ${shown.length}`:`${payload.cards.length.toLocaleString()} card records · try a favourite`;
    $('card-results').replaceChildren(...shown.map(c=>{
      const b=document.createElement('button');b.className='result';const title=document.createElement('strong');title.textContent=c.name;const small=document.createElement('small');small.textContent=c.types||'Card';b.append(title,small);b.onclick=()=>{preserveSavedDraft=false;loadCard(c);};return b;
    }));
  }
  function blank(draw=false) {
    const v={schema_version:schema.version,layout:'Card',faces:[{schema_version:schema.version,name:draw?'New draw spell':'New creature',characteristics:{supertypes:[],card_types:[draw?'Instant':'Creature'],subtypes:[],mana_cost:[],color_indicator:[],power_toughness:draw?null:[1,1],loyalty:null,defense:null},abilities:[]}],data:[]};
    if(draw) v.faces[0].abilities=[{kind:'Spell',args:{announcement:{targets:[],modes:null},body:{instructions:[{kind:'Effect',args:{result:4294967295,operation:{kind:'Draw',args:{players:{kind:'One',args:{kind:'AbilityController'}},amount:{kind:'Constant',args:1}}}}}]}}}];
    return v;
  }
  function download(name,text) { const url=URL.createObjectURL(new Blob([text],{type:name.endsWith('.card')?'text/plain':'application/json'})),a=document.createElement('a');a.href=url;a.download=name;a.click();setTimeout(()=>URL.revokeObjectURL(url),1000); }
  $('dump-format').onchange=showDump;showDump();
  $('search').oninput=searchCards;$('search').onkeydown=e=>{if(e.key==='Enter')$('card-results').querySelector('button')?.click();};
  function setMode(mode) {
    $('studio').classList.toggle('build-mode',mode==='blocks');
    workspace.getToolbox().setVisible(mode==='blocks');
    if(mode!=='blocks')workspace.getFlyout().hide();
    for(const tab of ['cards','blocks']){$(tab+'-tab').classList.toggle('active',tab===mode);$(tab+'-tab').setAttribute('aria-selected',String(tab===mode));}
    Blockly.svgResize(workspace);
  }
  $('cards-tab').onclick=()=>setMode('cards');$('blocks-tab').onclick=()=>setMode('blocks');
  function openInspector(open) {
    $('studio').classList.toggle('hide-inspector',!open);$('code-view').setAttribute('aria-pressed',String(open));
    Blockly.svgResize(workspace);Blockly.renderManagement.finishQueuedRenders().then(()=>workspace.zoomToFit());
  }
  $('code-view').onclick=()=>openInspector($('studio').classList.contains('hide-inspector'));
  $('details').onclick=()=>{detailMode=!detailMode;$('details').setAttribute('aria-pressed',String(detailMode));applyPresentation();workspace.render();openInspector(detailMode);};
  setMode('blocks');
  $('new-card').onclick=()=>{preserveSavedDraft=false;++loadSequence;const kind=$('starter').value,v=blank(kind!=='creature');if(kind==='spell'){v.faces[0].abilities=[abilityValue('Spell')];v.faces[0].name='New spell';}loadIR(v,false);};
  $('fit').onclick=()=>workspace.zoomToFit();$('zoom-in').onclick=()=>workspace.zoomCenter(1);$('zoom-out').onclick=()=>workspace.zoomCenter(-1);
  $('undo').onclick=()=>workspace.undo(false);$('redo').onclick=()=>workspace.undo(true);
  $('expand').onclick=()=>{workspace.getAllBlocks(false).forEach(b=>b.setCollapsed(false));workspace.zoomToFit();};
  $('collapse').onclick=()=>{workspace.getAllBlocks(false).filter(b=>['Characteristics','Announcement','EntrySpec','MovementTerms','BattlefieldArrival','ExileArrival','CardArrival','TargetSpec'].includes(b.state.ty.named)).forEach(b=>b.setCollapsed(true));workspace.zoomToFit();};
  $('export').onclick=()=>{
    if(!lastValid)return;
    try {if(encode(generated())!==lastSubmittedIR){changed();notice('Checking the latest edit before exporting.');return;}bindingEditor.document();}
    catch(e){invalid(e.message);return;}
    download(($('card-title').textContent||'card').replace(/[^\p{L}\p{N}_-]+/gu,'_')+'.ir.json',lastValid);
  };
  $('save-workspace').onclick=()=>download('card.workspace.json',JSON.stringify(savedWorkspace(),null,2));
  $('import').onclick=()=>$('file').click();$('file').onchange=async()=>{
    const f=$('file').files[0];if(!f)return;const seq=++loadSequence;
    try {
      const text=await f.text();let value,document=null;
      if(text.trimStart().startsWith('{'))value=parseExact(text);
      else {const r=await codeRequest('parseCode',text);if(!r.valid)throw Error(r.error);value=parseExact(r.ir);document=parseExact(r.document);}
      if(seq!==loadSequence)return;
      if(value.format==='mtgish.blockly.workspace.v1') {if(value.schema===schema.version){restore(value);if((value.presentation??1)>=6)preserveSavedDraft=false;}else await migrateWorkspace(value);}
      else if(document) {preserveSavedDraft=false;loadIR(value,true,document);}
      else if([1,2].includes(value.version)&&value.card&&Array.isArray(value.bindings)) {const r=await codeRequest('formatDocument',encode(value));if(seq!==loadSequence)return;if(!r.valid)throw Error(r.error);const upgraded=parseExact(r.document);preserveSavedDraft=false;loadIR(upgraded.card,true,upgraded);}
      else {const r=await codeRequest('migrateIR',encode(value));if(seq!==loadSequence)return;if(!r.valid)throw Error(r.error);preserveSavedDraft=false;loadIR(parseExact(r.ir));}
      notice('Imported '+f.name);
    }catch(e){notice('Import failed: '+e.message);}finally{$('file').value='';}
  };
  searchCards();
  let restored=false;
  try {const saved=JSON.parse(localStorage.getItem('mtgish.blockly.v1'));if(saved){if(saved.schema===schema.version)restore(saved);else await migrateWorkspace(saved);restored=true;}}catch(e){preserveSavedDraft=true;notice('Saved draft could not be upgraded; the original save is preserved. '+e.message);}
  if(!restored&&!workspace.getAllBlocks(false).length) await loadCard(payload.cards.find(c=>c.name==='Lightning Bolt')||payload.cards[0]);
  $('loading').classList.add('hidden');
  // Small public inspection surface also used by the browser regression suite.
  window.cardBlocks={valueView,flowView,choiceView,projections,selectors:selectorEditor,namedAbilities,costs:costEditor,properties:propertyEditor,sections,addSection,removeSection,moveAbility,removeOption,workspace,bindings:bindingEditor,fakeBlock,schema,parseExact,encode,dump,fromBlock,blockState,loadIR,generated,blank,loadCard,shard,payload,types,parts,applyPresentation,setMode,addToFlow,abilityValue,actionValue,defaultValue,ready:()=>workerReady,valid:validCurrent,savedWorkspace,restore,validateTexts,describeBindings:text=>codeRequest('describeBindings',text),formatDocument:text=>codeRequest('formatDocument',text),formatCode:text=>codeRequest('formatCode',text),parseCode:text=>codeRequest('parseCode',text),migrateWorkspace,migrateIR:text=>codeRequest('migrateIR',text)};
})().catch(e=>{document.getElementById('loading').textContent='Could not start demo: '+e.message;console.error(e);});
