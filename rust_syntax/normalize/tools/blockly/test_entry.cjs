/* Typed entry sockets and authentic schema-181 saves, including private programs. */
const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),{pathToFileURL}=require('node:url');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
let browser;
(async()=>{
 const target=process.argv[2]||'__build/card-blocks.html';
 browser=await chromium.launch({headless:true,...(process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE?{executablePath:process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE}:{})});
 const page=await browser.newPage(),errors=[];page.on('pageerror',e=>errors.push(e.message));
 await page.goto(/^https?:/.test(target)?target:pathToFileURL(path.resolve(target)).href,{timeout:120000});
 await page.waitForFunction(()=>window.cardBlocks?.valid(),null,{timeout:120000});
 const currentSchema=await page.evaluate(()=>window.cardBlocks.schema.version);assert.ok(currentSchema>=182);
 const code='Card{name:"Entry sockets",properties:{card_types:[Instant]},abilities:[Spell{do:[size=BindValue{value:5},TurnFaceDownAs{objects:One(This),as_creature:{power:BoundValue(size),toughness:3}}]}]}';
 await page.evaluate(async code=>{const d=window.cardBlocks,r=await d.parseCode(code);if(!r.valid)throw Error(r.error);d.loadIR(d.parseExact(r.ir),false,d.parseExact(r.document));},code);
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].name==='Entry sockets');
 await page.evaluate(()=>{const b=window.cardBlocks.workspace.getAllBlocks(false).find(b=>b.state.ty.named==='FaceDownAs'),n=b.getInputTargetBlock('F_toughness');if(!n.isShadow())throw Error('Toughness has no literal socket');n.setFieldValue('4','_VALUE');});
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions[1].args.operation.args.as_creature.toughness.args===4);
 const before=await page.evaluate(()=>window.cardBlocks.encode(window.cardBlocks.generated()));
 await page.evaluate(async()=>{const d=window.cardBlocks,p=d.bindings.ports(d.workspace.getTopBlocks(false)[0]).find(p=>p.definition&&d.bindings.metadata(p)?.name==='size');await d.bindings.rename(d.bindings.metadata(p).key,'creature_size');});
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.bindings.document().bindings.some(b=>b.name==='creature_size'));
 assert.equal(await page.evaluate(()=>window.cardBlocks.encode(window.cardBlocks.generated())),before);
 function metadataKeys(node,out=new Set()) {for(const m of Object.values(node.extraState?.bindings||{}))out.add(m.key);for(const input of Object.values(node.inputs||{})){if(input.block)metadataKeys(input.block,out);if(input.shadow)metadataKeys(input.shadow,out);}if(node.next?.block)metadataKeys(node.next.block,out);return [...out].sort();}
 const docs=fs.readFileSync(path.resolve(__dirname,'../../tests/fixtures/entry_documents_v181.jsonl'),'utf8').trim().split('\n').map(JSON.parse);
 for(const [fixture,name] of [['magar','Magar of the Magic Strings'],['return','Lagrella, the Magpie']]) {
  const old=require('./fixtures/workspace-v6-schema181-'+fixture+'.json'),oldDoc=docs.find(d=>d.card.faces[0].name===name),oldKeys=metadataKeys(old.workspace.blocks.blocks[0]);
  const expected=await page.evaluate(async doc=>{const d=window.cardBlocks,r=await d.formatDocument(d.encode(doc));if(!r.valid)throw Error(r.error);return d.parseExact(r.document);},oldDoc);
  async function checked() {
   await page.waitForFunction(name=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].name===name&&window.cardBlocks.bindings.document().bindings.some(b=>b.name==='kept_0'),name);
   const actual=await page.evaluate(()=>{const d=window.cardBlocks;return {ir:d.encode(d.generated()),doc:d.bindings.document(),save:d.savedWorkspace(),backup:JSON.parse(localStorage.getItem('mtgish.blockly.before-schema-upgrade'))};});
   assert.deepEqual(JSON.parse(actual.ir),expected.card);
   const names=doc=>doc.bindings.map(b=>[b.kind,b.scope_name,b.name,b.explicit]).sort((a,b)=>JSON.stringify(a).localeCompare(JSON.stringify(b)));
   assert.deepEqual(names(actual.doc),names(expected));assert.deepEqual(metadataKeys(actual.save.workspace.blocks.blocks[0]),oldKeys);
   assert.equal(actual.save.schema,currentSchema);assert.deepEqual(actual.backup,old);
  }
  await page.locator('#file').setInputFiles({name:'named-entry-workspace.json',mimeType:'application/json',buffer:Buffer.from(JSON.stringify(old))});
  await checked();
  if(fixture==='return') assert.equal(await page.evaluate(()=>{const b=window.cardBlocks.workspace.getAllBlocks(false).find(b=>b.state.ty.named==='EntryState'&&b.state.variant==='ExiledForWith');return b.getInputTargetBlock('F_body')?.state.ty.named;}),'Instruction');
  await page.evaluate(old=>localStorage.setItem('mtgish.blockly.workspace.v1',JSON.stringify(old)),old);
  await page.reload();await checked();
 }
 const current=await page.evaluate(()=>JSON.stringify(window.cardBlocks.savedWorkspace()));
 const broken=structuredClone(require('./fixtures/workspace-v6-schema181-magar.json'));let forged=false;
 function corrupt(node) {if(node.extraState?.variant==='Term'&&node.fields?._F_family==='PT'){node.fields._F_family='InventedPT';forged=true;return;}for(const input of Object.values(node.inputs||{})){if(input.block)corrupt(input.block);if(input.shadow)corrupt(input.shadow);}if(node.next?.block)corrupt(node.next.block);}
 corrupt(broken.workspace.blocks.blocks[0]);assert.ok(forged);
 const failure=await page.evaluate(async old=>{try{await window.cardBlocks.migrateWorkspace(old);return null;}catch(e){return e.message;}},broken);
 assert.match(failure,/power.toughness|PT|family/i);assert.equal(await page.evaluate(()=>JSON.stringify(window.cardBlocks.savedWorkspace())),current);
 assert.deepEqual(errors,[]);console.log(JSON.stringify({typedEntrySockets:true,creatorNamePreserved:true,named181Import:true,named181Startup:true,bindingKeysRetained:true,privateReturnProgram:true,grantedAbilityScopesRetained:true,failedUpgradeKeepsWorkspace:true,errors},null,2));
 await browser.close();
})().catch(async e=>{console.error(e);if(browser)await browser.close();process.exitCode=1;});
