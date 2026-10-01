import {icon} from './icons.js';

// Presentation facts describe the reproducible seed, not a changing live plan.
const chapters = [
  {id:'factory', label:'The factory', seconds:30, title:'One factory. Hundreds of competing priorities.',
    subtitle:'Northstar Valve Works makes valve assemblies to customer order. The products are familiar. Coordinating their production is the challenge.',
    takeaway:'The goal is to deliver complete orders on time, while sharing equipment, materials and people.'},
  {id:'products', label:'The products', seconds:40, title:'A common body. Three different journeys.',
    subtitle:'Distributor, regulator and sensor assemblies share the same production system. Each adds a different level of downstream work.',
    takeaway:'A customer buys a finished assembly. The planner must coordinate every lot and every required step.'},
  {id:'flow', label:'Material flow', seconds:70, title:'Two stages. Flexible resources inside each.',
    subtitle:'Order-driven production moves from body manufacturing into a flexible assembly and test area. Cleaned bodies transfer as whole lots.',
    takeaway:'The stages stay in order. Machine selection and downstream routes create the flexibility.'},
  {id:'decisions', label:'The decisions', seconds:60, title:'A free machine is only part of the answer.',
    subtitle:'A feasible start needs the preceding work, the right equipment, usable material and a qualified person at the same time.',
    takeaway:'Sequence, route, machine and person decisions are coupled. Improving one queue can delay another order.'},
  {id:'monday', label:'Monday morning', seconds:60, title:'The plan meets the shop floor.',
    subtitle:'Monday, 05 October 2026 · 10:00. Some lots are finished, some are running, and the planner is preparing the next moves.',
    takeaway:'The MES records reality. Excel holds the intended sequence. A change in one calls for a review of the other.'},
  {id:'apex', label:'The APEX handoff', seconds:40, title:'From scattered decisions to a checked plan.',
    subtitle:'The next step is to connect this source environment to APEX, preserve committed work and compare feasible alternatives.',
    takeaway:'The conversation moves from “Which row do I move?” to “Which feasible plan best meets our priorities?”'},
];

export const isShowcase = page => page === 'guide' || page === 'showcase' || page.startsWith('showcase/');
const pad = n => String(n).padStart(2, '0');
const jump = (page, label) => `<button class="sc-link" data-mes="${page}">${label} ${icon('arrow')}</button>`;
const tag = text => `<span class="sc-tag">${text}</span>`;
const svg = (label, content, viewBox = '0 0 320 190') => `<svg viewBox="${viewBox}" role="img" aria-label="${label}" xmlns="http://www.w3.org/2000/svg">${content}</svg>`;

function valve(type) {
  const top = type === 'regulator'
    ? '<path d="M133 61V26h54v35" fill="#c7d2fe"/><rect x="123" y="16" width="74" height="15" rx="5" fill="#4f46e5"/><path d="M143 38h34m-34 9h34" stroke="#818cf8" stroke-width="3"/>'
    : type === 'sensor'
      ? '<path d="M162 64V38h43V14h47v30h-32v22" fill="none" stroke="#0ea5b7" stroke-width="9" stroke-linejoin="round"/><rect x="193" y="9" width="59" height="32" rx="7" fill="#cffafe" stroke="#0891b2" stroke-width="2"/><circle cx="237" cy="25" r="5" fill="#06b6d4"/>'
      : '<path d="M138 65V39h40v26" fill="#bfdbfe"/><rect x="130" y="29" width="56" height="16" rx="5" fill="#3b82f6"/>';
  return svg(`${type} assembly, schematic illustration`, `<ellipse cx="160" cy="159" rx="112" ry="13" fill="#0f172a08"/>${top}<path d="m66 78 53-23 139 27-53 23Z" fill="#e2e8f0" stroke="#94a3b8"/><path d="M66 78v58l139 25v-56Z" fill="#cbd5e1" stroke="#94a3b8"/><path d="m205 105 53-23v58l-53 21Z" fill="#94a3b8" stroke="#64748b"/><g fill="#475569" stroke="#f8fafc" stroke-width="3"><ellipse cx="91" cy="109" rx="11" ry="14"/><ellipse cx="134" cy="117" rx="11" ry="14"/><ellipse cx="177" cy="124" rx="11" ry="14"/></g><ellipse cx="231" cy="120" rx="10" ry="14" fill="#334155"/><path d="m114 76 87 17" stroke="#fff" stroke-width="3"/><circle cx="118" cy="62" r="3" fill="#64748b"/><circle cx="237" cy="85" r="3" fill="#64748b"/>`);
}

function factory() {
  return `<div class="sc-factory-layout"><div class="sc-factory-copy"><div class="sc-kicker">THE SYNTHETIC STARTING POINT</div><h2>Make to order.<br>Plan across the whole factory.</h2><p>Every order is split into production lots. Each lot travels through machining, cleaning, assembly and mandatory final testing.</p><div class="sc-tags">${tag('Order-driven / push')}${tag('Two-stage production')}${tag('Whole-lot transfer')}</div><div class="sc-fact-grid"><div><strong>120</strong><span>customer orders</span></div><div><strong>600</strong><span>production lots</span></div><div><strong>3,800</strong><span>work operations</span></div><div><strong>24</strong><span>people on two shifts</span></div></div></div><div class="sc-factory-visual">
    ${svg('Customer orders become lots, pass through body manufacturing and assembly and test, then ship as complete orders.', `<defs><marker id="sc-factory-arrow" markerWidth="8" markerHeight="8" refX="6" refY="4" orient="auto"><path d="m1 1 5 3-5 3" fill="none" stroke="#6680e8" stroke-width="1.5"/></marker></defs><g stroke="#6680e8" stroke-width="2" marker-end="url(#sc-factory-arrow)"><path d="M195 70v23"/><path d="M195 179v27"/><path d="M195 290v30"/></g><rect x="70" y="10" width="250" height="56" rx="10" fill="#fff" stroke="#c7d2fe"/><text x="195" y="33" class="sc-svg-title" text-anchor="middle">Customer orders</text><text x="195" y="53" class="sc-svg-small" text-anchor="middle">Split into lots of 5–25 pieces</text><rect x="20" y="99" width="350" height="76" rx="12" fill="#eef2ff" stroke="#a5b4fc"/><text x="40" y="121" class="sc-svg-label">STAGE 01</text><text x="40" y="147" class="sc-svg-heading">Body manufacturing</text><text x="40" y="166" class="sc-svg-small">CNC → Deburr → Wash</text><rect x="20" y="212" width="350" height="76" rx="12" fill="#ecfeff" stroke="#67e8f9"/><text x="40" y="234" class="sc-svg-label">STAGE 02</text><text x="40" y="260" class="sc-svg-heading">Assembly &amp; test</text><text x="40" y="279" class="sc-svg-small">Variant routes · shared specialists</text><rect x="70" y="328" width="250" height="44" rx="10" fill="#fff" stroke="#c7d2fe"/><text x="195" y="356" class="sc-svg-title" text-anchor="middle">Complete shipment</text>`, '0 0 390 382')}
    <span class="sc-diagram-caption">Planning horizon · 05–16 October 2026</span></div></div>`;
}

function products() {
  const families = [
    ['distributor','D','Distributor','Distributes a fluid flow.','Seals, plugs and connectors complete the machined body.','Assembly → Leak test'],
    ['regulator','R','Regulator','Controls a pressure or flow.','A valve kit adds precision adjustment before function testing.','Assembly → Adjustment → Test'],
    ['sensor','S','Sensor assembly','Adds measurement and electronics.','Electronics and calibration add specialist work and a return to assembly.','Preassembly → Electronics → Calibration → Final assembly → Test'],
  ];
  return `<div class="sc-products">${families.map(([id,code,name,purpose,description,route])=>`<article class="sc-product sc-${id}"><div class="sc-product-art">${valve(id)}<span class="sc-product-code">${code}</span></div><div class="sc-product-copy"><h2>${name}</h2><p class="sc-product-purpose">${purpose}</p><p>${description}</p><div class="sc-product-route">${route}</div></div></article>`).join('')}</div><div class="sc-ribbon"><div><strong>18 article variants</strong><span>Family × body size × material combinations in the demo assortment</span></div><div>${tag('Compact / standard / large')}${tag('Aluminium / stainless steel')}</div></div>`;
}

const step = (label, detail='', cls='') => `<div class="sc-step ${cls}"><strong>${label}</strong>${detail?`<small>${detail}</small>`:''}</div>`;
function flow() {
  return `<div class="sc-flow-stage"><div class="sc-stage-label"><span>01</span><div><h2>Body manufacturing</h2><p>Common flow · alternative CNC resources</p></div><span class="sc-stage-meta">6 CNC · 2 deburr · 2 washers</span></div><div class="sc-stage-one">${step('Blank','Aluminium / stainless')}${icon('arrow')}${step('CNC machining','Choose an eligible cell','sc-step-blue')}${icon('arrow')}${step('Deburr & inspect')}${icon('arrow')}${step('Wash & dry','Cleaned body ready')}</div><div class="sc-route-alternative"><span>Alternative workplan</span><strong>Rough machining</strong>${icon('arrow')}<strong>Drill & thread</strong><span>replaces combined CNC, then rejoins at deburring</span></div></div>
  <div class="sc-transfer">${icon('arrow')} Cleaned bodies move to assembly only after the whole lot is ready</div>
  <div class="sc-flow-stage sc-stage-two"><div class="sc-stage-label"><span>02</span><div><h2>Assembly & quality</h2><p>Flexible job shop · variant-specific routes</p></div><span class="sc-stage-meta">8 assembly · 2 electronics · 1 calibration · 3 test</span></div><div class="sc-route-row"><span class="sc-route-family sc-distributor">D</span>${step('Assembly','Shared benches')}${icon('arrow')}${step('Leak / function test','Eligible QA bench')}<span class="sc-route-finish">Finished</span></div><div class="sc-route-row"><span class="sc-route-family sc-regulator">R</span>${step('Assembly','Shared benches')}${icon('arrow')}${step('Valve adjustment','Precision skill')}${icon('arrow')}${step('Function test','QA-02 / QA-03')}<span class="sc-route-finish">Finished</span></div><div class="sc-route-row sc-sensor-row"><span class="sc-route-family sc-sensor">S</span>${step('Preassembly','Shared benches','sc-shared')}${icon('arrow')}${step('Sensor install','Electronics')}${icon('arrow')}${step('Calibration','1 bench')}${icon('arrow')}${step('Final assembly','Back to shared benches','sc-shared')}${icon('arrow')}${step('Function test')}<span class="sc-route-finish">Finished</span></div></div><div class="sc-flow-legend"><span><i></i> Repeated assembly steps compete for the same capacity.</span><span>Machine alternatives change <strong>where</strong>; workplan alternatives change <strong>which steps</strong>.</span></div>`;
}

function decisions() {
  const constraints = [
    ['operations','Sequence','A washed body must exist before assembly. Every quality step is required.'],
    ['machines','Equipment','Size and material limit CNC eligibility. Maintenance removes dated capacity.'],
    ['personnel','People','A qualified person must be on shift and available. CNC needs attendance during setup.'],
    ['materials','Material','Sensor kits are consumed at installation. A confirmed future receipt is not stock today.'],
  ];
  return `<div class="sc-decisions"><div class="sc-decision-hub"><div class="sc-kicker">ONE OPERATION</div><h2>Can this lot<br>start here, now?</h2><div class="sc-hub-icon">${icon('operations')}</div><p>All four conditions must hold together.</p><div class="sc-hub-footer">Then choose a useful sequence.</div></div><div class="sc-constraint-grid">${constraints.map(([i,title,text],n)=>`<article><div class="sc-constraint-title">${icon(i)}<h3>${title}</h3><span>${pad(n+1)}</span></div><p>${text}</p></article>`).join('')}</div></div><div class="sc-choice-strip"><div><span>120 orders</span><strong>What comes first?</strong></div>${icon('arrow')}<div><span>Alternative routes & machines</span><strong>Where should it run?</strong></div>${icon('arrow')}<div><span>Shared qualifications</span><strong>Who is available?</strong></div></div><p class="sc-caption">The baseline uses fixed setup allowances. Sequence-dependent setup matrices are a later modeling step for this showcase.</p>`;
}

function monday() {
  return `<div class="sc-monday-grid"><div class="sc-handover"><div class="sc-kicker">THE INITIAL HANDOVER</div><h2>Keep what is committed.<br>Reconsider what can move.</h2><ol class="sc-events"><li><span class="sc-event-dot blue"></span><div><strong>Work is already in motion</strong><p>Completed and running operations are recorded. Excel fixes planned starts in the next two hours.</p></div></li><li><span class="sc-event-dot amber"></span><div><strong>Supply arrives over the week</strong><p>Sensor stock is limited. The next sensor-kit receipt is Wednesday at 10:00.</p></div></li><li><span class="sc-event-dot violet"></span><div><strong>Capacity has interruptions</strong><p>CNC-03 needs a spindle inspection. QA-03 has a calibration appointment.</p></div></li></ol></div><div class="sc-source-stack"><article><div class="sc-source-heading">${icon('machines')}<h3>MES · what is true</h3><span class="sc-source-badge">Source system</span></div><p>Orders, released workplans, progress, stock, shifts and dated availability.</p><div class="sc-source-links">${jump('orders','Production')}${jump('machines','Equipment')}${jump('receipts','Deliveries')}</div></article><div class="sc-source-bridge">${icon('arrow')} The planner reviews the effect of a change</div><article><div class="sc-source-heading">${icon('workbook')}<h3>Excel · what is intended</h3><span class="sc-source-badge">Planning source</span></div><p>Sequence, machine and person assignments, planned times, fixed decisions and qualifications.</p><div class="sc-source-links">${jump('workbook','Open planning workbook')}</div></article><div class="sc-discussion"><strong>Try this in the call</strong><p>“A sensor delivery slips. Which jobs can continue, and which commitments need attention?”</p></div></div></div><p class="sc-caption">The workbook is a separate baseline. Manual edits do not automatically repair conflicts or synchronize with the MES.</p>`;
}

function apex() {
  return `<div class="sc-apex-flow"><article class="sc-apex-input"><span class="sc-kicker">01 / INPUT</span><h2>Business facts<br>& planner decisions</h2><p>MES records + Excel sequence, skills and fixed work.</p><div class="sc-mini-sources">${icon('machines')} MES <span>+</span>${icon('workbook')} Excel</div></article>${icon('arrow')}<article class="sc-apex-engine"><span class="sc-kicker">02 / MODEL & PLAN</span><h2>APEX</h2><p>Model constraints.<br>Explore alternatives.<br>Validate the resulting schedule.</p><span class="sc-engine-note">Demo adapter is the next integration step</span></article>${icon('arrow')}<article class="sc-apex-output"><span class="sc-kicker">03 / REVIEW</span><h2>A plan you<br>can explain</h2><p>Compare delivery performance, resource use and the impact of changed priorities.</p><div class="sc-review-line">Review → Commit decisions → Replan</div></article></div><div class="sc-apex-bottom"><div><h3>Start the conversation</h3><ul><li>Which delivery commitments matter most?</li><li>What may move, and what must stay fixed?</li><li>Which constraints live only in your planner’s head?</li></ul></div><div class="sc-scope"><span class="sc-source-badge">Available in this demo</span><p>Editable MES records, frozen work instructions, production confirmations and a separate Excel planning file.</p><p><strong>Next:</strong> connect these sources to APEX. No optimized result or live Excel synchronization is claimed here.</p></div></div><div class="sc-next-actions">${jump('orders','Explore production')}${jump('routings','Inspect workplans')}${jump('workbook','Open Excel planning')}</div>`;
}

const content = [factory, products, flow, decisions, monday, apex];

export function createShowcase({navigate}) {
  let current = 0, presenting = false;
  let lastPage = 'showcase/factory';
  const go = index => navigate(`showcase/${chapters[index].id}`);
  function togglePresentation() {
    presenting = !presenting;
    document.body.classList.toggle('presenting', presenting);
    const button = document.querySelector('#sc-present');
    button.textContent = presenting ? 'Exit presentation' : 'Present';
    button.setAttribute('aria-pressed', String(presenting));
    window.scrollTo(0,0);
  }
  // Installed once; operational forms and ordinary MES pages keep their keys.
  document.addEventListener('keydown', event => {
    if (!document.body.classList.contains('showcase-open') || event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
    if (event.target.closest('input, textarea, select, [contenteditable="true"]')) return;
    if (event.key === 'Escape' && presenting) { event.preventDefault(); togglePresentation(); }
    if (event.key === 'ArrowRight' && current < chapters.length - 1) { event.preventDefault(); go(current + 1); }
    if (event.key === 'ArrowLeft' && current > 0) { event.preventDefault(); go(current - 1); }
  });
  return {
    get lastPage() { return lastPage; },
    leave() {
      presenting = false;
      document.body.classList.remove('showcase-open','presenting');
    },
    render(page) {
      current = Math.max(0, chapters.findIndex(chapter => page === `showcase/${chapter.id}`));
      lastPage = `showcase/${chapters[current].id}`;
      document.body.classList.add('showcase-open');
      document.body.classList.toggle('presenting', presenting);
      const chapter = chapters[current];
      const remaining = chapters.slice(current).reduce((sum, item) => sum + item.seconds, 0);
      const chapterLinks = chapters.map((item,index)=>`<a href="#showcase/${item.id}" data-chapter="${index}" ${index===current?'aria-current="step"':''}><span>${pad(index+1)}</span>${item.label}</a>`).join('');
      document.querySelector('#main').innerHTML = `<section class="showcase" aria-label="Five-minute factory showcase"><header class="sc-toolbar"><div class="sc-showcase-brand"><img src="/assets/qunevo-logo.svg" width="25" height="25" alt="Qunevo"><strong>Factory showcase</strong><span>6 chapters · 5 minutes</span></div><button class="button" id="sc-present" aria-pressed="${presenting}">${presenting?'Exit presentation':'Present'}</button></header><nav class="sc-chapters" aria-label="Showcase chapters">${chapterLinks}</nav><div class="sc-slide"><header class="sc-slide-heading"><div class="sc-kicker">${pad(current+1)} / ${chapter.label.toUpperCase()} <span>${chapter.seconds} SEC</span></div><h1 id="sc-heading" tabindex="-1">${chapter.title}</h1><p>${chapter.subtitle}</p></header><div class="sc-content">${content[current]()}</div><div class="sc-takeaway"><span>THE KEY POINT</span><p>${chapter.takeaway}</p></div></div><footer class="sc-footer"><div class="sc-footer-note"><strong>${pad(current+1)} / ${pad(chapters.length)}</strong><span>← → to navigate · ${Math.floor(remaining/60)}:${pad(remaining%60)} suggested time left</span></div><div class="sc-controls"><button class="button" id="sc-prev" ${current===0?'disabled':''}>Previous</button>${current===chapters.length-1?'<button class="button" id="sc-restart">Start again</button>':`<button class="button primary" id="sc-next">Next: ${chapters[current+1].label} ${icon('arrow')}</button>`}</div></footer><p class="sc-baseline-note">Fictional factory · figures describe the initial demo baseline · diagrams are schematic</p></section>`;
      for (const link of document.querySelectorAll('[data-chapter]')) link.addEventListener('click', event => {event.preventDefault();go(Number(link.dataset.chapter));});
      for (const button of document.querySelectorAll('[data-mes]')) button.addEventListener('click',()=>navigate(button.dataset.mes));
      document.querySelector('#sc-prev').addEventListener('click',()=>go(Math.max(0,current-1)));
      document.querySelector('#sc-next')?.addEventListener('click',()=>go(current+1));
      document.querySelector('#sc-restart')?.addEventListener('click',()=>go(0));
      document.querySelector('#sc-present').addEventListener('click',togglePresentation);
      document.querySelector('#sc-heading').focus({preventScroll:true});
    },
  };
}
