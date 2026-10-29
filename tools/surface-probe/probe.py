"""7c 브라우저 프로브 (Chromium, Playwright). 사용: python3 probe.py  (같은 디렉터리에서 http.server 로 서빙)"""
import json, os, subprocess, sys, time
from playwright.sync_api import sync_playwright
srv = subprocess.Popen([sys.executable, "-m", "http.server", "8765"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
time.sleep(1)
out = {}
try:
    with sync_playwright() as p:
        b = getattr(p, os.environ.get("BROWSER", "chromium")).launch()
        pg = b.new_page()
        errs = []
        pg.on("pageerror", lambda e: errs.append(str(e)))
        pg.on("console", lambda m: errs.append(m.text) if m.type == "error" else None)
        pg.goto("http://localhost:8765/index.html")
        pg.wait_for_function("window.ready===true")
        out["browser"] = b.version

        # 1. SVG 네임스페이스와 class
        out["svg"] = pg.evaluate("""()=>{const m=document.getElementById('m');const p=new Probe(m);
          const s=p.spawn(-1,1,'svg','icon','width=10;height=10',null); const g=p.spawn(s,1,'g','grp','fill=red',null); p.tick();
          const e=m.querySelector('svg'), ge=m.querySelector('g');
          return {svgNs:e.namespaceURI,gNs:ge.namespaceURI,svgClass:e.getAttribute('class'),gClass:ge.getAttribute('class'),
                  isSVGElement:e instanceof SVGElement,fill:ge.getAttribute('fill'),dropped:p.dropped()}}""")

        # 2. checked: 사용자 클릭 뒤 SetBoolProp / SetAttr 의 효과
        pg.evaluate("""()=>{const m=document.getElementById('m');m.innerHTML='';window.p2=new Probe(m);
          window.cb=window.p2.spawn(-1,0,'input',null,'type=checkbox',null);window.p2.tick();}""")
        pg.click("#m input")
        after_click = pg.evaluate("document.querySelector('#m input').checked")
        pg.click("#m input")  # 두 번째 클릭: 사용자가 끈 상태(dirty) — setAttribute가 프로퍼티를 못 바꾸는지 보려면 false여야 한다
        after_second_click = pg.evaluate("document.querySelector('#m input').checked")
        pg.evaluate("window.p2.set_attr(window.cb,'checked','');window.p2.tick()")
        attr_after_click = pg.evaluate("document.querySelector('#m input').checked")
        pg.evaluate("window.p2.set_bool_prop(window.cb,0,false);window.p2.tick()")
        prop_false = pg.evaluate("document.querySelector('#m input').checked")
        pg.evaluate("window.p2.set_bool_prop(window.cb,0,true);window.p2.tick()")
        prop_true = pg.evaluate("document.querySelector('#m input').checked")
        out["checked"] = dict(after_user_click=after_click, after_second_user_click=after_second_click, after_setAttribute_checked_when_user_unchecked=attr_after_click,
                              after_SetBoolProp_false=prop_false, after_SetBoolProp_true=prop_true)

        # 3. MutationRecord: no-op 때 기록이 생기는가
        out["mutation"] = pg.evaluate("""async()=>{const m=document.getElementById('m');m.innerHTML='';const p=new Probe(m);
          const e=p.spawn(-1,0,'div','a',null,null);p.tick();const el=m.firstChild;
          const rec=[];const mo=new MutationObserver(r=>rec.push(...r));mo.observe(m,{attributes:true,childList:true,characterData:true,subtree:true});
          const n=async(f)=>{rec.length=0;f();p.tick();await Promise.resolve();rec.push(...mo.takeRecords());return rec.length};
          const r={};
          r.toggle_on_already_on=await n(()=>p.toggle_class(e,'a',true));
          r.toggle_on_new=await n(()=>p.toggle_class(e,'b',true));
          r.toggle_off_absent=await n(()=>p.toggle_class(e,'zzz',false));
          r.remove_missing_attr=await n(()=>p.remove_attr(e,'nope'));
          r.set_attr_new=await n(()=>p.set_attr(e,'title','x'));
          r.set_attr_same_value=await n(()=>p.set_attr(e,'title','x'));
          r.remove_present_attr=await n(()=>p.remove_attr(e,'title'));
          return r}""")

        # 4. adopt_element_children: 재포맷 HTML, 복합 row
        out["adopt"] = pg.evaluate("""()=>{const m=document.getElementById('m');
          m.innerHTML='\\n  <ul>\\n    <li>a</li>\\n    <!-- c -->\\n    <li>b</li>\\n  </ul>\\n';
          const ul=m.querySelector('ul'); const p=new Probe(ul);
          const plain=p.adopt_root();
          const ul2=document.createElement('ul');ul2.innerHTML='<li>v<span>x</span></li><li> w </li>';m.appendChild(ul2);
          const q=new Probe(ul2);const composite=q.adopt_root();
          return {plain,composite}}""")

        # 5. textarea / select 의 set_input_value
        out["input_value"] = pg.evaluate("""()=>{const m=document.getElementById('m');m.innerHTML='';const p=new Probe(m);
          const t=p.spawn(-1,0,'textarea',null,null,null);const s=p.spawn(-1,0,'select',null,null,null);
          const o1=p.spawn(s,0,'option',null,'value=a','A');const o2=p.spawn(s,0,'option',null,'value=b','B');p.tick();
          p.set_input_value(t,'hello');p.set_input_value(s,'b');p.tick();
          return {textarea:m.querySelector('textarea').value,select:m.querySelector('select').value,
                  optionSelectedProp:m.querySelectorAll('option')[1].selected}}""")

        # 6. 혼합 텍스트: 정적 형제 보존
        out["mixed_text"] = pg.evaluate("""()=>{const m=document.getElementById('m');m.innerHTML='';const p=new Probe(m);
          const d=p.spawn(-1,0,'div',null,null,'Count: ');const t=p.spawn_text(d,'5');p.tick();const before=m.firstChild.innerHTML;
          p.set_text_data(t,'6');p.tick();return {before,after:m.firstChild.innerHTML,childNodes:m.firstChild.childNodes.length}}""")
        out["page_errors"] = errs
        b.close()
finally:
    srv.terminate()
print(json.dumps(out, ensure_ascii=False, indent=1))
