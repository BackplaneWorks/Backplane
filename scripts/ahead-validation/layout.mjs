// Actual Yoga boxes, fed from the real D.Node/CSS cascade adapter. Advisory.
import Yoga from 'yoga-layout';
import {readFileSync,writeFileSync} from 'node:fs';
const start=performance.now(),input=JSON.parse(readFileSync(process.argv[2],'utf8'));
const config=Yoga.Config.create();config.setUseWebDefaults(true);
const warnings=[...input.unsupported],all=[],relations=[],measured=new Set();
const edge={top:Yoga.EDGE_TOP,right:Yoga.EDGE_RIGHT,bottom:Yoga.EDGE_BOTTOM,left:Yoga.EDGE_LEFT};
const number=x=>typeof x==='number'||typeof x==='string'&&/^\d+(\.\d+)?%$/.test(x);
function make(t,parent=null){
  const n=Yoga.Node.createWithConfig(config),s=t.style;
  n.setFlexDirection(['flex','inline-flex'].includes(s.display)?s['flex-direction']==='column'?Yoga.FLEX_DIRECTION_COLUMN:Yoga.FLEX_DIRECTION_ROW:
    ['button','span','label'].includes(t.tag)?Yoga.FLEX_DIRECTION_ROW:Yoga.FLEX_DIRECTION_COLUMN);
  if(s.display==='none')n.setDisplay(Yoga.DISPLAY_NONE);
  if(s['flex-wrap']==='wrap')n.setFlexWrap(Yoga.WRAP_WRAP);
  for(const [k,method] of [['width','setWidth'],['height','setHeight'],['min-width','setMinWidth'],['max-width','setMaxWidth'],['min-height','setMinHeight'],['max-height','setMaxHeight'],['flex-basis','setFlexBasis']])
    if(number(s[k]))n[method](s[k]);
  for(const [k,method] of [['flex-grow','setFlexGrow'],['flex-shrink','setFlexShrink']])if(typeof s[k]==='number')n[method](s[k]);
  const align={'center':Yoga.ALIGN_CENTER,'stretch':Yoga.ALIGN_STRETCH,'flex-start':Yoga.ALIGN_FLEX_START,'flex-end':Yoga.ALIGN_FLEX_END};
  if(align[s['align-items']]!==undefined)n.setAlignItems(align[s['align-items']]);
  if(align[s['align-self']]!==undefined)n.setAlignSelf(align[s['align-self']]);
  const justify={'space-between':Yoga.JUSTIFY_SPACE_BETWEEN,'center':Yoga.JUSTIFY_CENTER,'flex-end':Yoga.JUSTIFY_FLEX_END};
  if(justify[s['justify-content']]!==undefined)n.setJustifyContent(justify[s['justify-content']]);
  if(typeof s['row-gap']==='number')n.setGap(Yoga.GUTTER_ROW,s['row-gap']);
  if(typeof s['column-gap']==='number')n.setGap(Yoga.GUTTER_COLUMN,s['column-gap']);
  for(const [side,e] of Object.entries(edge)){
    if(number(s['padding-'+side]))n.setPadding(e,s['padding-'+side]);
    if(number(s['margin-'+side])||s['margin-'+side]==='auto')n.setMargin(e,s['margin-'+side]);
    if(typeof s['border-'+side+'-width']==='number')n.setBorder(e,s['border-'+side+'-width']);
  }
  if(s.position==='absolute'||s.position==='fixed')n.setPositionType(Yoga.POSITION_TYPE_ABSOLUTE);
  const flatten=kids=>kids.flatMap(k=>k.style.display==='contents'?flatten(k.children):k.style.display==='none'?[]:[k]);
  const kids=flatten(t.children);
  const inline=kids.every(k=>k.tag==='bp-text'||['span','kbd','small','b','i'].includes(k.tag)&&!['flex','grid','inline-flex'].includes(k.style.display));
  if(t.tag==='svg'||!kids.length||inline){
    measured.add(n);
    n.setMeasureFunc((width,mode)=>{const nowrap=s['white-space']==='nowrap'||t.tag==='svg';
      const available=mode===Yoga.MEASURE_MODE_UNDEFINED?t.textWidth:width;
      return {width:nowrap?t.textWidth:Math.min(t.textWidth,available),height:t.tag==='svg'?Number(s.height)||18:
        Math.max(1,nowrap?1:Math.ceil(t.textWidth/Math.max(1,available)))*t.lineHeight};});
    // CSS automatic minimum for flex items is min-content unless overflow
    // is hidden or an explicit minimum opts out. Yoga does not model this.
    if(parent&&['flex','inline-flex'].includes(parent.style.display)&&s['min-width']===undefined&&s.overflow!=='hidden')n.setMinWidth(t.minTextWidth);
  }else for(const child of kids){const c=make(child,t);relations.push({parent:n,child:c});n.insertChild(c,n.getChildCount());}
  all.push({t,n});return n;
}
const root=make(input.tree);root.calculateLayout(input.width,undefined,Yoga.DIRECTION_LTR);
const boxes=all.filter(x=>x.t.style.display!=='none').map(({t,n})=>({tag:t.tag,class:t.class,id:t.id,...n.getComputedLayout()}));
const foot=boxes.find(x=>x.class.split(' ').includes('composer-foot'));
const buttons=boxes.filter(x=>x.tag==='button'&&x.height>0&&x.class!=='row');
const maxButton=Math.max(...buttons.map(x=>x.height));
const overflow=relations.flatMap(({parent,child})=>{
  const p=parent.getComputedLayout(),c=child.getComputedLayout(),meta=all.find(r=>r.n===child);
  return c.height&&c.width&&meta.t.style.position!=='absolute'&&(c.left < -1||c.left+c.width > p.width+1)?
    [{tag:meta.t.tag,class:meta.t.class,right:c.left+c.width,parentWidth:p.width}]:[];
});
const textClipping=all.flatMap(({t,n})=>{
  if(!measured.has(n)||t.tag==='svg'||!t.textWidth)return [];
  const b=n.getComputedLayout(),inner=b.width-n.getComputedPadding(Yoga.EDGE_LEFT)-n.getComputedPadding(Yoga.EDGE_RIGHT)-n.getComputedBorder(Yoga.EDGE_LEFT)-n.getComputedBorder(Yoga.EDGE_RIGHT);
  return b.height&&t.style['white-space']==='nowrap'&&t.textWidth>inner+1?
    [{class:t.class,intentionalEllipsis:t.style['text-overflow']==='ellipsis',textWidth:t.textWidth,available:inner}]:[];
});
const result={approach:'layout',width:input.width,footHeight:foot?.height,buttonHeight:maxButton,
  oneButtonTall:foot?foot.height<=maxButton+1:null,overflow,textClipping,boxes,unsupported:warnings,assumptions:input.assumptions,
  seconds:(performance.now()-start)/1000,scope:'Yoga + real CSS cascade subset; advisory, never browser-equivalent'};
if(process.argv[3])writeFileSync(process.argv[3],JSON.stringify(result,null,2));
console.log(JSON.stringify({...result,boxes:undefined}));root.freeRecursive();config.free();
