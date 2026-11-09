/* Card blocks: schema-driven Blockly <-> exact Rust IR, with local Wasm validation. */
'use strict';
(async () => {
  const payload = JSON.parse(document.getElementById('demo-payload').textContent);
  let types=payload.schema.types;
  const schema = payload.schema, authoring = schema.authoring, $ = id => document.getElementById(id);
  const clone = v => structuredClone(v), typeKey = t => JSON.stringify(t);
  const pretty = text => text.replace(/_/g,' ').replace(/([a-z0-9])([A-Z])/g,'$1 $2');
  const typeName = t => t.named || (t.vec ? `List<${typeName(t.vec)}>` : t.option ? `Optional<${typeName(t.option)}>` : `(${t.tuple.map(typeName).join(', ')})`);
  const descriptor = t => t.named && types[t.named];
  const newtype = t => {const d=descriptor(t);return d?.kind==='struct' && d.fields.kind==='tuple' && d.fields.items.length===1 ? d.fields.items[0]:null;};
  const scalar = t => !!t.named && (!types[t.named] || (newtype(t) && scalar(newtype(t))) || (types[t.named].kind === 'enum' && !types[t.named].adjacent && types[t.named].variants.every(v => v.fields.kind === 'unit')));
  const excluded = v => /^(Draft|Named$)/.test(v.name) || ['Primitive','Form'].includes(v.name);
  const preferred = {Ability:'Spell',Instruction:'Effect',Operation:'Draw',Expr:'Constant',PlayerRef:'AbilityController',PlayerSet:'One',ObjectRef:'Source',ObjectSet:'One',ManaSymbol:'Generic',Keyword:'Flying',Count:'Exactly',Destination:'Exile',TargetKind:'Object',ObjectKind:'Permanent'};
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
  function field(t,value) {
    if(newtype(t))return field(newtype(t),value);
    if (t.named === 'bool') return new Blockly.FieldCheckbox(fieldValue(t,value));
    if (types[t.named]) return new Blockly.FieldDropdown(types[t.named].variants.map(v => [pretty(v.name),v.wire]));
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
    EventSpec:[['this card enters',{ZoneChange:{objects:{kind:'SameObject',args:{kind:'Source'}},from:null,to:'Battlefield'}}]],
  };
  const same=(a,b)=>{
    if(a===b)return true;
    if(a===null||b===null||typeof a!=='object'||typeof b!=='object')return false;
    if(Array.isArray(a)!==Array.isArray(b))return false;
    const keys=Object.keys(a);return keys.length===Object.keys(b).length&&keys.every(k=>Object.hasOwn(b,k)&&same(a[k],b[k]));
  };
  const targetPresets=authoring.targets.map(t=>[t.label,t.fields]);
  const targetPreset = v => targetPresets.findIndex(([,p])=>Object.keys(p).every(k=>same(p[k],v[k])));
  function targetSummary(v) {
    const i=targetPreset(v),base=i<0?'custom target':targetPresets[i][0];
    const count=v.count?.Exactly?.kind==='Constant'?v.count.Exactly.args:null;
    return `${count===1?'':count!==null?count+' × ':'variable count · '}${base}${v.constraints.length||v.distribute!==null||v.chooser!==null?' · with conditions':''}`;
  }
  function mapTargetDomain(t,value,slot,preset,root=false) {
    // Nested ability definitions introduce their own target-slot scope.
    if(t.named==='Ability'&&!root)return value;
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
  function chooseTarget(b,index) {
    if(b.isInFlyout)return;
    if(index==='CUSTOM') {
      b.state.shown=[...new Set([...(b.state.shown||[]),'F_kind','F_filter','F_players'])];
      applyPresentation();changed();return;
    }
    const preset=targetPresets[Number(index)];if(!preset)return;
    let owner=b.getParent();while(owner&&owner.state.ty.named!=='Ability')owner=owner.getParent();
    if(owner) {
      const slot=Number(b.getFieldValue('_F_slot'));
      // Keep recipient references in the same scope consistent with the new entity kind.
      replaceView(owner,{},mapTargetDomain(owner.state.ty,fromBlock(owner),slot,preset[1],true));
      return;
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
      result=[{key:'VALUE',label:'',ty:t,inline:state.literal}];
    } else if(state.pr==='single') {
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
    return result.map(p=>{
      const inline=state.inline?.[p.key];
      if(inline && (state.ui<3 || inline==='pt') && !state.noInline?.includes(p.key))p={...p,inline,editTy:inline==='constant'?{named:'i64'}:p.ty};
      if(state.ui>=2 && !p.inline) {
        if(p.ty.named==='Program')p={...p,flow:'program',socketTy:{named:'Instruction'},statement:true};
        else if(p.ty.vec && ['Ability','Instruction'].includes(p.ty.vec.named))p={...p,flow:'array',socketTy:p.ty.vec,statement:true};
      }
      if(state.negativePrevention && p.key.endsWith('preventable'))p={...p,inline:'unpreventable',label:"can't be prevented"};
      if(p.label==='characteristics')p.label='properties';
      if(p.label==='announcement')p.label='targets';
      if(p.flow)p.label=p.ty.named==='Program'?(p.label==='otherwise'?'otherwise':'do'):p.ty.vec.named==='Ability'?'abilities':'do';
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
    const variant=variantFor(t,value),s={ty:t,ui:3,negativePrevention:true,...(variant?{variant:variant.wire}:{})};
    if(t.named==='Expr'&&value.kind==='Constant'){s.pr='literal';s.literal='constant';}
    else if(choices[t.named]?.some(([,v])=>same(v,value))){s.pr='literal';s.literal='choice';}
    if(t.named==='UnifiedCard' && value.faces.length===1)s.pr='single';
    if(t.named==='Instruction' && value.kind==='Effect'){s.pr='effect';s.op=value.args.operation.kind;}
    if(t.named==='Ability' && value.kind==='Keyword'){s.pr='keyword';s.keyword=value.args.kind;}
    return s;
  }
  function fieldValues(f,a,prefix='F_') {
    if(f.kind==='unit')return {};
    if(f.kind==='record')return Object.fromEntries(f.fields.map(f=>[prefix+f.name,a[f.name]]));
    return Object.fromEntries(f.items.map((_,i)=>[`${prefix}ARG_${i}`,f.items.length===1?a:a[i]]));
  }
  function childValues(t,value,state) {
    if(state.pr==='literal')return {VALUE:value};
    if(state.pr==='single')return {ROOT_schema:value.schema_version,ROOT_layout:value.layout,ROOT_data:value.data,...fieldValues(types.Card.fields,value.faces[0])};
    if(state.pr==='effect')return {RESULT:value.args.result,...fieldValues(viewVariant(state).fields,value.args.operation.args,'O_')};
    if(state.pr==='keyword')return fieldValues(viewVariant(state).fields,value.args.args,'K_');
    if(scalar(t))return {VALUE:value};
    if(t.vec)return Object.fromEntries(value.map((v,i)=>[`ITEM_${i}`,v]));
    if(t.option)return value===null?{}:{SOME:value};
    const d=descriptor(t),v=variantFor(t,value),f=t.tuple?{kind:'tuple',items:t.tuple}:v?v.fields:d.fields;
    return fieldValues(f,v?(d.adjacent?value.args:value[v.wire]):value,f.kind==='tuple'?'':'F_');
  }
  function chainState(ty,values,outline) {
    let head=null,last=null;
    for(const value of values) {
      const next=toBlock(ty,value,outline);next.extraState.statement=true;
      if(last)last.next={block:next};else head=next;last=next;
    }
    return head;
  }
  function toBlock(t,value,outline=false,override={}) {
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
    if(variant && !scalar(t))result.fields.VARIANT=variant.wire;
    if(outline && ['Characteristics','EntrySpec','MovementTerms'].includes(t.named))result.collapsed=true;
    return result;
  }
  function colour(t) {
    const n=typeName(t);
    if (/Card|Characteristics|ManaSymbol/.test(n)) return '#657b62';
    if (/Ability|Keyword|Casting/.test(n)) return '#477d70';
    if (/Operation|Instruction|Program/.test(n)) return '#b46b4f';
    if (/Expr|ResultId|Binding|Slot|i32|u32|i64/.test(n)) return '#ac8730';
    if (/Condition|Filter|Duration|Count/.test(n)) return '#86739a';
    if (/Object|Target|Recipient/.test(n)) return '#5a7f9e';
    if (/Player|Event/.test(n)) return '#508d88';
    return '#7a8b86';
  }
  const icon = s => 'data:image/svg+xml;charset=utf-8,'+encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" width="22" height="22"><rect x="1" y="1" width="20" height="20" rx="5" fill="white" fill-opacity=".22"/><text x="11" y="16" text-anchor="middle" fill="white" font-size="19">${s}</text></svg>`);
  function button(symbol,label,fn) { return new Blockly.FieldImage(icon(symbol),22,22,label,fn); }
  let workspace, suppress=false, revision=0, timer, lastValid=null, lastSubmittedIR=null, workerReady=false, preserveSavedDraft=false;
  let history=[],historyIndex=-1;
  let detailMode=false,codeDirty=false,codeIR=null,lastHuman=null;
  function scalarType(p){return p.inline==='pt'?{named:'i32'}:p.inline==='constant'?{named:'i64'}:p.inline==='choice'?{named:'String'}:scalar(p.ty)?p.ty:null;}
  function valueField(p,value){return p.inline==='choice'?new Blockly.FieldDropdown(choices[p.ty.named].map(([label],i)=>[label,String(i)])):field(scalarType(p),value);}
  function isMetadataEmpty(v) {
    if(v===null || v===false || v==='DestinationDefault')return true;
    if(Array.isArray(v))return v.length===0;
    return !!v && typeof v==='object' && Object.values(v).every(isMetadataEmpty);
  }
  const defaultStructures=new Set(['Announcement','TriggerLimits','ActivationLimits','EntrySpec','MovementTerms']);
  function hiddenReason(b,p) {
    if(b.state.shown?.includes(p.key))return false;
    if(p.key==='RESULT'||(b.state.ty.named==='Instruction'&&b.state.variant==='Effect'&&p.key==='F_result'))return true;
    if(detailMode)return false;
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
  function optionsFor(b) {
    const out=[['⋯','NONE']];
    for(const p of parts(b.state)) {
      if(!detailMode && (p.key.startsWith('ROOT_')||['F_schema_version','RESULT'].includes(p.key)))continue;
      if(hiddenReason(b,p))out.push([p.ty.named==='Announcement'?'Add a target':p.ty.vec?'Add '+p.label.replace(/ies$/,'y').replace(/s$/,''):p.flow?'Add otherwise':pretty(p.label||p.key),'SHOW:'+p.key]);
      if(b.state.ui<3 && p.inline && p.inline!=='unpreventable')out.push(['Use a block for '+pretty(p.key.replace(/^(F_|O_|K_)/,'')),'BLOCK:'+p.key]);
    }
    return out;
  }
  function collapsedSummary(b) {
    const name=b.state.ty.named;
    if(!['Characteristics','Announcement','TargetSpec'].includes(name))return null;
    const v=fromBlock(b);
    if(name==='Characteristics')return [...v.supertypes,...v.card_types,...v.subtypes,v.power_toughness?.join('/')].filter(Boolean).join(' · ')||'Properties';
    if(name==='TargetSpec')return targetSummary(v);
    return [...v.targets.map(targetSummary),v.modes?'mode choices':''].filter(Boolean).join(' · ')||'No targets';
  }
  function applyPresentation() {
    if(!workspace)return;
    const all=[...workspace.getAllBlocks(false),...(workspace.getFlyout()?.getWorkspace().getAllBlocks(false)||[])];
    for(const b of all) {
      if(!b.state)continue;
      if(b.isCollapsed()) {
        try {
          const summary=collapsedSummary(b);
          // Summary text is presentation, not a card edit. Avoid validation/save loops.
          Blockly.Events.disable();try{if(summary)b.getField('_TEMP_COLLAPSED_FIELD')?.setValue(summary);}finally{Blockly.Events.enable();}
        }catch(_){}
        continue;
      }
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
      b.getField('COST_ZERO')?.setValue(b.state.present&&b.getInputTargetBlock('SOME')?.state.count===0?'0':'');
      let rerender=false;
      for(const p of parts(b.state)) {
        const input=b.getInput(p.key),hidden=hiddenReason(b,p);
        if(input && input.isVisible()===hidden){input.setVisible(!hidden);rerender=true;}
        b.getField('FILL_'+p.key)?.setVisible(!hidden && (p.flow || !b.getInputTargetBlock(p.key)));
      }
      const menu=b.getField('MORE');if(menu)menu.setVisible(optionsFor(b).length>1);
      b.getField('TYPE_LABEL')?.setVisible(detailMode);
      if(rerender && b.rendered)b.render();
    }
    Blockly.renderManagement.finishQueuedRenders();
  }
  function replaceView(b,patch,value=fromBlock(b)) {
    const old=Blockly.serialization.blocks.save(b),next=old.next;
    const fresh=blockState(b.state.ty,value,b.isCollapsed(),{...b.state,...patch});
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
    b.state.shown=[...new Set([...(b.state.shown||[]),key])];
    const child=b.getInputTargetBlock(key);
    child?.setCollapsed(false);
    if(p.ty.named==='Announcement' && child)child.getInputTargetBlock('F_targets')?.resize(1);
    else if(child?.state.ty.vec)child.resize(1);
    else if(child?.state.ty.option && !child.state.present)child.resize(0);
    else if(scalarType(p)?.named==='bool')b.setFieldValue(b.getFieldValue('_'+key)==='TRUE'?'FALSE':'TRUE','_'+key);
    applyPresentation();b.render();changed();
  }
  function addToFlow(parent,p,value) {
    if(parent.isInFlyout)return;
    const t=p.socketTy,state=blockState(t,value??defaultValue(t));state.extraState.statement=true;if(state.extraState.pr==='effect')state.extraState.autoResult=true;
    const b=Blockly.serialization.blocks.append(state,workspace);
    let last=parent.getInputTargetBlock(p.key);
    if(!last)parent.getInput(p.key).connection.connect(b.previousConnection);
    else {while(last.getNextBlock())last=last.getNextBlock();last.nextConnection.connect(b.previousConnection);}
    b.select();changed();
  }
  function insertDefault(parent,p) {
    if(parent.isInFlyout)return;
    try {
      if(p.flow){addToFlow(parent,p);return;}
      const value=defaultValue(p.ty);
      if(p.ty.named==='TargetSpec'&&parent.state.ty.vec?.named==='TargetSpec'){const slots=parent.getChildren(false).filter(b=>b.state.ty.named==='TargetSpec').map(b=>Number(b.getFieldValue('_F_slot')));value.slot=slots.length?Math.max(...slots)+1:0;}
      const state=blockState(p.ty,value);if(p.statement)state.extraState.statement=true;
      const b=Blockly.serialization.blocks.append(state,workspace);
      parent.getInput(p.key).connection.connect(p.statement?b.previousConnection:b.outputConnection);b.select();
    }catch(e){notice(e.message);}
  }
  function caption(state) {
    if(state.pr==='single')return 'Card';
    if(state.ty.named==='Characteristics')return 'Properties';
    if(state.ty.named==='TargetSpec')return 'Target';
    if(state.ty.named==='Announcement')return 'Targets & choices';
    return typeName(state.ty);
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
          setTimeout(()=>{if(workspace.getBlockById(this.id))chooseTarget(this,selected);},0);
        }
        if(e.type===Blockly.Events.BLOCK_CHANGE && e.blockId===this.id && e.name==='VARIANT') {
          const v=this.getFieldValue('VARIANT'),key=this.state.pr==='effect'?'op':this.state.pr==='keyword'?'keyword':'variant';
          if(v && v!==this.state[key]){this.state[key]=v;this.rebuild();}
        }
      });
    },
    saveExtraState(){return clone(this.state);},
    loadExtraState(state) {
      if(!state?.ty || !Number.isInteger(state.count??0) || (state.count??0)<0 || (state.count??0)>2048)throw Error('Invalid block state or a list longer than 2,048 items.');
      this.state=clone(state);this.rebuild();
    },
    rebuild() {
      const attached={},shadows={},values={};
      Blockly.Events.disable();
      try {
        for(const input of this.inputList){
          const child=input.connection?.targetBlock();
          if(input.connection)shadows[input.name]=input.connection.stashShadowState().shadowState;
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
        if(this.state.statement){this.setOutput(false);this.setPreviousStatement(true,typeKey(t));this.setNextStatement(['Instruction','Ability'].includes(t.named),typeKey(t));}
        else {this.setNextStatement(false);this.setPreviousStatement(false);this.setOutput(true,typeKey(t));}
        this.setColour(colour(this.state.pr==='effect'?{named:'Operation'}:t));
        if(this.state.pr==='literal') { /* The editable field itself is the literal block. */ }
        else if(t.named==='TargetSpec')header.appendField(new Blockly.FieldDropdown([...targetPresets.map(([label],i)=>[label,String(i)]),['custom target…','CUSTOM']]),'TARGET_PRESET');
        else if(t.vec){header.appendField(t.vec.named==='TargetSpec'?'Targets':typeName(t.vec)+' items');header.appendField(button('+','Add an item',()=>this.resize(1)));if(this.state.count)header.appendField(button('−','Remove last item',()=>this.resize(-1)));}
        else if(t.option){header.appendField(t.option.vec?.named==='ManaSymbol'?(this.state.present?'Mana cost':'No mana cost'):(this.state.present?'Value':'Not set'));if(t.option.vec?.named==='ManaSymbol')header.appendField('','COST_ZERO');header.appendField(button(this.state.present?'−':'+','Toggle optional value',()=>this.resize(0)));}
        else if(variant && !scalar(t)) {
          const vd=this.state.pr==='effect'?types.Operation:this.state.pr==='keyword'?types.Keyword:d;
          header.appendField(new Blockly.FieldDropdown(vd.variants.map(v=>[this.state.pr==='effect'?(authoring.actions[v.wire]?.label||pretty(v.name)):(authoring.references[this.state.ty.named]?.[v.wire]?pretty(authoring.references[this.state.ty.named][v.wire]):pretty(v.name)),v.wire])),'VARIANT');this.setFieldValue(variant.wire,'VARIANT');
          header.appendField('· '+t.named,'TYPE_LABEL');
        }else header.appendField(caption(this.state));
        if(header)header.appendField(new Blockly.FieldDropdown(()=>optionsFor(this),v=>{
          if(v==='NONE')return null;
          setTimeout(()=>{if(v.startsWith('SHOW:'))revealOption(this,v.slice(5));else if(v.startsWith('BLOCK:'))replaceView(this,{noInline:[...(this.state.noInline||[]),v.slice(6)]});},0);return null;
        }),'MORE');
        for(const p of parts(this.state)) {
          if(p.inline==='pt') {
            const input=this.appendDummyInput(p.key).appendField('power / toughness');
            for(const [i,suffix] of ['P','T'].entries()){if(i)input.appendField('/');input.appendField(new Blockly.FieldTextInput(values['_'+p.key+'_'+suffix]||'0'),'_'+p.key+'_'+suffix);}
          }else if(scalarType(p)) {
            const v=values['_'+p.key]??(p.inline==='choice'?'0':fieldValue(scalarType(p),defaultValue(scalarType(p))));
            const input=this.appendDummyInput(p.key);if(p.label)input.appendField(p.label);input.appendField(valueField(p,v),'_'+p.key);this.getField('_'+p.key).setValue(v);if(p.suffix)input.appendField(p.suffix);
          }else {
            const ty=p.socketTy||p.ty;
            (p.statement?this.appendStatementInput(p.key):this.appendValueInput(p.key)).setCheck(typeKey(ty)).appendField(p.label)
              .appendField(button('+',p.flow?'Add a block':`Fill ${p.label}`,()=>insertDefault(this,p)),'FILL_'+p.key);
            const shadow=shadows[p.key];
            if(shadow&&typeKey(shadow.extraState.ty)===typeKey(ty))this.getInput(p.key).connection.setShadowState(shadow);
            const old=attached[p.key];
            if(old && typeKey(old.state.ty)===typeKey(ty))this.getInput(p.key).connection.connect(p.statement?old.previousConnection:old.outputConnection);
          }
        }
        this.setTooltip(`${typeName(t)}${variant?'::'+variant.name:''}\n${variant?.doc||''}`);
        this.setInputsInline(this.state.pr==='literal'||this.state.pr==='effect'||this.state.pr==='keyword'||scalar(t)||(d?.kind==='struct'&&d.fields.kind==='tuple'&&d.fields.items.every(scalar)));
      }finally{Blockly.Events.enable();}
      if(this.rendered)this.render();
    },
    resize(delta) {
      if(this.isInFlyout)return;
      if(this.state.ty.vec&&delta>0&&this.state.count>=2048){notice('This demo allows up to 2,048 items in one list.');return;}
      Blockly.Events.setGroup(true);
      try {
        if(this.state.ty.vec){if(delta<0)this.getInputTargetBlock(`ITEM_${this.state.count-1}`)?.dispose(false);this.state.count=Math.max(0,this.state.count+delta);}
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
  function fromBlock(b) {
    if(!b || b.type!=='ir_value')throw Error('Fill the missing block before exporting.');
    if(!b.isEnabled())throw Error('Re-enable or remove disabled blocks before exporting IR.');
    const state=b.state,t=state.ty,d=descriptor(t),values={};
    for(const p of parts(state)) {
      if(p.inline==='pt')values[p.key]=[readScalar({named:'i32'},b.getFieldValue('_'+p.key+'_P')),readScalar({named:'i32'},b.getFieldValue('_'+p.key+'_T'))];
      else if(p.inline==='constant')values[p.key]={kind:'Constant',args:readScalar({named:'i64'},b.getFieldValue('_'+p.key))};
      else if(p.inline==='choice') {
        const choice=choices[p.ty.named]?.[Number(b.getFieldValue('_'+p.key))];if(!choice)throw Error('Choose a valid '+p.label);values[p.key]=clone(choice[1]);
      } else if(p.inline==='unpreventable')values[p.key]=!readScalar(p.ty,b.getFieldValue('_'+p.key));
      else if(scalar(p.ty))values[p.key]=readScalar(p.ty,b.getFieldValue('_'+p.key));
      else if(p.flow) {const list=chainValues(b.getInputTargetBlock(p.key));values[p.key]=p.flow==='program'?{instructions:list}:list;}
      else {
        const child=b.getInputTargetBlock(p.key);if(!child)throw Error(`${typeName(t)}: fill the ${p.label} socket.`);
        if(typeKey(child.state.ty)!==typeKey(p.ty))throw Error(`Wrong type in ${p.label}`);
        values[p.key]=fromBlock(child);
      }
    }
    if(state.pr==='literal')return values.VALUE;
    if(state.pr==='single')return {schema_version:values.ROOT_schema,layout:values.ROOT_layout,data:values.ROOT_data,faces:[fieldsResult(types.Card.fields,values)]};
    if(state.pr==='effect')return {kind:'Effect',args:{result:values.RESULT,operation:packEnum(types.Operation,viewVariant(state),fieldsResult(viewVariant(state).fields,values,'O_'))}};
    if(state.pr==='keyword')return {kind:'Keyword',args:packEnum(types.Keyword,viewVariant(state),fieldsResult(viewVariant(state).fields,values,'K_'))};
    if(scalar(t))return values.VALUE;
    if(t.vec) {
      if(['Instruction','Ability'].includes(t.vec.named))return parts(state).flatMap(p=>chainValues(b.getInputTargetBlock(p.key)));
      return parts(state).map(p=>values[p.key]);
    }
    if(t.option)return state.present?values.SOME:null;
    const v=viewVariant(state),f=t.tuple?{kind:'tuple',items:t.tuple}:v?v.fields:d.fields;
    const a=fieldsResult(f,values,f.kind==='tuple'?'':'F_');return v?packEnum(d,v,a):a;
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
  function abilityValue(name) {
    const v=types.Ability.variants.find(v=>v.wire===name),args=fieldsDefault(v.fields,[],`Ability::${name}`);
    if(name==='Triggered')args.event=clone(choices.EventSpec[0][1]);
    return packEnum(types.Ability,v,args);
  }
  function flyoutBlock(type,value) {
    const state=blockState({named:type},value);
    if(['Ability','Instruction'].includes(type))state.extraState.statement=true;
    if(state.extraState.pr==='effect')state.extraState.autoResult=true;
    return {kind:'block',...state};
  }
  function variantBlock(type,name) {
    const d=types[type],v=d.variants.find(v=>v.wire===name);
    return flyoutBlock(type,packEnum(d,v,fieldsDefault(v.fields,[],`${type}::${name}`)));
  }
  function actionGroups() {
    const groups=new Map();
    for(const v of types.Operation.variants.filter(v=>!excluded(v)))try {
      const category=authoring.actions[v.wire].category;
      if(!groups.has(category))groups.set(category,[]);
      groups.get(category).push(flyoutBlock('Instruction',actionValue(v.wire)));
    }catch(_){}
    return [...groups].sort(([a],[b])=>a.localeCompare(b)).map(([name,contents])=>({kind:'category',name,colour:'#b46b4f',contents}));
  }
  const toolboxDefinition={kind:'categoryToolbox',contents:[
    {kind:'category',name:'Abilities',colour:'#477d70',contents:[
      {kind:'label',text:'Drag an ability into your card'},
      ...['Spell','Triggered','Activated'].map(n=>flyoutBlock('Ability',abilityValue(n))),
      {kind:'label',text:'Keywords'},
      ...['Flying','Deathtouch','Haste','Vigilance','Lifelink','Trample'].map(n=>flyoutBlock('Ability',{kind:'Keyword',args:{kind:n}}))
    ]},
    {kind:'category',name:'Actions',colour:'#b46b4f',contents:[
      {kind:'label',text:'Stack actions inside an ability'},
      ...['Draw','GainLife','LoseLife'].map(n=>flyoutBlock('Instruction',actionValue(n,{players:{kind:'One',args:SELF},amount:{kind:'Constant',args:1}}))),
      flyoutBlock('Instruction',actionValue('DealDamage',{source:{kind:'Source'},recipients:{kind:'Targets',args:0},amount:{kind:'Constant',args:3},preventable:true})),
      flyoutBlock('Instruction',actionValue('Move',{objects:{kind:'One',args:{kind:'Source'}},destination:'Exile'}))
    ]},
    {kind:'category',name:'All actions',colour:'#b46b4f',contents:actionGroups()},
    {kind:'category',name:'Control',colour:'#9b7a42',contents:[
      ...['If','Optional','Repeatable','ForEachPlayer'].map(n=>variantBlock('Instruction',n))
    ]},
    {kind:'category',name:'Events',colour:'#508d88',contents:[
      flyoutBlock('EventSpec',choices.EventSpec[0][1]),
      flyoutBlock('EventSpec',{Turn:{point:'UpkeepBegins',players:{kind:'One',args:SELF}}})
    ]},
    {kind:'category',name:'Targets',colour:'#5a7f9e',contents:[
      ...targetPresets.map(([,p])=>flyoutBlock('TargetSpec',{...defaultValue({named:'TargetSpec'}),...clone(p)})),
      ...['Target','Source'].map(n=>variantBlock('ObjectRef',n)),
      variantBlock('ObjectSet','Targets')
    ]},
    {kind:'category',name:'Values',colour:'#ac8730',contents:[
      flyoutBlock('Expr',{kind:'Constant',args:1}),
      flyoutBlock('Expr',{kind:'AnnouncedX'}),
      flyoutBlock('Expr',{kind:'Add',args:[{kind:'Constant',args:0},{kind:'Constant',args:0}]}),
      ...choices.PlayerSet.map(([,v])=>flyoutBlock('PlayerSet',v)),
      ...['Compare','Not','All','Any'].filter(n=>types.Condition.variants.some(v=>v.wire===n)).map(n=>variantBlock('Condition',n))
    ]},
    {kind:'category',name:'Advanced',colour:'#86739a',contents:
      ['Expr','Condition','PlayerRef','PlayerSet','ObjectFilter','ObjectRef','ObjectSet','CostPart','Duration','EventSpec'].map(type=>({kind:'category',name:type,colour:colour({named:type}),contents:types[type].variants.filter(v=>!excluded(v)).flatMap(v=>{try{return [variantBlock(type,v.wire)];}catch(_){return [];}})}))}
  ]};
  const theme=Blockly.Theme.defineTheme('cardBlocks',{base:Blockly.Themes.Classic,fontStyle:{family:'system-ui, sans-serif',size:11},componentStyles:{workspaceBackgroundColour:'#f4f6f1',scrollbarColour:'#a8b5a4',scrollbarOpacity:.6,insertionMarkerColour:'#52794c',insertionMarkerOpacity:.4}});
  workspace=Blockly.inject('workspace',{renderer:'zelos',theme,toolbox:toolboxDefinition,trashcan:false,sounds:false,zoom:{controls:false,wheel:true,startScale:.75,maxScale:1.6,minScale:.12,scaleSpeed:1.16},move:{scrollbars:true,drag:true,wheel:true},grid:{spacing:22,length:1,colour:'#d6dfd1',snap:false}});
  new ResizeObserver(()=>Blockly.svgResize(workspace)).observe($('workspace'));
  $('block-search').oninput=()=>{
    const q=$('block-search').value.trim().toLocaleLowerCase();
    if(!q){workspace.updateToolbox(toolboxDefinition);return;}
    const contents=[];
    for(const v of types.Operation.variants.filter(v=>!excluded(v))) {
      const meta=authoring.actions[v.wire];
      if(!`${v.name} ${meta.name} ${meta.label} ${meta.category} ${v.doc}`.toLocaleLowerCase().includes(q))continue;
      try{contents.push(flyoutBlock('Instruction',actionValue(v.wire)));}catch(_){}
    }
    workspace.updateToolbox({kind:'categoryToolbox',contents:[{kind:'category',name:'Matches',colour:'#b46b4f',contents:contents.length?contents:[{kind:'label',text:'No matching actions'}]}]});
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
    if(e.data.id!==revision) return;
    const r=e.data.result;
    if(e.data.error || !r.valid) { invalid(e.data.error||r.error); return; }
    lastValid=r.ir;if($('dump-format').value==='human')refreshCode().catch(e=>{$('code-status').textContent=e.message;}); $('export').disabled=false; $('validation').className='badge valid'; $('validation').textContent='✓ Valid Rust IR';
    $('validation-detail').textContent=`Schema ${r.schema} · ${r.faces} face${r.faces===1?'':'s'} · checked in this browser with WebAssembly`;
  };
  worker.onerror=e=>{startedReject(Error(e.message));invalid('The WebAssembly worker stopped: '+e.message);for(const p of requests.values())p.reject(Error(e.message));requests.clear();};
  worker.postMessage({init:payload.wasm});
  async function codeRequest(kind,text){await workerStarted;return new Promise((resolve,reject)=>{const id=nextRequest--;requests.set(id,{resolve,reject});worker.postMessage({id,[kind]:text});});}
  async function refreshCode(){
    if(!lastValid||codeDirty)return;
    const ir=lastValid;
    if(codeIR===ir&&lastHuman!==null){$('human-editor').value=lastHuman;return;}
    const r=await codeRequest('formatCode',ir);
    if(lastValid!==ir||codeDirty)return;
    if(!r.valid){$('code-status').textContent=r.error;return;}
    codeIR=ir;lastHuman=r.code;$('human-editor').value=r.code;$('code-status').textContent='Checked by Rust · edit and apply to update blocks';
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
      preserveSavedDraft=false;codeDirty=false;codeIR=null;loadIR(parseExact(r.ir));notice('Applied validated card code');
    }catch(e){$('code-status').textContent=e.message;}
  };
  $('export-code').onclick=async()=>{
    if(codeDirty){notice('Apply code edits before exporting.');return;}
    if(!lastValid)return;const ir=lastValid;
    try{const r=await codeRequest('formatCode',ir);if(r.valid&&lastValid===ir)download(($('card-title').textContent||'card').replace(/[^\p{L}\p{N}_-]+/gu,'_')+'.card',r.code);else if(!r.valid)notice(r.error);}catch(e){notice(e.message);}
  };

  function generated() {
    const roots=workspace.getTopBlocks(false);
    if(roots.length!==1 || roots[0].state.ty.named!=='UnifiedCard') throw Error('Keep exactly one card record, with all other blocks connected. Delete or attach the loose blocks.');
    return fromBlock(roots[0]);
  }
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
      allocateFreshResults();
      applyPresentation();
      remember();
      try {
        const value=generated(),ir=encode(value); $('ir').textContent=dump(value);
        $('card-title').textContent=value.faces.map(f=>f.name).join(' // ')||'New card';
        $('workspace-meta').textContent=`Saved locally${detailMode?' · schema '+schema.version:''} · drag blocks from the left to build`;
        try {if(!preserveSavedDraft)localStorage.setItem('mtgish.blockly.v1',JSON.stringify(savedWorkspace()));else $('workspace-meta').textContent='Original draft preserved · current workspace is not autosaved';}catch(_){$('workspace-meta').textContent+=' · local save unavailable';}
        if(workerReady) {lastSubmittedIR=ir;worker.postMessage({id:revision,ir});}
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
  function loadIR(value,outline=true) {
    const block=blockState({named:'UnifiedCard'},value,outline);
    install({blocks:{languageVersion:0,blocks:[{...block,x:40,y:40}]}});
  }
  function savedWorkspace() { return {format:'mtgish.blockly.workspace.v1',presentation:4,schema:schema.version,model:payload.model,workspace:Blockly.serialization.workspaces.save(workspace)}; }
  function decodeWorkspace(value,model=schema) {
    const nodes=value.workspace?.blocks?.blocks;
    if(nodes?.length!==1)throw Error('Keep exactly one complete card record before migration.');
    // Decoding is synchronous and uses fake blocks only. Restore the editor's
    // current descriptor table before loading or rendering any real blocks.
    const current=types;
    try {
      types=model.types;
      const fake=s=>({type:s.type,state:s.extraState,isEnabled:()=>!s.disabled&&!s.disabledReasons?.length,getFieldValue:n=>s.fields?.[n],getInputTargetBlock:n=>{const child=s.inputs?.[n]?.block||s.inputs?.[n]?.shadow;return child?fake(child):null;},getNextBlock:()=>s.next?.block?fake(s.next.block):null});
      return fromBlock(fake(nodes[0]));
    }finally{types=current;}
  }
  async function migrateWorkspace(value) {
    const sequence=loadSequence,before=JSON.stringify(Blockly.serialization.workspaces.save(workspace));
    localStorage.setItem('mtgish.blockly.before-schema-upgrade',JSON.stringify(value));
    const legacy=payload.legacy_models[String(value.schema)];
    if(!legacy)throw Error('Unsupported saved-workspace schema '+value.schema);
    const ir=decodeWorkspace(value,legacy),r=await codeRequest('migrateIR',encode(ir));
    if(!r.valid)throw Error(r.error);
    if(sequence!==loadSequence||before!==JSON.stringify(Blockly.serialization.workspaces.save(workspace)))throw Error('The workspace changed while upgrading.');
    loadIR(parseExact(r.ir));preserveSavedDraft=false;
  }
  function restore(value) {
    if(value.schema!==schema.version)throw Error('This workspace requires a schema upgrade; use Import.');
    if(value.workspace?.blocks?.blocks?.length===1 && (value.presentation!==4||value.model!==payload.model)) {
      try {const ir=decodeWorkspace(value);localStorage.setItem('mtgish.blockly.before-authoring-v4',JSON.stringify(value));loadIR(ir);return;}catch(_){}
    }
    install(value.workspace);
  }
  workspace.addChangeListener(e=>{
    if(e.isUiEvent && e.type!==Blockly.Events.SELECTED)setTimeout(applyPresentation,0);
    if(e.type===Blockly.Events.SELECTED){ const b=workspace.getBlockById(e.newElementId); if(b){ const d=descriptor(b.state.ty),v=d?.variants?.find(v=>v.wire===b.state.variant); $('selection').replaceChildren(); const title=document.createElement('strong');title.textContent=typeName(b.state.ty)+(v?'::'+v.name:'');$('selection').append(title,document.createTextNode('  '+(v?.doc||'Connect a value of this type to a matching socket.'))); } }
    if(!e.isUiEvent && e.type!==Blockly.Events.FINISHED_LOADING) changed();
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
  $('new-card').onclick=()=>{preserveSavedDraft=false;++loadSequence;const kind=$('starter').value,v=blank(kind!=='creature');if(kind==='spell'){v.faces[0].abilities=[];v.faces[0].name='New spell';}loadIR(v,false);};
  $('fit').onclick=()=>workspace.zoomToFit();$('zoom-in').onclick=()=>workspace.zoomCenter(1);$('zoom-out').onclick=()=>workspace.zoomCenter(-1);
  $('undo').onclick=()=>workspace.undo(false);$('redo').onclick=()=>workspace.undo(true);
  $('expand').onclick=()=>{workspace.getAllBlocks(false).forEach(b=>b.setCollapsed(false));workspace.zoomToFit();};
  $('collapse').onclick=()=>{workspace.getAllBlocks(false).filter(b=>['Characteristics','Announcement','EntrySpec','MovementTerms','TargetSpec'].includes(b.state.ty.named)).forEach(b=>b.setCollapsed(true));workspace.zoomToFit();};
  $('export').onclick=()=>{
    if(!lastValid)return;
    try {if(encode(generated())!==lastSubmittedIR){changed();notice('Checking the latest edit before exporting.');return;}}
    catch(e){invalid(e.message);return;}
    download(($('card-title').textContent||'card').replace(/[^\p{L}\p{N}_-]+/gu,'_')+'.ir.json',lastValid);
  };
  $('save-workspace').onclick=()=>download('card.workspace.json',JSON.stringify(savedWorkspace(),null,2));
  $('import').onclick=()=>$('file').click();$('file').onchange=async()=>{
    const f=$('file').files[0];if(!f)return;const seq=++loadSequence;
    try {const text=await f.text();let value;if(text.trimStart().startsWith('{'))value=parseExact(text);else{const r=await codeRequest('parseCode',text);if(!r.valid)throw Error(r.error);value=parseExact(r.ir);}if(seq!==loadSequence)return;if(value.format==='mtgish.blockly.workspace.v1'){if(value.schema===schema.version){restore(value);preserveSavedDraft=false;}else await migrateWorkspace(value);}else {const r=await codeRequest('migrateIR',encode(value));if(seq!==loadSequence)return;if(!r.valid)throw Error(r.error);preserveSavedDraft=false;loadIR(parseExact(r.ir));}notice('Imported '+f.name);}catch(e){notice('Import failed: '+e.message);}finally{$('file').value='';}
  };
  searchCards();
  let restored=false;
  try {const saved=JSON.parse(localStorage.getItem('mtgish.blockly.v1'));if(saved){if(saved.schema===schema.version)restore(saved);else await migrateWorkspace(saved);restored=true;}}catch(e){preserveSavedDraft=true;notice('Saved draft could not be upgraded; the original save is preserved. '+e.message);}
  if(!restored&&!workspace.getAllBlocks(false).length) await loadCard(payload.cards.find(c=>c.name==='Lightning Bolt')||payload.cards[0]);
  $('loading').classList.add('hidden');
  // Small public inspection surface also used by the browser regression suite.
  window.cardBlocks={workspace,schema,parseExact,encode,dump,fromBlock,blockState,loadIR,generated,blank,loadCard,shard,payload,types,parts,applyPresentation,setMode,addToFlow,abilityValue,actionValue,defaultValue,ready:()=>workerReady,valid:()=>lastValid,savedWorkspace,restore,validateTexts,describeBindings:text=>codeRequest('describeBindings',text),formatDocument:text=>codeRequest('formatDocument',text),formatCode:text=>codeRequest('formatCode',text),parseCode:text=>codeRequest('parseCode',text),migrateWorkspace,migrateIR:text=>codeRequest('migrateIR',text)};
})().catch(e=>{document.getElementById('loading').textContent='Could not start demo: '+e.message;console.error(e);});
