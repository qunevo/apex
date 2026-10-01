import assert from 'node:assert/strict';
import en from '../web/locales/en.js';
import de from '../web/locales/de.js';
import {t, translateError, setLanguage, valueLabel, locale} from '../web/i18n.js';

assert.deepEqual(Object.keys(de).sort(),Object.keys(en).sort());
for(const key of Object.keys(en)){
  assert.ok(de[key].trim(),key);
  const parameters=text=>[...text.matchAll(/\{(\w+)\}/g)].map(m=>m[1]).sort();
  assert.deepEqual(parameters(de[key]),parameters(en[key]),key);
}
setLanguage('de');
assert.equal(locale(),'de-DE');
assert.equal(t('Customer orders'),'Kundenaufträge');
assert.equal(translateError('Quantity is required'),'Menge ist erforderlich');
assert.equal(translateError('Unknown reference for Article: ART-01'),'Unbekannte Referenz für Artikel: ART-01');
assert.equal(t('Currently running on {machine}',{machine:'CNC-03'}),'Läuft gerade auf CNC-03');
assert.equal(valueLabel('orders','status','In progress'),'In Bearbeitung');
assert.equal(valueLabel('orders','customer','High'),'High');
assert.equal(valueLabel('orders','note','Waiting'),'Waiting');
assert.equal(t('constructor'),'constructor');
assert.equal(t('__proto__'),'__proto__');
assert.equal(valueLabel('items','name','Unknown custom text'),'Unknown custom text');
setLanguage('en');
assert.equal(t('Present showcase'),'Present');
assert.equal(t('In progress'),'In progress');
assert.equal(t('Customer orders'),'Customer orders');
console.log(`Both locales cover ${Object.keys(en).length} templates; parameters, API errors and source values verified.`);
