// canonicalisation: order-independent where it should be, order-dependent where order is the claim
import {canonical,digest} from '../oracle.mjs';
const a={projects:[{id:'p2',title:'b'},{id:'p1',title:'a'}],threads:[{id:'t1',entries:['m1','m2']}],settings:[{key:'z',value:'1'},{key:'a',value:'2'}],seq:3};
const b={seq:3,settings:[{value:'2',key:'a'},{value:'1',key:'z'}],threads:[{entries:['m1','m2'],id:'t1'}],projects:[{title:'a',id:'p1'},{title:'b',id:'p2'}]};
const c={...b,threads:[{entries:['m2','m1'],id:'t1'}]};
let bad=0;const t=(n,ok)=>{console.log(ok?'ok  ':'FAIL',n);if(!ok)bad++};
t('same state, different order, same digest',digest(a)===digest(b));
t('entry order is part of the digest',digest(a)!==digest(c));
t('seq is part of the digest',digest(a)!==digest({...a,seq:4}));
process.exit(bad?1:0);
