// takt view's script. Its frame (the data read from an inline JSON element, one Vue app in
// setup() style, a nav of pages with titles and descriptions) follows the Context Lens
// dashboard's ui/app.js (MIT, see LICENSE-context-lens); the timeline and the transcript are
// takt's own. It reads only the data the page holds and fetches nothing.
const D=JSON.parse(document.getElementById('takt-view-data').textContent);
const {createApp,ref,computed,nextTick}=Vue;
createApp({setup(){
 const page=ref('timeline'),axis=ref('start'),zoom=ref(1),selected=ref(null),raw=ref(false),detail=ref(null);
 const nav=[{id:'timeline',label:'Timeline',icon:'M3 6h18M3 12h18M3 18h18M7 4v4m6 2v4m4 2v4'},{id:'runs',label:'Runs',icon:'M5 3h14v18H5zM8 7h8m-8 5h8m-8 5h5'}];
 const titles={timeline:'The runs on one time axis',runs:'The runs'};
 const descriptions={timeline:'One lane per run; each operation is a mark at its time, naming its step, action and outcome. Click a mark for its step and, for a harness step, its transcript.',runs:'What each trace\'s header says about its run.'};
 const pageTitle=computed(()=>titles[page.value]),pageDescription=computed(()=>descriptions[page.value]);
 const axes=[{id:'start',label:'From each run\'s start'},{id:'wall',label:'Wall clock'}];
 const actors=['harness','validator','forge','takt','human','idle'];
 const runs=D.runs.map((r,i)=>({...r,key:i,epoch:r.t0?Date.parse(r.t0)/1000:null}));
 const dirName=computed(()=>{const p=(D.dir||'').split('/').filter(Boolean);return p.length?p[p.length-1]:D.dir});
 const epoch0=Math.min(...runs.map(r=>r.epoch).filter(e=>e!==null&&!Number.isNaN(e)),Infinity);
 const offset=r=>axis.value==='wall'&&r.epoch!==null&&Number.isFinite(epoch0)&&!Number.isNaN(r.epoch)?r.epoch-epoch0:0;
 const start=op=>op.t_start??op.t_end??0, end=op=>op.t_end??op.t_start??0;
 const span=computed(()=>{let m=0;for(const r of runs){const o=offset(r);for(const op of r.ops)m=Math.max(m,o+end(op));if(r.wall_s)m=Math.max(m,o+r.wall_s)}return m>0?m:1});
 const pct=t=>Math.max(0,Math.min(100,t/span.value*100));
 const isInstant=op=>end(op)-start(op)<=0;
 const markStyle=(r,op)=>{const o=offset(r),l=pct(o+start(op));return isInstant(op)?{left:l+'%'}:{left:l+'%',width:Math.max(0,pct(o+end(op))-l)+'%'}};
 const niceStep=s=>{const raw=s/8,steps=[1,2,5,10,15,30,60,120,300,600,900,1800,3600,7200,14400,28800,86400];return steps.find(x=>x>=raw)||Math.ceil(raw/86400)*86400};
 const ticks=computed(()=>{const s=span.value,step=niceStep(s/zoom.value),out=[];for(let t=0;t<=s+1e-9;t+=step)out.push({t,left:pct(t),label:fmtClock(t)});return out});
 function fmtClock(t){t=Math.round(t);const h=Math.floor(t/3600),m=Math.floor(t%3600/60),s=t%60;return h?`${h}:${String(m).padStart(2,'0')}:${String(s).padStart(2,'0')}`:`${m}:${String(s).padStart(2,'0')}`}
 function fmtDuration(s){if(s===null||s===undefined)return '-';if(s<60)return `${Math.round(s*10)/10}s`;if(s<3600)return `${Math.floor(s/60)}m ${Math.round(s%60)}s`;return `${Math.floor(s/3600)}h ${Math.round(s%3600/60)}m`}
 function fmtBytes(n){if(n<1024)return `${n} B`;if(n<1048576)return `${(n/1024).toFixed(1)} KiB`;return `${(n/1048576).toFixed(1)} MiB`}
 const axisNote=computed(()=>axis.value==='wall'?'Each lane starts at its run\'s first instant on one wall clock; the axis is time since the earliest run began.':'Each lane starts at 0, its run\'s first instant; the axis is time since then.');
 const outcomeClass=o=>{if(!o)return '';const v=String(o);if(['pass','Completed','merged','changed','opened','pushed'].includes(v))return 'ok';if(['fail','refused','error','StepRefused','NoProgress','failed','DiskLow'].includes(v)||v.startsWith('error'))return 'bad';if(['pending','approval','ApprovalRequired','Waiting','unchanged'].includes(v))return 'wait';return ''};
 const totalOps=runs.reduce((n,r)=>n+r.ops.length,0),totalSteps=runs.reduce((n,r)=>n+(r.steps||0),0);
 const harnessSteps=runs.reduce((n,r)=>n+r.ops.filter(op=>op.actor==='harness').length,0),withTranscript=runs.reduce((n,r)=>n+r.ops.filter(op=>op.actor==='harness'&&op.transcript!==null).length,0);
 const longest=runs.reduce((m,r)=>Math.max(m,r.wall_s||0),0);
 const outcomeSummary=computed(()=>{const c={};for(const r of runs)c[r.outcome]=(c[r.outcome]||0)+1;return Object.entries(c).map(([k,v])=>`${v} ${k}`).join(', ')});
 const isSelected=(r,op)=>selected.value&&selected.value.run.key===r.key&&selected.value.op.seq===op.seq;
 function select(r,op){selected.value={run:r,op};raw.value=false;if(history.replaceState)history.replaceState(null,'',`#run=${r.key}&op=${op.seq}`);nextTick(()=>{if(detail.value&&detail.value.scrollIntoView)detail.value.scrollIntoView({behavior:'smooth',block:'start'})})}
 const transcript=computed(()=>{const s=selected.value;if(!s||s.op.transcript===null||s.op.transcript===undefined)return null;return D.transcripts[s.op.transcript]||null});
 const opFields=computed(()=>{const op=selected.value&&selected.value.op;if(!op)return [];const f=[];const add=(k,v)=>{if(v!==null&&v!==undefined&&v!==''&&!(Array.isArray(v)&&!v.length))f.push([k,Array.isArray(v)?v.join(', '):typeof v==='object'?JSON.stringify(v):String(v)])};
  add('step',op.step);add('run',op.run);add('action',op.action);add('name',op.name);add('outcome',op.outcome);add('intent',op.intent);add('evidence',op.evidence);add('label',op.label);
  add('time',op.t_start!=null?`${fmtClock(op.t_start)} to ${fmtClock(op.t_end??op.t_start)} (${fmtDuration((op.t_end??op.t_start)-op.t_start)})`:null);
  add('model',op.model);add('tokens',op.tokens_sum!=null?op.tokens_sum.toLocaleString():null);add('token components',op.tokens);add('cost (USD)',op.cost_usd!=null?op.cost_usd.toFixed(4):null);add('turns',op.turns);add('paths',op.paths);return f});
 const short=(v,n=4000)=>{const s=typeof v==='string'?v:JSON.stringify(v,null,1);return s&&s.length>n?s.slice(0,n)+` … (${s.length-n} more characters)`:s};
 function textOf(c){if(typeof c==='string')return c;if(Array.isArray(c))return c.map(x=>x&&x.type==='text'?x.text:short(x)).join('\n');return short(c)}
 const entries=computed(()=>{const t=transcript.value;if(!t)return [];const out=[];
  for(const line of t.text.split('\n')){if(!line.trim())continue;let e;try{e=JSON.parse(line)}catch(_){out.push({type:'line',kind:'raw',body:line});continue}
   const type=e.type||'?';
   if(type==='takt.prompt'){out.push({type:'prompt (step '+e.step+')',kind:'prompt',body:e.prompt});continue}
   if(type==='takt.cut'||type==='takt.view.cut'){out.push({type:'cut',kind:'cut',body:`${e.omitted_bytes} bytes left out here`});continue}
   if(type==='system'){out.push({type:'system'+(e.subtype?'/'+e.subtype:''),kind:'system',body:short({model:e.model,tools:e.tools,cwd:e.cwd})});continue}
   if(type==='result'){out.push({type:'result'+(e.subtype?'/'+e.subtype:''),kind:'result',body:short(e.result??e)+(e.usage?'\nusage: '+JSON.stringify(e.usage):'')});continue}
   const content=e.message&&e.message.content;
   if((type==='assistant'||type==='user')&&Array.isArray(content)){for(const c of content){
     if(c.type==='text')out.push({type,kind:type==='assistant'?'assistant':'user',body:c.text});
     else if(c.type==='thinking')out.push({type:'thinking',kind:'assistant',body:c.thinking});
     else if(c.type==='tool_use')out.push({type:'tool: '+c.name,kind:'tool',body:short(c.input)});
     else if(c.type==='tool_result')out.push({type:'tool result'+(c.is_error?' (error)':''),kind:'result',body:short(textOf(c.content))});
     else out.push({type:type+'/'+c.type,kind:'raw',body:short(c)})}continue}
   out.push({type,kind:'raw',body:short(e)})}
  return out});
 const linked=new URLSearchParams(location.hash.slice(1)),linkedRun=runs[Number(linked.get('run'))],linkedOp=linkedRun&&linkedRun.ops.find(op=>op.seq===Number(linked.get('op')));
 if(linkedOp)selected.value={run:linkedRun,op:linkedOp};
 return {D,nav,page,pageTitle,pageDescription,axes,axis,zoom,actors,runs,dirName,ticks,markStyle,isInstant,axisNote,outcomeClass,fmtDuration,fmtBytes,totalOps,totalSteps,harnessSteps,withTranscript,longest,outcomeSummary,selected,isSelected,select,transcript,opFields,entries,raw,detail}
}}).mount('#app');
