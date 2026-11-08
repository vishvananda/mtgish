/* Card blocks: schema-driven Blockly <-> exact Rust IR, with local Wasm validation. */
'use strict';
(async () => {
  const payload = JSON.parse(document.getElementById('demo-payload').textContent);
  const schema = payload.schema, types = schema.types, $ = id => document.getElementById(id);
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
  function fieldValue(t,value) {
    if(newtype(t))return fieldValue(newtype(t),value);
    if (t.named === 'bool') return value ? 'TRUE':'FALSE';
    return String(value);
  }
  function readScalar(t,text) {
    if(newtype(t))return readScalar(newtype(t),text);
    if (t.named === 'String') return text;
    if (t.named === 'bool') return text === 'TRUE';
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
      return fieldsDefault(d.fields,path);
    }
    const v = pickVariant(d,t.named), args = fieldsDefault(v.fields,path);
    return packEnum(d,v,args);
  }
  function fieldsDefault(f,path) {
    if (f.kind === 'unit') return null;
    if (f.kind === 'record') return Object.fromEntries(f.fields.map(x => [x.name,defaultValue(x.ty,path)]));
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
  function parts(state) {
    const t=state.ty,d=descriptor(t);
    if (scalar(t)) return [{key:'VALUE',label:t.named,ty:t}];
    if (t.vec) return Array.from({length:state.count},(_,i) => ({key:`ITEM_${i}`,label:`${i+1}`,ty:t.vec,statement:!scalar(t.vec)}));
    if (t.option) return state.present ? [{key:'SOME',label:'value',ty:t.option}] : [];
    const f=t.tuple ? {kind:'tuple',items:t.tuple} : d.kind==='struct' ? d.fields : d.variants.find(v => v.wire===state.variant).fields;
    if (f.kind==='unit') return [];
    if (f.kind==='record') return f.fields.map(f => ({key:`F_${f.name}`,label:pretty(f.name),ty:f.ty}));
    return f.items.map((ty,i) => ({key:`ARG_${i}`,label:f.items.length===1?'value':String(i+1),ty}));
  }
  function stateFor(t,value) {
    if (t.vec) return {ty:t,count:value.length};
    if (t.option) return {ty:t,present:value!==null};
    const v=variantFor(t,value); return {ty:t,...(v?{variant:v.wire}:{})};
  }
  function childValues(t,value,state) {
    if (scalar(t)) return {VALUE:value};
    if (t.vec) return Object.fromEntries(value.map((v,i) => [`ITEM_${i}`,v]));
    if (t.option) return value===null ? {} : {SOME:value};
    const d=descriptor(t),v=variantFor(t,value);
    const f=t.tuple ? {kind:'tuple',items:t.tuple} : v ? v.fields : d.fields;
    const a=v ? (d.adjacent ? value.args : value[v.wire]) : value;
    if (f.kind==='unit') return {};
    if (f.kind==='record') return Object.fromEntries(f.fields.map(f => [`F_${f.name}`,a[f.name]]));
    return Object.fromEntries(f.items.map((_,i) => [`ARG_${i}`,f.items.length===1 ? a : a[i]]));
  }
  function toBlock(t,value,outline = false) {
    const state=stateFor(t,value), values=childValues(t,value,state), result={type:'ir_value',extraState:state,fields:{},inputs:{}};
    for (const p of parts(state)) {
      if (values[p.key] === undefined) throw Error(`Missing ${typeName(t)}.${p.label}`);
      if (scalar(p.ty)) result.fields[p.key]=fieldValue(p.ty,values[p.key]);
      else {const child=toBlock(p.ty,values[p.key],outline);if(p.statement)child.extraState.statement=true;result.inputs[p.key]={block:child};}
    }
    if (descriptor(t)?.kind==='enum' && !scalar(t)) result.fields.VARIANT=state.variant;
    if (outline && ['Characteristics','Announcement','EntryProperties','MoveTerms','TargetSpec'].includes(t.named)) result.collapsed=true;
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
  let workspace, suppress=false, revision=0, timer, lastValid=null, lastSubmittedIR=null, workerReady=false;
  let history=[],historyIndex=-1;
  function insertDefault(parent,p) {
    try {
      const state=blockState(p.ty,defaultValue(p.ty));if(p.statement)state.extraState.statement=true;
      const b=Blockly.serialization.blocks.append(state,workspace);
      parent.getInput(p.key).connection.connect(p.statement?b.previousConnection:b.outputConnection);
      b.select();
    } catch(e) { notice(e.message); }
  }
  Blockly.Blocks.ir_value = {
    init() {
      this.state={ty:{named:'String'}}; this.setOutput(true,typeKey(this.state.ty)); this.setInputsInline(false);
      this.appendDummyInput('HEADER').appendField('IR value');
      this.setOnChange(e => {
        if (e.type===Blockly.Events.BLOCK_CHANGE && e.blockId===this.id && e.name==='VARIANT') {
          const v=this.getFieldValue('VARIANT'); if (v && v!==this.state.variant) { this.state.variant=v; this.rebuild(); }
        }
      });
    },
    saveExtraState() { return clone(this.state); },
    loadExtraState(state) {
      if (!state || !state.ty || !Number.isInteger(state.count??0) || (state.count??0)<0 || (state.count??0)>2048) throw Error('Invalid block state or a list longer than 2,048 items.');
      this.state=clone(state); this.rebuild();
    },
    rebuild() {
      const attached={}, values={};
      for (const input of this.inputList) {
        const child=input.connection?.targetBlock(); if (child) { attached[input.name]=child; child.unplug(false); }
        for (const f of input.fieldRow) if (f.name) values[f.name]=f.getValue();
      }
      Blockly.Events.disable();
      try {
        for (const input of [...this.inputList]) this.removeInput(input.name);
        const t=this.state.ty,d=descriptor(t),header=this.appendDummyInput('HEADER');
        if(this.state.statement){this.setOutput(false);this.setPreviousStatement(true,typeKey(t));}
        else {this.setPreviousStatement(false);this.setOutput(true,typeKey(t));}
        this.setColour(colour(t));
        if (t.vec) {
          header.appendField(`${typeName(t.vec)} list · ${this.state.count}`);
          header.appendField(button('+','Add list item',()=>this.resize(1)));
          if (this.state.count) header.appendField(button('−','Remove last list item',()=>this.resize(-1)));
        } else if (t.option) {
          header.appendField(`${typeName(t.option)} · ${this.state.present?'some':'none'}`);
          header.appendField(button(this.state.present?'−':'+','Toggle optional value',()=>this.resize(0)));
        } else if (d?.kind==='enum' && !scalar(t)) {
          header.appendField(new Blockly.FieldDropdown(d.variants.map(v=>[pretty(v.name),v.wire])),'VARIANT');
          this.setFieldValue(this.state.variant,'VARIANT'); header.appendField(`· ${t.named}`);
        } else header.appendField(typeName(t));
        for (const p of parts(this.state)) {
          if (scalar(p.ty)) {
            const v=values['_'+p.key] ?? fieldValue(p.ty,defaultValue(p.ty));
            this.appendDummyInput(p.key).appendField(p.label).appendField(field(p.ty,v),'_'+p.key);
            this.getField('_'+p.key).setValue(v);
          } else {
            (p.statement?this.appendStatementInput(p.key):this.appendValueInput(p.key)).setCheck(typeKey(p.ty)).appendField(p.label)
              .appendField(button('+',`Fill ${p.label}`,()=>{if(!this.getInputTargetBlock(p.key)) insertDefault(this,p);}));
            const old=attached[p.key];
            if (old && typeKey(old.state.ty)===typeKey(p.ty)) this.getInput(p.key).connection.connect(p.statement?old.previousConnection:old.outputConnection);
          }
        }
        const variant=d?.variants?.find(v=>v.wire===this.state.variant);
        this.setTooltip(`${typeName(t)}${variant ? '::'+variant.name : ''}\n${variant?.doc||''}`);
        this.setInputsInline(scalar(t) || (d?.kind==='struct' && d.fields.kind==='tuple' && d.fields.items.every(scalar)));
      } finally { Blockly.Events.enable(); }
      if (this.rendered) this.render();
    },
    resize(delta) {
      if(this.state.ty.vec && delta>0 && this.state.count>=2048){notice('This demo allows up to 2,048 items in one list.');return;}
      Blockly.Events.setGroup(true);
      try {
        if (this.state.ty.vec) {
          if (delta<0) this.getInputTargetBlock(`ITEM_${this.state.count-1}`)?.dispose(false);
          this.state.count=Math.max(0,this.state.count+delta);
        } else {
          if (this.state.present) this.getInputTargetBlock('SOME')?.dispose(false);
          this.state.present=!this.state.present;
        }
        this.rebuild();
        const p=parts(this.state).at(-1); if (p && !scalar(p.ty) && !this.getInputTargetBlock(p.key)) insertDefault(this,p);
      } finally { Blockly.Events.setGroup(false); }
      changed();
    },
  };
  function fromBlock(b) {
    if (!b || b.type!=='ir_value') throw Error('A required block is missing. Fill the empty socket with ＋.');
    if (!b.isEnabled()) throw Error('Re-enable or remove disabled blocks before exporting IR.');
    const state=b.state,t=state.ty,d=descriptor(t), values={};
    for (const p of parts(state)) {
      if (scalar(p.ty)) values[p.key]=readScalar(p.ty,b.getFieldValue('_'+p.key));
      else {
        const child=b.getInputTargetBlock(p.key);
        if (!child) throw Error(`${typeName(t)}: fill the ${p.label} socket.`);
        if (typeKey(child.state.ty)!==typeKey(p.ty)) throw Error(`Wrong type in ${p.label}`);
        values[p.key]=fromBlock(child);
      }
    }
    if (scalar(t)) return values.VALUE;
    if (t.vec) return parts(state).map(p=>values[p.key]);
    if (t.option) return state.present ? values.SOME:null;
    const v=d?.kind==='enum' ? d.variants.find(v=>v.wire===state.variant) : null;
    const f=t.tuple ? {kind:'tuple',items:t.tuple} : v ? v.fields : d.fields;
    let a=null;
    if (f.kind==='record') a=Object.fromEntries(f.fields.map(f=>[f.name,values['F_'+f.name]]));
    if (f.kind==='tuple') { a=f.items.map((_,i)=>values['ARG_'+i]); if(a.length===1) a=a[0]; }
    return v ? packEnum(d,v,a):a;
  }
  // Blockly serializes field names separately from state; use those exact names.
  function fixFieldNames(s) {
    s.fields=Object.fromEntries(Object.entries(s.fields).map(([k,v])=>[k==='VARIANT'?k:'_'+k,v]));
    Object.values(s.inputs||{}).forEach(i=>fixFieldNames(i.block)); return s;
  }
  const blockState=(t,v,outline=false)=>fixFieldNames(toBlock(t,v,outline));
  const theme=Blockly.Theme.defineTheme('cardBlocks',{base:Blockly.Themes.Classic,fontStyle:{family:'system-ui, sans-serif',size:11},componentStyles:{workspaceBackgroundColour:'#f4f6f1',scrollbarColour:'#a8b5a4',scrollbarOpacity:.6,insertionMarkerColour:'#52794c',insertionMarkerOpacity:.4}});
  workspace=Blockly.inject('workspace',{renderer:'geras',theme,toolbox:null,trashcan:false,sounds:false,zoom:{controls:false,wheel:true,startScale:.75,maxScale:1.6,minScale:.12,scaleSpeed:1.16},move:{scrollbars:true,drag:true,wheel:true},grid:{spacing:22,length:1,colour:'#d6dfd1',snap:false}});
  new ResizeObserver(()=>Blockly.svgResize(workspace)).observe($('workspace'));
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
  const workerCode = `${VALIDATOR_GLUE}\nself.onmessage=async e=>{const m=e.data;try{if(m.init){const stream=new Blob([Uint8Array.from(atob(m.init),c=>c.charCodeAt(0))]).stream().pipeThrough(new DecompressionStream('gzip'));wasm_bindgen.initSync({module:new Uint8Array(await new Response(stream).arrayBuffer())});self.postMessage({ready:true});}else if(m.batch)self.postMessage({id:m.id,batch:m.batch.map(ir=>{const r=JSON.parse(wasm_bindgen.validate_ir(ir));return {valid:r.valid,error:r.error};})});else self.postMessage({id:m.id,result:JSON.parse(wasm_bindgen.validate_ir(m.ir))});}catch(e){self.postMessage({id:m.id,error:String(e)});}};`;
  const worker=new Worker(URL.createObjectURL(new Blob([workerCode],{type:'application/javascript'})));
  const requests=new Map();let nextRequest=-1;
  function validateTexts(batch){return new Promise((resolve,reject)=>{const id=nextRequest--;requests.set(id,{resolve,reject});worker.postMessage({id,batch});});}
  worker.onmessage=e=>{
    if(requests.has(e.data.id)){const p=requests.get(e.data.id);requests.delete(e.data.id);e.data.error?p.reject(Error(e.data.error)):p.resolve(e.data.batch);return;}
    if(e.data.ready){workerReady=true; changed(); return;}
    if(e.data.id!==revision) return;
    const r=e.data.result;
    if(e.data.error || !r.valid) { invalid(e.data.error||r.error); return; }
    lastValid=r.ir; $('export').disabled=false; $('validation').className='badge valid'; $('validation').textContent='✓ Valid Rust IR';
    $('validation-detail').textContent=`Schema ${r.schema} · ${r.faces} face${r.faces===1?'':'s'} · checked in this browser with WebAssembly`;
  };
  worker.onerror=e=>{invalid('The WebAssembly worker stopped: '+e.message);for(const p of requests.values())p.reject(Error(e.message));requests.clear();};
  worker.postMessage({init:payload.wasm});
  function generated() {
    const roots=workspace.getTopBlocks(false);
    if(roots.length!==1 || roots[0].state.ty.named!=='UnifiedCard') throw Error('Keep exactly one card record, with all other blocks connected. Delete or attach the loose blocks.');
    return fromBlock(roots[0]);
  }
  function changed() {
    if(suppress) return;
    ++revision; clearTimeout(timer); invalid('Changes are checked with the native Rust validation rules.',true);
    timer=setTimeout(()=>{
      remember();
      try {
        const value=generated(),ir=encode(value); $('ir').textContent=ir;
        $('card-title').textContent=value.faces.map(f=>f.name).join(' // ')||'New card';
        $('workspace-meta').textContent=`${workspace.getAllBlocks(false).length} blocks · schema ${schema.version} · saved locally`;
        try {localStorage.setItem('mtgish.blockly.v1',JSON.stringify(savedWorkspace()));}catch(_){$('workspace-meta').textContent+=' · local save unavailable';}
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
    workspace.render();
    Blockly.renderManagement.finishQueuedRenders().then(()=>workspace.zoomToFit());
  }
  function loadIR(value,outline=true) {
    const block=blockState({named:'UnifiedCard'},value,outline);
    install({blocks:{languageVersion:0,blocks:[{...block,x:40,y:40}]}});
  }
  function savedWorkspace() { return {format:'mtgish.blockly.workspace.v1',schema:schema.version,model:payload.model,workspace:Blockly.serialization.workspaces.save(workspace)}; }
  function restore(value) {
    if(value.schema!==schema.version || value.model!==payload.model) throw Error('This workspace was saved against a different Rust model. Import its IR instead.');
    install(value.workspace);
  }
  workspace.addChangeListener(e=>{
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
      const b=document.createElement('button');b.className='result';const title=document.createElement('strong');title.textContent=c.name;const small=document.createElement('small');small.textContent=c.types||'Card';b.append(title,small);b.onclick=()=>loadCard(c);return b;
    }));
  }
  const common=['Ability','Instruction','Operation','Expr','PlayerSet','PlayerRef','ObjectSet','ObjectRef','TargetSpec','ObjectFilter','Count','CostPart','EventSpec','Duration'];
  const palette=Object.entries(types).flatMap(([n,d])=> d.kind==='enum' ? d.variants.filter(v=>!excluded(v)).map(v=>({type:n,variant:v.wire,label:pretty(v.name),doc:v.doc})):[{type:n,label:pretty(n),doc:''}]);
  function searchBlocks() {
    const q=$('block-search').value.toLowerCase(), matches=palette.filter(p=>q ? `${p.type} ${p.label} ${p.doc}`.toLowerCase().includes(q) : common.includes(p.type));
    const shown=matches.slice(0,50);$('block-count').textContent=`${matches.length} constructors · showing ${shown.length}`;
    $('block-results').replaceChildren(...shown.map(p=>{
      const b=document.createElement('button');b.className='result';const strong=document.createElement('strong');strong.textContent=p.label;const small=document.createElement('small');small.textContent=p.type;b.append(strong,small);b.title=p.doc;
      b.onclick=()=>{try{
        const t={named:p.type},d=types[p.type];let value;
        if(p.variant){const v=d.variants.find(v=>v.wire===p.variant);value=packEnum(d,v,fieldsDefault(v.fields,[]));}else value=defaultValue(t);
        const state=blockState(t,value);if(['Card','Ability','Instruction','CostPart'].includes(p.type))state.extraState.statement=true;
        const node=Blockly.serialization.blocks.append(state,workspace);node.moveBy(60,60);node.select();workspace.centerOnBlock(node.id);changed();
      }catch(e){notice(e.message);}};return b;
    }));
  }
  function blank(draw=false) {
    const v={schema_version:schema.version,layout:'Card',faces:[{schema_version:schema.version,name:draw?'New draw spell':'New creature',characteristics:{supertypes:[],card_types:[draw?'Instant':'Creature'],subtypes:[],mana_cost:[],color_indicator:[],power_toughness:draw?null:[1,1],loyalty:null,defense:null},abilities:[]}],data:[]};
    if(draw) v.faces[0].abilities=[{kind:'Spell',args:{announcement:{targets:[],modes:null},body:{instructions:[{kind:'Effect',args:{result:4294967295,operation:{kind:'Draw',args:{players:{kind:'One',args:{kind:'AbilityController'}},amount:{kind:'Constant',args:1}}}}}]}}}];
    return v;
  }
  function download(name,text) { const url=URL.createObjectURL(new Blob([text],{type:'application/json'})),a=document.createElement('a');a.href=url;a.download=name;a.click();setTimeout(()=>URL.revokeObjectURL(url),1000); }
  $('search').oninput=searchCards;$('search').onkeydown=e=>{if(e.key==='Enter')$('card-results').querySelector('button')?.click();};$('block-search').oninput=searchBlocks;
  for(const tab of ['cards','blocks']) $(tab+'-tab').onclick=()=>{for(const t of ['cards','blocks']){$(t+'-panel').classList.toggle('hidden',t!==tab);$(t+'-tab').classList.toggle('active',t===tab);$(t+'-tab').setAttribute('aria-selected',String(t===tab));}};
  $('new-card').onclick=()=>{++loadSequence;loadIR(blank($('starter').value==='draw'),false);};
  $('fit').onclick=()=>workspace.zoomToFit();$('zoom-in').onclick=()=>workspace.zoomCenter(1);$('zoom-out').onclick=()=>workspace.zoomCenter(-1);
  $('undo').onclick=()=>workspace.undo(false);$('redo').onclick=()=>workspace.undo(true);
  $('expand').onclick=()=>{workspace.getAllBlocks(false).forEach(b=>b.setCollapsed(false));workspace.zoomToFit();};
  $('collapse').onclick=()=>{workspace.getAllBlocks(false).filter(b=>['Characteristics','Announcement','EntryProperties','MoveTerms','TargetSpec'].includes(b.state.ty.named)).forEach(b=>b.setCollapsed(true));workspace.zoomToFit();};
  $('export').onclick=()=>{
    if(!lastValid)return;
    try {if(encode(generated())!==lastSubmittedIR){changed();notice('Checking the latest edit before exporting.');return;}}
    catch(e){invalid(e.message);return;}
    download(($('card-title').textContent||'card').replace(/[^\p{L}\p{N}_-]+/gu,'_')+'.ir.json',lastValid);
  };
  $('save-workspace').onclick=()=>download('card.workspace.json',JSON.stringify(savedWorkspace(),null,2));
  $('import').onclick=()=>$('file').click();$('file').onchange=async()=>{
    const f=$('file').files[0];if(!f)return;++loadSequence;
    try {const text=await f.text(),value=parseExact(text);if(value.format==='mtgish.blockly.workspace.v1')restore(value);else loadIR(value);notice('Imported '+f.name);}catch(e){notice('Import failed: '+e.message);}finally{$('file').value='';}
  };
  searchCards();searchBlocks();
  let restored=false;
  try {const saved=JSON.parse(localStorage.getItem('mtgish.blockly.v1'));if(saved){restore(saved);restored=true;}}catch(_){}
  if(!restored) await loadCard(payload.cards.find(c=>c.name==='Lightning Bolt')||payload.cards[0]);
  $('loading').classList.add('hidden');
  // Small public inspection surface also used by the browser regression suite.
  window.cardBlocks={workspace,schema,parseExact,encode,fromBlock,blockState,loadIR,generated,blank,loadCard,shard,payload,types,defaultValue,ready:()=>workerReady,valid:()=>lastValid,savedWorkspace,restore,validateTexts};
})().catch(e=>{document.getElementById('loading').textContent='Could not start demo: '+e.message;console.error(e);});
