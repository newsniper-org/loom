use bevy_ecs::prelude::Entity;
use loom_runtime::web::BrowserDom;
use loom_runtime::*;
use wasm_bindgen::prelude::*;

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

#[wasm_bindgen]
pub struct Probe {
    app: LoomApp,
    dom: BrowserDom,
    root: NodeHandle,
    ents: Vec<Entity>,
}

#[wasm_bindgen]
impl Probe {
    #[wasm_bindgen(constructor)]
    pub fn new(mount: web_sys::Element) -> Probe {
        let document = web_sys::window().unwrap().document().unwrap();
        let (dom, root) = BrowserDom::new(document, mount.into());
        Probe { app: LoomApp::new(), dom, root, ents: Vec::new() }
    }

    fn push(&mut self, w: SurfaceWrite) {
        self.app.world.resource_mut::<PendingDomWrites>().0.push(DomWrite::Surface(w));
    }

    fn parent(&self, parent: i32) -> WriteParent {
        if parent < 0 { WriteParent::Node(self.root) } else { WriteParent::Entity(self.ents[parent as usize]) }
    }

    /// ns: 0 Html 1 Svg. attrs는 "k=v;k=v". 돌려주는 값은 엔티티 인덱스.
    pub fn spawn(&mut self, parent: i32, ns: u8, tag: &str, class: Option<String>, attrs: Option<String>, text: Option<String>) -> u32 {
        let attrs = attrs.unwrap_or_default();
        let attrs: Vec<(&'static str, &'static str)> = attrs
            .split(';')
            .filter(|s| !s.is_empty())
            .map(|kv| {
                let (k, v) = kv.split_once('=').unwrap();
                (leak(k.into()), leak(v.into()))
            })
            .collect();
        let spec: &'static ElementSpec = Box::leak(Box::new(ElementSpec {
            ns: if ns == 1 { Namespace::Svg } else { Namespace::Html },
            tag: leak(tag.into()),
            class: class.map(leak),
            loom_id: None,
            attrs: Box::leak(attrs.into_boxed_slice()),
        }));
        let e = self.app.world.spawn_empty().id();
        let p = self.parent(parent);
        self.push(SurfaceWrite::SpawnSpec { entity: e, parent: p, spec, text });
        self.ents.push(e);
        (self.ents.len() - 1) as u32
    }

    pub fn spawn_text(&mut self, parent: u32, text: &str) -> u32 {
        let e = self.app.world.spawn_empty().id();
        let p = self.parent(parent as i32);
        self.push(SurfaceWrite::SpawnText { entity: e, parent: p, text: text.into() });
        self.ents.push(e);
        (self.ents.len() - 1) as u32
    }

    pub fn set_text_data(&mut self, text_ent: u32, text: &str) {
        let h = self.app.world.get::<TextNodeTarget>(self.ents[text_ent as usize]).unwrap().0;
        self.push(SurfaceWrite::SetTextData { node: h, text: text.into() });
    }
    pub fn set_attr(&mut self, ent: u32, name: &str, value: &str) {
        let node = self.parent(ent as i32);
        self.push(SurfaceWrite::SetAttr { node, name: leak(name.into()), value: value.into() });
    }
    pub fn remove_attr(&mut self, ent: u32, name: &str) {
        let node = self.parent(ent as i32);
        self.push(SurfaceWrite::RemoveAttr { node, name: leak(name.into()) });
    }
    pub fn toggle_class(&mut self, ent: u32, class: &str, on: bool) {
        let node = self.parent(ent as i32);
        self.push(SurfaceWrite::ToggleClass { node, class: leak(class.into()), on });
    }
    /// prop: 0 checked 1 selected 2 indeterminate
    pub fn set_bool_prop(&mut self, ent: u32, prop: u8, value: bool) {
        let node = self.parent(ent as i32);
        let prop = [BoolProp::Checked, BoolProp::Selected, BoolProp::Indeterminate][prop as usize];
        self.push(SurfaceWrite::SetBoolProp { node, prop, value });
    }
    /// 기존 `SetInputValue`(textarea/select 덕타이핑 확인용).
    pub fn set_input_value(&mut self, ent: u32, value: &str) {
        let h = self.app.world.get::<DomTarget>(self.ents[ent as usize]).unwrap().0;
        self.app.world.resource_mut::<PendingDomWrites>().0.push(DomWrite::SetInputValue { node: h, value: value.into() });
    }
    pub fn tick(&mut self) {
        self.app.tick(&mut self.dom);
    }
    /// 마운트 노드의 자식을 `adopt_element_children`으로 입양해 "텍스트|텍스트|…"를 돌려준다.
    pub fn adopt_root(&mut self) -> String {
        self.dom.adopt_element_children(self.root).into_iter().map(|(_, t)| t).collect::<Vec<_>>().join("|")
    }
    pub fn dropped(&self) -> usize {
        self.app.dropped_writes
    }
}
