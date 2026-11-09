/* Current semantic names and genuine schema-184 named workspace upgrades. */
const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),{pathToFileURL}=require('node:url');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');let browser;
(async()=>{
 const input=process.argv[2]||'__build/card-blocks.html';browser=await chromium.launch({headless:true,...(process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE?{executablePath:process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE}:{})});
 const page=await browser.newPage(),errors=[];page.on('pageerror',e=>errors.push(e.message));await page.goto(/^https?:/.test(input)?input:pathToFileURL(path.resolve(input)).href,{timeout:120000});await page.waitForFunction(()=>window.cardBlocks?.valid(),null,{timeout:120000});
 const epoch=await page.evaluate(()=>window.cardBlocks.schema.version);assert.ok(epoch>=185);
 assert.deepEqual(await page.evaluate(()=>{const t=window.cardBlocks.schema.types;return [!!t.TargetSlot,!!t.LoopBinding,!!t.Slot,!!t.Binding,t.Layout.variants.filter(v=>v.wire!=='Extension').length];}),[true,true,false,false,18]);
 const code='Card{name:"Names",properties:{card_types:[Instant]},abilities:[Spell{do:[Tap{objects:One(This),tapped:true}]}]}';
 const parsed=await page.evaluate(code=>window.cardBlocks.parseCode(code),code);assert.equal(parsed.valid,true,parsed.error);assert.equal(JSON.parse(parsed.ir).faces[0].abilities[0].args.body.instructions[0].args.operation.kind,'SetTapped');
 const formatted=await page.evaluate(ir=>window.cardBlocks.formatCode(ir),parsed.ir);assert.match(formatted.code,/SetTapped/);
 function keys(node,out=new Set()){for(const m of Object.values(node.extraState?.bindings||{}))out.add(m.key);for(const v of Object.values(node.inputs||{})){if(v.block)keys(v.block,out);if(v.shadow)keys(v.shadow,out);}if(node.next?.block)keys(node.next.block,out);return [...out].sort();}
 const cases=require('./fixtures/workspaces-api-v6-schema184.json');assert.equal(cases.length,6);
 for(const {name,document,workspace:old} of cases) {
  const expected=await page.evaluate(async doc=>{const d=window.cardBlocks,r=await d.formatDocument(d.encode(doc));if(!r.valid)throw Error(r.error);return d.parseExact(r.document);},document),oldKeys=keys(old.workspace.blocks.blocks[0]);
  async function checked(){await page.waitForFunction(expected=>{const d=window.cardBlocks;if(!d?.valid())return false;const c=d.generated();return c.layout===expected.card.layout&&JSON.stringify(c.faces.map(f=>f.name))===JSON.stringify(expected.card.faces.map(f=>f.name));},expected);
   const actual=await page.evaluate(()=>{const d=window.cardBlocks;return {ir:d.encode(d.generated()),doc:d.bindings.document(),save:d.savedWorkspace(),backup:JSON.parse(localStorage.getItem('mtgish.blockly.before-schema-upgrade'))};});
   assert.deepEqual(JSON.parse(actual.ir),expected.card,name);assert.deepEqual(actual.doc.bindings,expected.bindings,name);assert.deepEqual(keys(actual.save.workspace.blocks.blocks[0]),oldKeys,name);assert.equal(actual.save.schema,epoch);assert.deepEqual(actual.backup,old);
  }
  await page.locator('#file').setInputFiles({name:'old-named.workspace.json',mimeType:'application/json',buffer:Buffer.from(JSON.stringify(old))});await checked();
  await page.evaluate(old=>localStorage.setItem('mtgish.blockly.v1',JSON.stringify(old)),old);await page.reload();await page.waitForFunction(()=>window.cardBlocks?.ready(),null,{timeout:120000});await checked();
 }
 const before=await page.evaluate(()=>JSON.stringify(window.cardBlocks.savedWorkspace())),bad=structuredClone(cases[0].workspace);bad.schema=0;
 const error=await page.evaluate(async old=>{try{await window.cardBlocks.migrateWorkspace(old);return null;}catch(e){return e.message;}},bad);assert.match(error,/Unsupported saved-workspace schema/);assert.equal(await page.evaluate(()=>JSON.stringify(window.cardBlocks.savedWorkspace())),before);
 assert.deepEqual(errors,[]);console.log(JSON.stringify({schema:epoch,knownLayouts:18,currentIdTypes:true,currentActionNames:true,named184Import:6,named184Startup:6,bindingKeysPreserved:true,failedUpgradePreservesDraft:true,errors},null,2));await browser.close();
})().catch(async e=>{console.error(e);if(browser)await browser.close();process.exitCode=1;});
