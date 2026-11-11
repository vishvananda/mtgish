/* Specific keyword starters, typed sockets and authentic schema-182 names. */
const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),{pathToFileURL}=require('node:url');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
let browser;
(async()=>{
 const target=process.argv[2]||'__build/card-blocks.html';
 browser=await chromium.launch({headless:true,...(process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE?{executablePath:process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE}:{})});
 const page=await browser.newPage(),errors=[];page.on('pageerror',e=>errors.push(e.message));
 await page.goto(/^https?:/.test(target)?target:pathToFileURL(path.resolve(target)).href,{timeout:120000});
 await page.waitForFunction(()=>window.cardBlocks?.valid(),null,{timeout:120000});
 const currentSchema=await page.evaluate(()=>window.cardBlocks.schema.version);assert.ok(currentSchema>=183);
 const code='Card{name:"Keyword sockets",properties:{card_types:[Instant]},abilities:[Spell{do:[size=BindValue{value:5},Keyword{actor:You,action:Incubate{amount:3}},Keyword{actor:You,action:Amass{amount:BoundValue(size),creature_type:Orc}}]}]}';
 await page.evaluate(async code=>{const d=window.cardBlocks,r=await d.parseCode(code);if(!r.valid)throw Error(r.error);d.loadIR(d.parseExact(r.ir),false,d.parseExact(r.document));},code);
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].name==='Keyword sockets');
 await page.evaluate(()=>{const b=window.cardBlocks.workspace.getAllBlocks(false).find(b=>b.state.ty.named==='KeywordAction'&&b.state.variant==='Incubate'),n=b.getInputTargetBlock('F_amount');if(!n.isShadow())throw Error('Incubate amount has no literal socket');n.setFieldValue('4','_VALUE');});
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions[1].args.operation.args.action.Incubate.amount.args===4);
 await page.evaluate(()=>{const b=window.cardBlocks.workspace.getAllBlocks(false).find(b=>b.state.ty.named==='KeywordAction'&&b.state.variant==='Amass');b.setFieldValue('Goblin','_F_creature_type');});
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions[2].args.operation.args.action.Amass.creature_type==='Goblin');
 const before=await page.evaluate(()=>window.cardBlocks.encode(window.cardBlocks.generated()));
 await page.evaluate(async()=>{const d=window.cardBlocks,p=d.bindings.ports(d.workspace.getTopBlocks(false)[0]).find(p=>p.definition&&d.bindings.metadata(p)?.name==='size');await d.bindings.rename(d.bindings.metadata(p).key,'army_size');});
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.bindings.document().bindings.some(b=>b.name==='army_size'));
 assert.equal(await page.evaluate(()=>window.cardBlocks.encode(window.cardBlocks.generated())),before);
 // The flat Effects menu has four disjoint effect groups and two named-action families.
 await page.locator('.blocklyToolboxCategory').filter({has:page.getByText('Effects',{exact:true})}).first().click();await page.evaluate(()=>window.cardBlocks.applyPresentation());
 const topic=await page.evaluate(()=>{const d=window.cardBlocks,rows=d.workspace.getFlyout().getWorkspace().getTopBlocks(false),effects=rows.filter(b=>b.state.pr==='effect'&&!['Keyword','KeywordEach'].includes(b.state.op));return{effects:effects.length,groups:effects.map(b=>b.getField('VARIANT').getOptions(false).map(([,name])=>name)),expected:d.types.Operation.variants.filter(v=>!(/^(Draft|Named$)/.test(v.name)||['Primitive','Form'].includes(v.name))&&!d.schema.authoring.actions[v.wire].control&&!['Keyword','KeywordEach'].includes(v.wire)).map(v=>v.wire)};});assert.equal(topic.effects,4);assert.deepEqual(topic.groups.flat().sort(),topic.expected.sort());
 const families=await page.evaluate(()=>window.cardBlocks.workspace.getFlyout().getWorkspace().getTopBlocks(false).filter(b=>['Keyword','KeywordEach'].includes(b.state.op)).map(b=>b.state.op));assert.ok(families.length>0&&families.length<=2);assert.equal(new Set(families).size,families.length);
 await page.locator('#block-search').fill('win game');assert.ok(await page.evaluate(()=>window.cardBlocks.workspace.getFlyout().getWorkspace().getAllBlocks(false).some(b=>b.state.ty.named==='KeywordAction'&&b.state.variant==='WinGame')));
 await page.locator('#block-search').fill('fight');
 const starter=await page.evaluate(()=>{const d=window.cardBlocks,b=d.workspace.getFlyout().getWorkspace().getAllBlocks(false).find(b=>b.state.ty.named==='KeywordAction'&&b.state.variant==='Fight');if(!b)throw Error('Specific Fight starter is missing');return {parent:!!b.getParent()?.previousConnection,first:!!b.getInput('F_first'),second:!!b.getInput('F_second'),amount:!!b.getInput('F_amount'),other:!!b.getInput('F_other')};});
 assert.deepEqual(starter,{parent:true,first:true,second:true,amount:false,other:false});
 await page.locator('#block-search').fill('');
 function metadataKeys(node,out=new Set()) {for(const m of Object.values(node.extraState?.bindings||{}))out.add(m.key);for(const input of Object.values(node.inputs||{})){if(input.block)metadataKeys(input.block,out);if(input.shadow)metadataKeys(input.shadow,out);}if(node.next?.block)metadataKeys(node.next.block,out);return [...out].sort();}
 const docs=fs.readFileSync(path.resolve(__dirname,'../../tests/fixtures/keyword_documents_v182.jsonl'),'utf8').trim().split('\n').map(JSON.parse);
 for(const [fixture,name] of [['fight','Abomination, Terrifying Titan'],['connive','Mask of the Schemer'],['amass','Along the Crooked Way']]) {
  const old=require('./fixtures/workspace-v6-schema182-'+fixture+'.json'),oldDoc=docs.find(d=>d.card.faces[0].name===name),oldKeys=metadataKeys(old.workspace.blocks.blocks[0]);
  const expected=await page.evaluate(async doc=>{const d=window.cardBlocks,r=await d.formatDocument(d.encode(doc));if(!r.valid)throw Error(r.error);return d.parseExact(r.document);},oldDoc);
  async function checked() {
   await page.waitForFunction(name=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].name===name&&window.cardBlocks.bindings.document().bindings.some(b=>b.name==='kept_0'),name);
   const actual=await page.evaluate(()=>{const d=window.cardBlocks;return {ir:d.encode(d.generated()),doc:d.bindings.document(),save:d.savedWorkspace(),backup:JSON.parse(localStorage.getItem('mtgish.blockly.before-schema-upgrade'))};});
   assert.deepEqual(JSON.parse(actual.ir),expected.card);
   const names=doc=>doc.bindings.map(b=>[b.kind,b.scope_name,b.name,b.explicit]).sort((a,b)=>JSON.stringify(a).localeCompare(JSON.stringify(b)));
   assert.deepEqual(names(actual.doc),names(expected));assert.deepEqual(metadataKeys(actual.save.workspace.blocks.blocks[0]),oldKeys);
   assert.equal(actual.save.schema,currentSchema);assert.deepEqual(actual.backup,old);
  }
  await page.locator('#file').setInputFiles({name:'named-keyword-workspace.json',mimeType:'application/json',buffer:Buffer.from(JSON.stringify(old))});await checked();
  await page.evaluate(old=>localStorage.setItem('mtgish.blockly.workspace.v1',JSON.stringify(old)),old);
  await page.reload();await checked();
 }
 const current=await page.evaluate(()=>JSON.stringify(window.cardBlocks.savedWorkspace())),broken=structuredClone(require('./fixtures/workspace-v6-schema182-fight.json'));let forged=false;
 function corrupt(node) {if(node.extraState?.ty?.named==='KeywordAction'&&node.extraState.variant==='Fight'){node.extraState.variant='Transform';if(node.fields?.VARIANT)node.fields.VARIANT='Transform';forged=true;return;}for(const input of Object.values(node.inputs||{})){if(input.block)corrupt(input.block);if(input.shadow)corrupt(input.shadow);}if(node.next?.block)corrupt(node.next.block);}
 corrupt(broken.workspace.blocks.blocks[0]);assert.ok(forged);
 const failure=await page.evaluate(async old=>{try{await window.cardBlocks.migrateWorkspace(old);return null;}catch(e){return e.message;}},broken);
 assert.match(failure,/invalid parameters|keyword/i);assert.equal(await page.evaluate(()=>JSON.stringify(window.cardBlocks.savedWorkspace())),current);
 for(const old of fs.readFileSync(path.resolve(__dirname,'../../tests/fixtures/keyword_subject_documents_v182.jsonl'),'utf8').trim().split('\n').map(JSON.parse)) {
  const upgraded=await page.evaluate(async old=>{const d=window.cardBlocks,r=await d.formatDocument(d.encode(old));if(!r.valid)throw Error(r.error);return {document:d.parseExact(r.document),code:r.code};},old);
  assert.deepEqual(upgraded.document.bindings,old.bindings);assert.match(upgraded.code,/other_creature/);
  const oldArgs=old.card.faces[0].abilities[0].args.body.instructions[0].args.operation.args;
  const newArgs=upgraded.document.card.faces[0].abilities[0].args.body.instructions[0].args.operation.args;
  assert.deepEqual(newArgs.action[oldArgs.action].subject,oldArgs.object);
 }
 assert.deepEqual(errors,[]);console.log(JSON.stringify({specificKeywordStarters:true,typedAmountSocket:true,creatureTypePicker:true,creatorNamePreserved:true,named182Import:true,named182Startup:true,bindingKeysRetained:true,failedUpgradeKeepsWorkspace:true,optionalSubjectDocuments182:true,errors},null,2));
 await browser.close();
})().catch(async e=>{console.error(e);if(browser)await browser.close();process.exitCode=1;});
