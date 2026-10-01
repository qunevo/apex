/** Author the reviewed synthetic Excel baseline using the Codex artifact runtime. */
import fs from 'node:fs/promises';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {Workbook, SpreadsheetFile} from '@oai/artifact-tool';

const here=path.dirname(fileURLToPath(import.meta.url));
const demo=path.dirname(here);
const source=JSON.parse(await fs.readFile(path.join(demo,'.local/seed.json'),'utf8'));
const output=path.join(here,'production-planning.xlsx');
const previews=path.join(demo,'.local/workbook-review');
await fs.mkdir(previews,{recursive:true});
const wb=Workbook.create();
const plan=wb.worksheets.add('Dispatch plan');
const skills=wb.worksheets.add('Skills');
const roster=wb.worksheets.add('Shift roster');
const orders=wb.worksheets.add('MES orders');
const material=wb.worksheets.add('Material');
const green='#254C3B', text='#334D3C', muted='#7C8E7E';
const excelDate=s=>(Date.parse(s+'Z')-Date.UTC(1899,11,30))/86400000;
function frame(sheet,title,note,headers,rows,widths){
  const count=rows.length, n=headers.length;
  const area=sheet.getRangeByIndexes(0,0,count+7,n);
  area.format.font={name:'Arial',size:10,color:text};
  area.format.rowHeight=22;
  area.format.verticalAlignment='center';
  sheet.showGridLines=false;
  sheet.getCell(1,0).values=[[title]];
  sheet.getCell(1,0).format.font={size:16,bold:true,color:green};
  sheet.getCell(2,0).values=[[note]];
  sheet.getCell(2,0).format.font={size:10,italic:true,color:muted};
  sheet.getRangeByIndexes(6,0,1,n).values=[headers];
  sheet.getRangeByIndexes(7,0,count,n).values=rows;
  const header=sheet.getRangeByIndexes(6,0,1,n);
  header.format={fill:green,font:{name:'Arial',size:10,bold:true,color:'#FFFFFF'},rowHeight:34,wrapText:true,horizontalAlignment:'center'};
  for(let i=0;i<n;i++)sheet.getRangeByIndexes(0,i,count+7,1).format.columnWidth=widths[i]||19;
  const table=sheet.tables.add(`A7:${String.fromCharCode(64+n)}${count+7}`,true,sheet.name.replaceAll(' ','')+'Table');
  table.style='TableStyleLight1';
  sheet.freezePanes.freezeRows(7);
  return count+7;
}
const end=frame(plan,'Northstar production planning · Weeks 41–42',
  'Synthetic baseline · 05 Oct 2026, 10:00 CEST · MES snapshot revision 0 · Source IDs stay fixed when sorting.',
  ['Operation ID','Lot','Order','Article','Qty (pcs)','Operation','Workplace','Sequence','Person','Planned start','Setup (min)','Run (min)','Planned finish','Ship due','Past ship due (h)','Fixed','Attendance (min)','Person released','Priority','Planner note'],
  source.plan.map(p=>[p.operation_id,p.lot_id,p.order_id,p.item_id,p.quantity,p.operation,p.machine_id,p.sequence,p.person_id,excelDate(p.start),p.setup_minutes,p.run_minutes,null,null,null,p.fixed,p.attendance_minutes,null,null,p.note]),
  [22,13,15,15,12,27,15,12,12,22,14,14,22,22,18,12,19,22,14,37]);
plan.tabColor=green;
plan.getRange('A5').values=[['Operations']];plan.getRange('B5').formulas=[[`=COUNTA(A8:A${end})`]];
plan.getRange('D5').values=[['Fixed operations']];plan.getRange('F5').formulas=[[`=COUNTIFS(P8:P${end},"Yes")`]];
plan.getRange('J5').values=[['Editable planning fields have pale amber fill. Dates use local plant time.']];
for(const col of ['G','H','I','J','K','L','P','Q','T'])plan.getRange(`${col}8:${col}${end}`).format.fill='#FFF9E9';
plan.getRange(`M8:M${end}`).formulas=source.plan.map((_,i)=>[`=J${i+8}+(K${i+8}+L${i+8})/1440`]);
plan.getRange(`N8:N${end}`).formulas=source.plan.map((_,i)=>[`=VLOOKUP(C${i+8},'MES orders'!$A$8:$H$${source.records.orders.length+7},5,FALSE)`]);
plan.getRange(`O8:O${end}`).formulas=source.plan.map((_,i)=>[`=MAX(0,(M${i+8}-N${i+8})*24)`]);
plan.getRange(`R8:R${end}`).formulas=source.plan.map((_,i)=>[`=J${i+8}+Q${i+8}/1440`]);
plan.getRange(`S8:S${end}`).formulas=source.plan.map((_,i)=>[`=VLOOKUP(C${i+8},'MES orders'!$A$8:$H$${source.records.orders.length+7},6,FALSE)`]);
for(const col of ['J','M','N','R']){
  plan.getRange(`${col}8:${col}${end}`).setNumberFormat('dd mmm yyyy hh:mm');
  plan.getRange(`${col}8:${col}${end}`).format.horizontalAlignment='center';
}
for(const col of ['E','H','K','L','Q'])plan.getRange(`${col}8:${col}${end}`).setNumberFormat('0');
plan.getRange(`O8:O${end}`).setNumberFormat('0.0');
plan.getRange(`P8:P${end}`).dataValidation={rule:{type:'list',values:['Yes','No']}};
plan.getRange(`I8:I${end}`).dataValidation={rule:{type:'list',formula1:"'Skills'!$A$8:$A$31"}};
plan.getRange(`O8:O${end}`).conditionalFormats.add('cellIs',{operator:'greaterThan',formula:0,format:{fill:'#FCEAE4',font:{color:'#A84735'}}});
plan.getRange(`P8:P${end}`).conditionalFormats.add('containsText',{text:'Yes',format:{fill:'#F5E9C8',font:{color:'#8B6527'}}});

const skillNames=['Setup','Mechanical','Precision','Electrical','Calibration','Testing'];
const skillEnd=frame(skills,'Qualification matrix','Owned by production planning · Yes means authorized for that activity · All people are fictional.',
  ['Person','Name',...skillNames],source.qualifications.map(p=>[p.person_id,p.name,...skillNames.map(k=>p[k]?'Yes':'No')]),[13,24,17,17,17,17,17,17]);
skills.getRange(`C8:H${skillEnd}`).format.fill='#FFF9E9';
skills.getRange(`C8:H${skillEnd}`).dataValidation={rule:{type:'list',values:['Yes','No']}};
skills.getRange(`C8:H${skillEnd}`).conditionalFormats.add('containsText',{text:'Yes',format:{fill:'#EAF3E9',font:{color:'#39704C'}}});
skills.getRange(`C8:H${skillEnd}`).format.horizontalAlignment='center';

const shifts=Object.fromEntries(source.records.shifts.map(s=>[s.id,s]));
const time=s=>{const [h,m]=s.split(':').map(Number);return(h*60+m)/1440;};
const rosterEnd=frame(roster,'Shift roster','MES attendance snapshot · Monday to Friday · 05–16 Oct 2026 · One person cannot cover two simultaneous activities.',
  ['Person','Name','Team','Shift','Shift start','Shift end','Break start','Break end','Attendance','Net hours / day'],
  source.records.personnel.map(p=>{const s=shifts[p.shift_id];return[p.id,p.name,p.team,s.name,time(s.start),time(s.end),time(s.break_start),time(s.break_end),p.attendance,null];}),
  [13,24,19,21,16,16,16,16,18,20]);
roster.getRange(`E8:H${rosterEnd}`).setNumberFormat('hh:mm');
roster.getRange(`E8:H${rosterEnd}`).format.horizontalAlignment='center';
roster.getRange(`J8:J${rosterEnd}`).formulas=source.records.personnel.map((_,i)=>[`=(F${i+8}-E${i+8}-H${i+8}+G${i+8})*24`]);
roster.getRange(`J8:J${rosterEnd}`).setNumberFormat('0.0');

const orderEnd=frame(orders,'MES order snapshot','Source: Northstar MES seed, revision 0 · This export is a fixed source snapshot. Refresh deliberately after MES changes.',
  ['Order','Customer','Article','Quantity (pcs)','Ship due','Priority','MES status','Delivery note'],
  source.records.orders.map(o=>[o.id,o.customer,o.item_id,o.quantity,excelDate(o.due),o.priority,o.status,o.note]),
  [18,19,18,18,24,16,18,35]);
orders.getRange(`E8:E${orderEnd}`).setNumberFormat('dd mmm yyyy hh:mm');
orders.getRange(`E8:E${orderEnd}`).format.horizontalAlignment='center';

const materialRows=[...source.records.materials.map(m=>[m.id,m.name,'Opening stock',m.stock,excelDate(source.factory.plan_start),m.location,'Usable stock before the plan starts']),
  ...source.records.receipts.map(r=>[r.material_id,source.records.materials.find(m=>m.id===r.material_id).name,'Confirmed receipt',r.quantity,excelDate(r.available_at),r.id,r.note])];
const materialEnd=frame(material,'Material availability','MES opening balance and confirmed receipts · Quantities are pieces · No automatic replenishment or procurement.',
  ['Material','Description','Supply type','Quantity (pcs)','Available from','Location / receipt','Note'],materialRows,[19,30,24,19,24,25,43]);
material.getRange(`E8:E${materialEnd}`).setNumberFormat('dd mmm yyyy hh:mm');
material.getRange(`E8:E${materialEnd}`).format.horizontalAlignment='center';

// Recalculate after all source cells exist; verify representative dependent edits.
wb.recalculate();
const firstExpected=excelDate(source.plan[0].end);
const initial=plan.getRange('M8').values[0][0];
if(Math.abs(initial-firstExpected)>1e-8)throw new Error('Finish formula differs from the source baseline');
const run=plan.getRange('L8').values[0][0];
plan.getRange('L8').values=[[run+15]];
if(Math.abs(plan.getRange('M8').values[0][0]-initial-15/1440)>1e-8)throw new Error('Finish did not react to a duration edit');
plan.getRange('L8').values=[[run]];
wb.recalculate();
console.log((await wb.inspect({kind:'table',range:'Dispatch plan!J7:T10',include:'values,formulas',tableMaxRows:4,tableMaxCols:11,maxChars:1600})).ndjson);
const errors=await wb.inspect({kind:'match',searchTerm:'#REF!|#DIV/0!|#VALUE!|#NAME\\?|#N/A|#NUM!|#NULL!',options:{useRegex:true,maxResults:10},maxChars:1500});
console.log(errors.ndjson);
await fs.writeFile(path.join(previews,'formula-errors.json'),errors.ndjson);
for(const [sheet,range,name] of [[plan,'A1:J17','dispatch'],[plan,'J7:T18','dispatch-formulas'],[skills,'A1:H19','skills'],[roster,'A1:J17','roster'],[orders,'A1:H17','orders'],[material,'A1:G16','material']]){
  const blob=await wb.render({sheetName:sheet.name,range,scale:1.5,format:'png'});
  await fs.writeFile(path.join(previews,name+'.png'),new Uint8Array(await blob.arrayBuffer()));
}
const xlsx=await SpreadsheetFile.exportXlsx(wb);await xlsx.save(output);
await fs.rename(output+'.inspect.ndjson',path.join(previews,'export-inspect.ndjson')).catch(error=>{if(error.code!=='ENOENT')throw error;});
console.log(JSON.stringify({output,operations:source.plan.length,sheets:5,formulaMutationCheck:'passed'}));
