/* Typed copy editing and a real schema-180 save with names in a granted rule. */
const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),{pathToFileURL}=require('node:url');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
let browser;
(async()=>{
 const target=process.argv[2]||'__build/card-blocks.html';
 browser=await chromium.launch({headless:true,...(process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE?{executablePath:process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE}:{})});
 const page=await browser.newPage(),errors=[];page.on('pageerror',e=>errors.push(e.message));
 await page.goto(/^https?:/.test(target)?target:pathToFileURL(path.resolve(target)).href,{timeout:120000});
 await page.waitForFunction(()=>window.cardBlocks?.valid(),null,{timeout:120000});
 const code='Card{name:"Typed copies",properties:{card_types:[Instant]},abilities:[Spell{targets:[Target{name:victim,select:Creature}],do:[size=BindValue{value:4},CreateTokens{players:You,tokens:[{amount:1,words:CreatingEffect,recipe:Copy{object:Target{slot:victim,index:0},exceptions:[SetPowerToughness{power:3,toughness:BoundValue(size)},AddAbilities([Keyword(Haste)])]}}]}]}]}';
 await page.evaluate(async code=>{const d=window.cardBlocks,r=await d.parseCode(code);if(!r.valid)throw Error(r.error);d.loadIR(d.parseExact(r.ir),false,d.parseExact(r.document));},code);
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].name==='Typed copies');
 await page.evaluate(()=>{const d=window.cardBlocks,b=d.workspace.getAllBlocks(false).find(b=>b.state.ty.named==='CopyException'&&b.state.variant==='SetPowerToughness'),p=b.getInputTargetBlock('F_power');if(!p.isShadow())throw Error('Copy amount did not receive a literal socket');p.setFieldValue('5','_VALUE');});
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.workspace.getAllBlocks(false).some(b=>b.state.ty.named==='CopyException'&&b.getInputTargetBlock('F_power')?.getFieldValue('_VALUE')==='5'));
 // Toolbox exceptions have the same typed statement connection as list items.
 await page.locator('#block-search').fill('keep name');
 const pin=await page.evaluate(()=>{const d=window.cardBlocks,b=d.workspace.getFlyout().getWorkspace().getAllBlocks(false).find(b=>b.state.ty.named==='CopyException'&&b.state.variant==='KeepName');if(!b)throw Error('Copy exception is missing from search');return {previous:!!b.previousConnection,output:!!b.outputConnection};});
 assert.deepEqual(pin,{previous:true,output:false});
 await page.locator('#block-search').fill('');
 const before=await page.evaluate(()=>window.cardBlocks.encode(window.cardBlocks.generated()));
 await page.evaluate(async()=>{const d=window.cardBlocks,p=d.bindings.ports(d.workspace.getTopBlocks(false)[0]).find(p=>p.definition&&d.bindings.metadata(p)?.name==='size');await d.bindings.rename(d.bindings.metadata(p).key,'copy_size');});
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.bindings.document().bindings.some(b=>b.name==='copy_size'));
 assert.equal(await page.evaluate(()=>window.cardBlocks.encode(window.cardBlocks.generated())),before);
 const formatted=await page.evaluate(async()=>{const d=window.cardBlocks,r=await d.formatDocument(d.encode(d.bindings.document()));if(!r.valid)throw Error(r.error);return r.code;});
 assert.match(formatted,/SetPowerToughness/);assert.match(formatted,/BoundValue\(copy_size\)/);assert.doesNotMatch(formatted,/TokenCopyEffects|CopyEffects|family:/);
 const wrong=code.replace('size=BindValue{value:4}','size=DrawCards{players:You,amount:1}').replace('SetPowerToughness{power:3,toughness:BoundValue(size)}','Colors{mode:Set,colors:ManaFrom(size)}');
 const rejected=await page.evaluate(code=>window.cardBlocks.parseCode(code),wrong);assert.equal(rejected.valid,false);assert.match(rejected.error,/produced mana/);
 const old=require('./fixtures/workspace-v6-schema180-copy.json');
 const oldDoc=JSON.parse(fs.readFileSync(path.resolve(__dirname,'../../tests/fixtures/copy_documents_v180.jsonl'),'utf8').split('\n')[0]);
 const expected=await page.evaluate(async doc=>{const d=window.cardBlocks,r=await d.formatDocument(d.encode(doc));if(!r.valid)throw Error(r.error);return d.parseExact(r.document);},oldDoc);
 function metadataKeys(node,out=new Set()) {for(const m of Object.values(node.extraState?.bindings||{}))out.add(m.key);for(const input of Object.values(node.inputs||{})){if(input.block)metadataKeys(input.block,out);if(input.shadow)metadataKeys(input.shadow,out);}if(node.next?.block)metadataKeys(node.next.block,out);return [...out].sort();}
 const oldKeys=metadataKeys(old.workspace.blocks.blocks[0]);
 async function checked() {
  await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].name==='Brenard, Ginger Sculptor'&&window.cardBlocks.bindings.document().bindings.some(b=>b.name==='retained_0'));
  const actual=await page.evaluate(()=>{const d=window.cardBlocks;return {ir:d.encode(d.generated()),doc:d.bindings.document(),save:d.savedWorkspace(),backup:JSON.parse(localStorage.getItem('mtgish.blockly.before-schema-upgrade'))};});
  assert.deepEqual(JSON.parse(actual.ir),expected.card);
  const names=doc=>doc.bindings.map(b=>[b.kind,b.scope_name,b.name,b.explicit]).sort((a,b)=>JSON.stringify(a).localeCompare(JSON.stringify(b)));
  assert.deepEqual(names(actual.doc),names(expected));assert.deepEqual(metadataKeys(actual.save.workspace.blocks.blocks[0]),oldKeys);
  assert.equal(actual.save.schema,181);assert.deepEqual(actual.backup,old);return actual;
 }
 await page.locator('#file').setInputFiles({name:'named-copy-workspace.json',mimeType:'application/json',buffer:Buffer.from(JSON.stringify(old))});
 await checked();
 await page.evaluate(old=>localStorage.setItem('mtgish.blockly.workspace.v1',JSON.stringify(old)),old);
 await page.reload();await checked();
 const current=await page.evaluate(()=>JSON.stringify(window.cardBlocks.savedWorkspace()));
 const broken=structuredClone(old);let forged=false;
 function corrupt(node) {if(node.extraState?.variant==='Term'&&node.fields?._F_family==='TokenCopyEffects'){node.fields._F_family='InventedCopyEffects';forged=true;return;}for(const input of Object.values(node.inputs||{})){if(input.block)corrupt(input.block);if(input.shadow)corrupt(input.shadow);}if(node.next?.block)corrupt(node.next.block);}
 corrupt(broken.workspace.blocks.blocks[0]);assert.ok(forged);
 const failure=await page.evaluate(async old=>{try{await window.cardBlocks.migrateWorkspace(old);return null;}catch(e){return e.message;}},broken);
 assert.match(failure,/copy.exception.family|copy exception|copy-exception/i);assert.equal(await page.evaluate(()=>JSON.stringify(window.cardBlocks.savedWorkspace())),current);
 assert.deepEqual(errors,[]);console.log(JSON.stringify({typedCopySockets:true,copyToolbox:true,creatorNamePreserved:true,manaProvenanceChecked:true,named180Import:true,named180Startup:true,bindingKeysRetained:true,grantedAbilityScopesRetained:true,failedUpgradeKeepsWorkspace:true,errors},null,2));
 await browser.close();
})().catch(async e=>{console.error(e);if(browser)await browser.close();process.exitCode=1;});
