#!/usr/bin/env python3
"""Real CSS selector/cascade to Yoga style input; deliberately bounded subset.

Not a browser: no JS/DOM execution, font downloads, grid, pseudo elements,
intrinsic replaced media or container-size discovery. Unsupported active
geometry is reported and must never receive a faithful/pass certification.
Deps: tinycss2==1.5.1 cssselect2==0.9.0 Pillow (system fonts).
"""
import argparse
import json
import math
from pathlib import Path
import re
import xml.etree.ElementTree as ET
import tinycss2
import cssselect2
from PIL import ImageFont

FONT = '/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc'


def element(node):
    if node['type']=='text':
        e=ET.Element('bp-text');e.text=node['text'];return e
    attrs={x['name']:x['value'] for x in node['attrs'] if 'name' in x}
    e=ET.Element(node['tag'],attrs)
    for n in node['kids']:
        e.append(element(n))
    return e


def active(query,width):
    for op,n,unit in re.findall(r'(min|max)-width\s*:\s*([\d.]+)(px|em)',query):
        limit=float(n)*(13 if unit=='em' else 1)
        if op=='max' and width>limit or op=='min' and width<limit:return False
    if 'pointer: coarse' in query or 'prefers-' in query:return False
    return True


def rules(css,width):
    matcher=cssselect2.Matcher();unsupported=[]
    def walk(rs):
        for r in rs:
            if r.type=='qualified-rule':
                selector=tinycss2.serialize(r.prelude)
                decl=[(d.lower_name,tinycss2.serialize(d.value).strip(),d.important) for d in tinycss2.parse_declaration_list(r.content,skip_comments=True,skip_whitespace=True) if d.type=='declaration']
                try:
                    for s in cssselect2.compile_selector_list(selector):
                        if s.pseudo_element is None:matcher.add_selector(s,decl)
                except cssselect2.SelectorError:unsupported.append('selector '+selector.strip())
            elif r.type=='at-rule' and r.content:
                query=tinycss2.serialize(r.prelude)
                if r.lower_at_keyword in ['media','container'] and active(query,width):
                    walk(tinycss2.parse_rule_list(r.content,skip_comments=True,skip_whitespace=True))
                elif r.lower_at_keyword=='supports':
                    unsupported.append('supports query '+query.strip())
    walk(tinycss2.parse_stylesheet(css,skip_comments=True,skip_whitespace=True))
    return matcher,unsupported


def expand(name,value):
    if name in ['padding','margin']:
        parts=value.split();parts=(parts*4 if len(parts)==1 else parts*2 if len(parts)==2 else parts+[parts[1]] if len(parts)==3 else parts)
        return list(zip([name+'-'+s for s in ['top','right','bottom','left']],parts))
    if name=='border':
        w=value.split()[0];return [('border-'+s+'-width',w) for s in ['top','right','bottom','left']]
    if re.fullmatch('border-(top|right|bottom|left)',name):return [(name+'-width',value.split()[0])]
    if name=='gap':
        v=value.split();return [('row-gap',v[0]),('column-gap',v[-1])]
    if name=='flex':
        if value=='none':return [('flex-grow','0'),('flex-shrink','0'),('flex-basis','auto')]
        if value=='auto':return [('flex-grow','1'),('flex-shrink','1'),('flex-basis','auto')]
        v=value.split();return [('flex-grow',v[0]),('flex-shrink',v[1] if len(v)>1 else '1'),('flex-basis',v[2] if len(v)>2 else '0%')]
    if name=='font':
        m=re.search(r'(\d+)px(?:/([\d.]+))?',value)
        if m:return [('font-size',m[1]+'px'),('line-height',m[2] or 'normal')]
    return [(name,value)]


def styled(node,css,width):
    doc=ET.Element('html');body=ET.SubElement(doc,'body');main=ET.SubElement(body,'main',{'class':'thread-view'});main.append(element(node))
    matcher,unsupported=rules(css,width);computed={}
    for w in cssselect2.ElementWrapper.from_html_root(doc).iter_subtree():
        parent=computed.get(w.etree_element if w.parent is None else w.parent.etree_element,{})
        s={k:v for k,v in parent.items() if k.startswith('--') or k in ['font-size','line-height','white-space','visibility']}
        for _,_,_,decl in matcher.match(w):
            for name,value,important in decl:
                for k,v in expand(name,value):s[k]=v
        for d in tinycss2.parse_declaration_list(w.etree_element.get('style',''),skip_comments=True,skip_whitespace=True):
            if d.type=='declaration':
                for k,v in expand(d.lower_name,tinycss2.serialize(d.value).strip()):s[k]=v
        computed[w.etree_element]=s
    fonts={}
    def font(size):
        key=round(size*4)
        if key not in fonts:fonts[key]=ImageFont.truetype(FONT,key)
        return fonts[key]
    def value(v,size=13):
        if v is None:return None
        if v in ['auto','none','normal','inherit']:return v
        # Resolve vars with fallback. All custom properties used by geometry
        # in this fixture have scalar fallbacks; nested font vars are not sizes.
        v=re.sub(r'var\(--[^,)]+,\s*([^()]+)\)',r'\1',v)
        if v.endswith('%') and not v.startswith('calc'):return v
        ch=font(size).getlength('0')/4
        v=re.sub(r'([\d.]+)(px|ch|em|rem|vh|dvh)',lambda m:str(float(m[1])*{'px':1,'ch':ch,'em':size,'rem':13,'vh':8.44,'dvh':8.44}[m[2]]),v)
        if re.fullmatch(r'[\d.]+',v):return float(v)
        if v.startswith('calc('):
            expr=v[5:-1];expr=re.sub(r'([\d.]+)%',lambda m:str(float(m[1])*width/100),expr)
            if re.fullmatch(r'[\d. +*/()-]+',expr):return float(eval(expr,{'__builtins__':{}},{}))
        return v
    def make(e):
        s=computed[e];size=value(s.get('font-size','13px'))
        if not isinstance(size,(float,int)):size=13
        lh=value(s.get('line-height','1.5'),size);lh=1.5*size if lh=='normal' else lh*size if isinstance(lh,(float,int)) and lh<4 else lh
        if not isinstance(lh,(float,int)):lh=size*1.5
        def resolved(v):
            for _ in range(8):
                old=v
                v=re.sub(r'var\((--[^,)]+)\)',lambda m:s.get(m[1],m[0]),v)
                if old==v:break
            return value(v,size)
        out={'tag':e.tag,'class':e.get('class',''),'id':e.get('id',''),'style':{k:resolved(v) for k,v in s.items() if not k.startswith('--')},'children':[],
             'fontSize':size,'lineHeight':lh,'text':e.text or ''}
        if s.get('display')=='none':return out
        if s.get('display')=='grid':unsupported.append('active grid: '+out['class'])
        for child in e:out['children'].append(make(child))
        def text_of(n):return '' if n['style'].get('display')=='none' else n['text']+''.join(text_of(c) for c in n['children'])
        text=text_of(out);out['textWidth']=font(size).getlength(text)/4
        out['minTextWidth']=max([font(size).getlength(t)/4 for t in re.split(r'\s+',text)]+[0])
        if e.tag=='svg':out['style'].setdefault('width',float(e.get('width','18')));out['style'].setdefault('height',float(e.get('height','18')));out['children']=[]
        if e.tag in ['input','path']:out['children']=[]
        return out
    return {'width':width,'tree':make(main[0]),'unsupported':sorted(set(unsupported)),
            'assumptions':['container queries use supplied composer width','font family fixed to local Noto Sans CJK','no pseudo elements, grid, line breaking/hyphenation or intrinsic media sizing']}


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('tree');p.add_argument('css');p.add_argument('--width',type=int,required=True);p.add_argument('--out',required=True);a=p.parse_args()
    Path(a.out).write_text(json.dumps(styled(json.loads(Path(a.tree).read_text()),Path(a.css).read_text(),a.width)))
