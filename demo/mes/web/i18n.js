import en from './locales/en.js';
import de from './locales/de.js';

let current = 'en';
try { current = localStorage.getItem('qunevo-mes-language') || (navigator.language.startsWith('de') ? 'de' : 'en'); } catch { /* Browser storage is optional. */ }
if (!['de','en'].includes(current)) current='en';
export const language = () => current;
export const locale = () => current==='de'?'de-DE':'en-GB';
export function setLanguage(value) {
  if (!['de','en'].includes(value)) return;
  current=value;
  try { localStorage.setItem('qunevo-mes-language',value); } catch { /* Keep this session usable without storage. */ }
  if(typeof document!=='undefined')document.documentElement.lang=current;
}
setLanguage(current);

const templates = Object.keys(en).filter(key=>/\{\w+\}/.test(key)).sort((a,b)=>b.replace(/\{\w+\}/g,'').length-a.replace(/\{\w+\}/g,'').length).map(key=>{
  const names=[];
  const pattern=key.split(/(\{\w+\})/).map(part=>{
    if(/^\{\w+\}$/.test(part)){names.push(part.slice(1,-1));return '(.+?)';}
    return part.replace(/[.*+?^${}()|[\]\\]/g,'\\$&');
  }).join('');
  return {key,names,regex:new RegExp('^'+pattern+'$')};
});

export function t(key, params) {
  if(key===null||key===undefined)return '';
  const text=String(key), messages=current==='de'?de:en;
  const template=Object.hasOwn(messages,text)?messages[text]:text;
  return template.replace(/\{(\w+)\}/g,(match,name)=>params?.[name]??match);
}

// Only API errors use pattern matching; arbitrary source prose is never interpreted as a template.
export function translateError(message) {
  if(current==='en'||Object.hasOwn(de,message))return t(message);
  for(const entry of templates){
    const match=message.match(entry.regex);
    if(match)return t(entry.key,Object.fromEntries(entry.names.map((name,i)=>[
      name,Object.hasOwn(de,match[i+1])?de[match[i+1]]:match[i+1],
    ])));
  }
  return message;
}

// Translate controlled vocabulary and known synthetic labels. Stored source values never change.
export function valueLabel(entity, key, value) {
  if(['active','cancelled','permanently_unavailable'].includes(key))return t(value===true||value==='True'?'Yes':'No');
  if((key==='name'&&['items','machines','routings','routing_steps','materials','shifts'].includes(entity))||key==='capability'||entity==='materials'&&key==='location'||entity==='operations'&&key==='instruction')return t(value);
  if(['unit','status','attendance','priority','quality_status','variant','family','size','material','body_material','group','skill','team','current_operation'].includes(key)
    || entity==='operations'&&['name','output'].includes(key) || entity==='lots'&&key==='location')return t(value);
  return value;
}
