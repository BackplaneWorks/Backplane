const {test}=require('node:test'),assert=require('node:assert/strict');
test('a send confirmation clears native text before the next input, without waiting for a render',async()=>{
 const {applyComposerDraft}=await import('../../src/web/composer-draft.js');
 const field={value:'first part'},doc={getElementById:()=>field,querySelector:()=>({getAttribute:()=> 'A'})};
 applyComposerDraft(doc,'A','');field.value+='next';assert.equal(field.value,'next');
 applyComposerDraft(doc,'A','next');assert.equal(field.value,'next');
});
test('a consumed prefix reaches the native field while another thread and unchanged caret stay untouched',async()=>{
 const {applyComposerDraft}=await import('../../src/web/composer-draft.js');let value='firstnext',writes=0;
 const field={get value(){return value},set value(text){writes++;value=text}},doc={getElementById:()=>field,querySelector:()=>({getAttribute:()=> 'A'})};
 applyComposerDraft(doc,'B','');assert.equal(value,'firstnext');assert.equal(writes,0);
 applyComposerDraft(doc,'A','next');assert.equal(value,'next');assert.equal(writes,1);
 applyComposerDraft(doc,'A','next');assert.equal(writes,1);
 applyComposerDraft({...doc,getElementById:()=>null},'A','');assert.equal(writes,1);
});
// a textarea's setRangeText with 'preserve', as the HTML standard has it
const textarea=(value,start=value.length,end=start)=>({value,selectionStart:start,selectionEnd:end,writes:0,
 setRangeText(text,from,to,mode){this.writes++;const delta=text.length-(to-from),last=from+text.length;
  this.value=this.value.slice(0,from)+text+this.value.slice(to);
  if(mode==='end'){this.selectionStart=this.selectionEnd=last;return;}
  assert.equal(mode,'preserve');
  this.selectionStart=this.selectionStart>to?this.selectionStart+delta:this.selectionStart>from?from:this.selectionStart;
  this.selectionEnd=this.selectionEnd>to?this.selectionEnd+delta:this.selectionEnd>from?last:this.selectionEnd;}});
const page=field=>({getElementById:()=>field,querySelector:()=>({getAttribute:()=> 'A'})});
test('a sent text taken off the front keeps the caret and a selection in what was typed after it',async()=>{
 const {applyComposerDraft}=await import('../../src/web/composer-draft.js');
 const caret=textarea('Hi next');applyComposerDraft(page(caret),'A',' next');
 assert.equal(caret.value,' next');assert.deepEqual([caret.selectionStart,caret.selectionEnd],[5,5]);
 const picked=textarea('Hi abc def',3,6);applyComposerDraft(page(picked),'A',' abc def');
 assert.equal(picked.value,' abc def');assert.equal(picked.value.slice(picked.selectionStart,picked.selectionEnd),'abc');
 const middle=textarea('aaab',2);applyComposerDraft(page(middle),'A','aab');
 assert.equal(middle.value,'aab');assert.equal(middle.selectionStart,1,'the cut is at the front, the caret moves with the text');
});
test('a whole new draft (Save giving the chat draft back) leaves the caret at its end, nothing selected',async()=>{
 const {applyComposerDraft}=await import('../../src/web/composer-draft.js');
 const field=textarea('/intent\n# Goal\nA lamp');applyComposerDraft(page(field),'A','my chat draft 日本語');
 assert.equal(field.value,'my chat draft 日本語');assert.deepEqual([field.selectionStart,field.selectionEnd],[17,17]);
 const typed=textarea('Hi there',0,8);applyComposerDraft(page(typed),'A','');assert.deepEqual([typed.value,typed.selectionStart,typed.selectionEnd],['',0,0]);
});
test('nothing is written into the composer during an IME composition',async()=>{
 const {applyComposerDraft}=await import('../../src/web/composer-draft.js');
 const field=textarea('Hi にほん',5);applyComposerDraft(page(field),'A',' にほん',true);
 assert.equal(field.value,'Hi にほん');assert.equal(field.writes,0);
 applyComposerDraft(page(field),'A',' にほん');assert.equal(field.value,' にほん');assert.equal(field.selectionStart,3);
});
test('where an edit starts: the selection, the character a Backspace takes, else the start',async()=>{
 const {editStart}=await import('../../src/web/sent-claims.js');
 assert.equal(editStart('insertText','Hi there',8,8),8);
 assert.equal(editStart('insertText','Hi there',0,8),0,'typing over a selection');
 assert.equal(editStart('insertFromPaste','Hi there',2,5),2);
 assert.equal(editStart('deleteContentBackward','Hi there',3,3),2);
 assert.equal(editStart('deleteContentBackward','Hi 🙂',5,5),3,'a surrogate pair is one character');
 assert.equal(editStart('deleteContentForward','Hi there',3,3),3);
 assert.equal(editStart('deleteWordBackward','Hi there',8,8),0);
 assert.equal(editStart('insertReplacementText','Hi there',8,8),0);
 assert.equal(editStart('historyUndo','Hi there',8,8),0);
 assert.equal(editStart('insertFromDrop','Hi there',8,8),0);
});
