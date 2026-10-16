//! **The control tree, and the patches that change it.**
//!
//! A control is `{"_c": type, "_i": id, properties…}`. A property whose value
//! is a map with `_c` is a child control, in a list too. Here every control
//! lives in one table by its id, and a property refers to a child by id
//! ([`Prop::Control`]); the patches and the events name controls by id.
//!
//! The patch format and its application follow the Dart client's
//! `models/control.dart` (`applyPatch`): `[tree_index, op, op, …]`, where the
//! tree index numbers the places the operations touch, and an operation is
//! replace `[0, index, key, value]`, add `[1, index, key, value]`, remove
//! `[2, index, key]` or move `[3, from_index, from_key, to_index, to_key]`.

use std::collections::HashMap;

use crate::msgpack::Value;

pub type ControlId = i64;

/// The type of a component (`@component` on the Python side); its body is
/// in `_b`, and it is drawn by drawing the body
pub const COMPONENT_TYPE: &str = "C";
pub const COMPONENT_BODY: &str = "_b";

/// A property's value
#[derive(Clone, Debug, PartialEq)]
pub enum Prop {
    Value(Value),
    Control(ControlId),
    List(Vec<Prop>),
    Map(Vec<(String, Prop)>),
}

impl Prop {
    fn get(&self, name: &str) -> Option<&Prop> {
        match self {
            Prop::Map(m) => m.iter().find(|(k, _)| k == name).map(|(_, p)| p),
            _ => None,
        }
    }

    fn get_mut(&mut self, name: &str) -> Option<&mut Prop> {
        match self {
            Prop::Map(m) => m.iter_mut().find(|(k, _)| k == name).map(|(_, p)| p),
            _ => None,
        }
    }

    fn set(&mut self, name: &str, value: Prop) -> Result<(), String> {
        match self {
            Prop::Map(m) => {
                match m.iter_mut().find(|(k, _)| k == name) {
                    Some((_, p)) => *p = value,
                    None => m.push((name.to_string(), value)),
                }
                Ok(())
            }
            _ => Err(format!("{name} を置く先が辞書ではありません")),
        }
    }

    fn remove(&mut self, name: &str) -> Option<Prop> {
        match self {
            Prop::Map(m) => m.iter().position(|(k, _)| k == name).map(|i| m.remove(i).1),
            _ => None,
        }
    }

    pub fn as_value(&self) -> Option<&Value> {
        match self {
            Prop::Value(v) => Some(v),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Control {
    pub id: ControlId,
    /// The `_c` of the control: `Page`, `Column`, `Text`, …
    pub ctype: String,
    /// Always a [`Prop::Map`]; `_i` and `_c` are not in it
    pub props: Prop,
    pub parent: Option<ControlId>,
}

impl Control {
    pub fn get(&self, name: &str) -> Option<&Prop> {
        self.props.get(name)
    }

    pub fn get_value(&self, name: &str) -> Option<&Value> {
        self.get(name).and_then(Prop::as_value).filter(|v| !v.is_nil())
    }

    pub fn get_str(&self, name: &str) -> Option<&str> {
        self.get_value(name).and_then(Value::as_str)
    }

    pub fn get_f64(&self, name: &str) -> Option<f64> {
        self.get_value(name).and_then(Value::as_f64)
    }

    pub fn get_i64(&self, name: &str) -> Option<i64> {
        self.get_value(name).and_then(Value::as_i64)
    }

    pub fn get_bool(&self, name: &str) -> Option<bool> {
        self.get_value(name).and_then(Value::as_bool)
    }

    /// `visible` defaults to true
    pub fn visible(&self) -> bool {
        self.get_bool("visible").unwrap_or(true)
    }

    /// Whether Python has a handler for the event (`on_<name>` is true)
    pub fn has_event(&self, name: &str) -> bool {
        let key = if name.starts_with("on_") { name.to_string() } else { format!("on_{name}") };
        self.get_bool(&key).unwrap_or(false)
    }

    /// The ids of the controls in the property (one control or a list)
    fn child_ids(&self, name: &str) -> Vec<ControlId> {
        match self.get(name) {
            Some(Prop::Control(id)) => vec![*id],
            Some(Prop::List(l)) => l.iter().filter_map(|p| if let Prop::Control(id) = p { Some(*id) } else { None }).collect(),
            _ => Vec::new(),
        }
    }
}

/// Where an operation lands: a control, and a path inside its properties
/// (empty for the properties map itself)
#[derive(Clone, Debug, PartialEq)]
enum Key {
    Name(String),
    Index(usize),
}

impl Key {
    fn from_value(v: &Value) -> Result<Key, String> {
        if let Some(s) = v.as_str() {
            return Ok(Key::Name(s.to_string()));
        }
        if let Some(n) = v.as_i64() {
            return usize::try_from(n).map(Key::Index).map_err(|_| format!("負の位置 {n}"));
        }
        Err(format!("鍵として読めません: {v:?}"))
    }
}

#[derive(Debug, Default)]
pub struct Tree {
    controls: HashMap<ControlId, Control>,
    pub root: Option<ControlId>,
}

impl Tree {
    pub fn new() -> Tree {
        Tree::default()
    }

    pub fn get(&self, id: ControlId) -> Option<&Control> {
        self.controls.get(&id)
    }

    pub fn controls(&self) -> impl Iterator<Item = &Control> {
        self.controls.values()
    }

    /// The first control of a type, in no particular order
    pub fn find_type(&self, ctype: &str) -> Option<&Control> {
        self.controls.values().find(|c| c.ctype == ctype)
    }

    /// The control a property names, if it is one and visible
    pub fn child(&self, id: ControlId, name: &str) -> Option<ControlId> {
        self.children(id, name).into_iter().next()
    }

    /// The visible controls in a property, components unwrapped to their
    /// bodies (a component without a body yet is skipped)
    pub fn children(&self, id: ControlId, name: &str) -> Vec<ControlId> {
        let Some(c) = self.get(id) else { return Vec::new() };
        c.child_ids(name)
            .into_iter()
            .filter_map(|cid| self.unwrap_component(cid))
            .filter(|cid| self.get(*cid).is_some_and(Control::visible))
            .collect()
    }

    /// Follows a component's body down to the control that is drawn
    pub fn unwrap_component(&self, mut id: ControlId) -> Option<ControlId> {
        loop {
            let c = self.get(id)?;
            if c.ctype != COMPONENT_TYPE {
                return Some(id);
            }
            match c.get(COMPONENT_BODY) {
                Some(Prop::Control(b)) => id = *b,
                _ => return None,
            }
        }
    }

    /// Makes the tree from a whole control (the Page of a registration
    /// response) and returns its id
    pub fn set_root(&mut self, v: &Value) -> Result<ControlId, String> {
        match self.transform(v, None)? {
            Prop::Control(id) => {
                self.root = Some(id);
                Ok(id)
            }
            _ => Err("根が部品ではありません(_c がありません)".into()),
        }
    }

    /// A value as a property: a map with `_c` becomes a control (and its
    /// children too), other maps and lists are walked, the rest is kept
    fn transform(&mut self, v: &Value, parent: Option<ControlId>) -> Result<Prop, String> {
        match v {
            Value::Map(pairs) if v.get("_c").is_some() => {
                let ctype = v.get("_c").and_then(Value::as_str).ok_or("_c が字ではありません")?.to_string();
                let id = v.get("_i").and_then(Value::as_i64).ok_or_else(|| format!("{ctype} に _i がありません"))?;
                let mut props = Vec::with_capacity(pairs.len());
                for (k, x) in pairs {
                    let k = k.as_str().ok_or("属性の名前が字ではありません")?;
                    if k == "_c" || k == "_i" {
                        continue;
                    }
                    props.push((k.to_string(), self.transform(x, Some(id))?));
                }
                self.controls.insert(id, Control { id, ctype, props: Prop::Map(props), parent });
                Ok(Prop::Control(id))
            }
            Value::Map(pairs) => {
                let mut out = Vec::with_capacity(pairs.len());
                for (k, x) in pairs {
                    let k = match k {
                        Value::Str(s) => s.clone(),
                        other => other.as_i64().map(|n| n.to_string()).ok_or("辞書の鍵が読めません")?,
                    };
                    out.push((k, self.transform(x, parent)?));
                }
                Ok(Prop::Map(out))
            }
            Value::Array(items) => {
                let mut out = Vec::with_capacity(items.len());
                for x in items {
                    out.push(self.transform(x, parent)?);
                }
                Ok(Prop::List(out))
            }
            other => Ok(Prop::Value(other.clone())),
        }
    }

    /// Merges a map of properties into a control, the way the Dart client's
    /// `update` does (a registration response's page, a window's state):
    /// maps merge, a child with the same `_i` merges, anything else replaces.
    /// Returns the controls that changed.
    pub fn update(&mut self, id: ControlId, props: &Value) -> Result<Vec<ControlId>, String> {
        let pairs = props.as_map().ok_or("属性の更新は辞書です")?;
        let mut changed = Vec::new();
        for (k, v) in pairs {
            let Some(k) = k.as_str() else { continue };
            if k == "_i" || k == "_c" {
                continue;
            }
            let existing = self.get(id).and_then(|c| c.get(k).cloned());
            match (&existing, v) {
                (Some(Prop::Control(cid)), Value::Map(_)) if v.get("_i").and_then(Value::as_i64) == Some(*cid) => {
                    changed.extend(self.update(*cid, v)?);
                }
                (Some(Prop::Map(_)), Value::Map(_)) if v.get("_c").is_none() => {
                    // a plain map: merge entry by entry
                    let merged = self.transform(v, Some(id))?;
                    let Some(Prop::Map(dst)) = self.controls.get_mut(&id).and_then(|c| c.props.get_mut(k)) else { continue };
                    if let Prop::Map(src) = merged {
                        for (sk, sv) in src {
                            match dst.iter_mut().find(|(dk, _)| *dk == sk) {
                                Some((_, d)) => *d = sv,
                                None => dst.push((sk, sv)),
                            }
                        }
                    }
                    push_once(&mut changed, id);
                }
                _ => {
                    let p = self.transform(v, Some(id))?;
                    if existing.as_ref() != Some(&p) {
                        self.controls.get_mut(&id).ok_or("部品がありません")?.props.set(k, p)?;
                        push_once(&mut changed, id);
                    }
                }
            }
        }
        Ok(changed)
    }

    /// Applies a patch with `root` as its control 0. Returns the controls
    /// whose properties changed (to draw again), parents included when a
    /// child's `visible` changed.
    pub fn apply_patch(&mut self, root: ControlId, patch: &Value) -> Result<Vec<ControlId>, String> {
        let items = patch.as_array().ok_or("差分は配列です")?;
        if items.len() < 2 {
            return Err("差分は木の索引と操作の 2 つ以上です".into());
        }
        if !self.controls.contains_key(&root) {
            return Err(format!("部品 {root} がありません(画面側が Python と食い違っています)"));
        }
        let mut paths: HashMap<i64, Vec<Key>> = HashMap::new();
        build_paths(&items[0], Vec::new(), &mut paths)?;
        let mut changed = Vec::new();
        for op in &items[1..] {
            let parts = op.as_array().ok_or("操作は配列です")?;
            let code = parts.first().and_then(Value::as_i64).ok_or("操作の種類がありません")?;
            let path_of = |i: usize| -> Result<&Vec<Key>, String> {
                let n = parts.get(i).and_then(Value::as_i64).ok_or("索引がありません")?;
                paths.get(&n).ok_or_else(|| format!("索引 {n} が木にありません"))
            };
            match code {
                0 | 1 => {
                    let (ctrl, rel) = self.locate(root, path_of(1)?)?;
                    let key = Key::from_value(parts.get(2).ok_or("鍵がありません")?)?;
                    let value = self.transform(parts.get(3).ok_or("値がありません")?, Some(ctrl))?;
                    let target = self.prop_mut(ctrl, &rel)?;
                    match (target, &key) {
                        (Prop::Map(_), Key::Name(n)) => {
                            self.prop_mut(ctrl, &rel)?.set(n, value)?;
                        }
                        (Prop::Map(_), Key::Index(i)) => {
                            self.prop_mut(ctrl, &rel)?.set(&i.to_string(), value)?;
                        }
                        (Prop::List(l), Key::Index(i)) => {
                            if code == 0 {
                                let slot = l.get_mut(*i).ok_or_else(|| format!("位置 {i} が無い"))?;
                                *slot = value;
                            } else {
                                let at = (*i).min(l.len());
                                l.insert(at, value);
                            }
                        }
                        _ => return Err(format!("置く先が辞書でも並びでもありません: {op:?}")),
                    }
                    push_once(&mut changed, ctrl);
                    if let Key::Name(n) = &key {
                        if n == "visible" {
                            if let Some(p) = self.get(ctrl).and_then(|c| c.parent) {
                                push_once(&mut changed, p);
                            }
                        }
                    }
                }
                2 => {
                    let (ctrl, rel) = self.locate(root, path_of(1)?)?;
                    let key = Key::from_value(parts.get(2).ok_or("鍵がありません")?)?;
                    let target = self.prop_mut(ctrl, &rel)?;
                    match (target, &key) {
                        (p @ Prop::Map(_), Key::Name(n)) => {
                            p.remove(n);
                        }
                        (Prop::List(l), Key::Index(i)) => {
                            if *i < l.len() {
                                l.remove(*i);
                            }
                        }
                        _ => return Err(format!("消す先が辞書でも並びでもありません: {op:?}")),
                    }
                    push_once(&mut changed, ctrl);
                }
                3 => {
                    let (from_ctrl, from_rel) = self.locate(root, path_of(1)?)?;
                    let from_key = Key::from_value(parts.get(2).ok_or("元の鍵がありません")?)?;
                    let (to_ctrl, to_rel) = self.locate(root, path_of(3)?)?;
                    let to_key = Key::from_value(parts.get(4).ok_or("先の鍵がありません")?)?;
                    let taken = match (self.prop_mut(from_ctrl, &from_rel)?, &from_key) {
                        (p @ Prop::Map(_), Key::Name(n)) => p.remove(n),
                        (Prop::List(l), Key::Index(i)) if *i < l.len() => Some(l.remove(*i)),
                        _ => None,
                    }
                    .ok_or_else(|| format!("移す元がありません: {op:?}"))?;
                    match (self.prop_mut(to_ctrl, &to_rel)?, &to_key) {
                        (p @ Prop::Map(_), Key::Name(n)) => p.set(n, taken)?,
                        (Prop::List(l), Key::Index(i)) => {
                            let at = (*i).min(l.len());
                            l.insert(at, taken);
                        }
                        _ => return Err(format!("移す先が辞書でも並びでもありません: {op:?}")),
                    }
                    push_once(&mut changed, from_ctrl);
                    push_once(&mut changed, to_ctrl);
                }
                other => return Err(format!("知らない操作 {other}")),
            }
        }
        Ok(changed)
    }

    /// Follows a tree-index path from `root`: the control reached last, and
    /// the path that remains inside its properties
    fn locate(&self, root: ControlId, path: &[Key]) -> Result<(ControlId, Vec<Key>), String> {
        let mut ctrl = root;
        let mut rel: Vec<Key> = Vec::new();
        let mut cur: &Prop = &self.get(ctrl).ok_or("根の部品がありません")?.props;
        for k in path {
            let next = match (cur, k) {
                (Prop::Map(_), Key::Name(n)) => cur.get(n),
                (Prop::Map(_), Key::Index(i)) => cur.get(&i.to_string()),
                (Prop::List(l), Key::Index(i)) => l.get(*i),
                _ => None,
            }
            .ok_or_else(|| format!("径路 {k:?} がありません"))?;
            rel.push(k.clone());
            match next {
                Prop::Control(cid) => {
                    ctrl = *cid;
                    rel.clear();
                    cur = &self.get(ctrl).ok_or_else(|| format!("部品 {cid} がありません"))?.props;
                }
                other => cur = other,
            }
        }
        Ok((ctrl, rel))
    }

    fn prop_mut(&mut self, ctrl: ControlId, rel: &[Key]) -> Result<&mut Prop, String> {
        let mut cur: &mut Prop = &mut self.controls.get_mut(&ctrl).ok_or("部品がありません")?.props;
        for k in rel {
            cur = match (cur, k) {
                (p @ Prop::Map(_), Key::Name(n)) => p.get_mut(n),
                (p @ Prop::Map(_), Key::Index(i)) => p.get_mut(&i.to_string()),
                (Prop::List(l), Key::Index(i)) => l.get_mut(*i),
                _ => None,
            }
            .ok_or_else(|| format!("径路 {k:?} がありません"))?;
        }
        Ok(cur)
    }
}

fn push_once(v: &mut Vec<ControlId>, id: ControlId) {
    if !v.contains(&id) {
        v.push(id);
    }
}

/// `[index, {key: child, …}]` → the path of every index
fn build_paths(node: &Value, path: Vec<Key>, out: &mut HashMap<i64, Vec<Key>>) -> Result<(), String> {
    let items = node.as_array().ok_or("木の索引の節は配列です")?;
    let n = items.first().and_then(Value::as_i64).ok_or("木の索引の番号がありません")?;
    out.insert(n, path.clone());
    if let Some(Value::Map(children)) = items.get(1) {
        for (k, child) in children {
            let mut p = path.clone();
            p.push(Key::from_value(k)?);
            build_paths(child, p, out)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(x: &str) -> Value {
        Value::str(x)
    }

    fn page() -> Value {
        // the registration response's page_patch (2026-10-17, trimmed)
        Value::map(vec![
            ("_c", s("Page")),
            ("_i", Value::Int(1)),
            ("width", Value::Int(800)),
            ("window", Value::map(vec![("_c", s("Window")), ("_i", Value::Int(0)), ("width", Value::Int(800))])),
            (
                "views",
                Value::Array(vec![Value::map(vec![
                    ("_c", s("View")),
                    ("_i", Value::Int(3)),
                    ("route", s("/")),
                    ("_internals", Value::map(vec![("host_expanded", Value::Bool(true))])),
                ])]),
            ),
        ])
    }

    fn patch_add_controls() -> Value {
        // [[0, {"views": [1, {0: [2]}]}], [0, 0, "title", "fake client test"], [0, 2, "controls", [...]]]
        Value::Array(vec![
            Value::Array(vec![
                Value::Int(0),
                Value::Map(vec![(
                    s("views"),
                    Value::Array(vec![Value::Int(1), Value::Map(vec![(Value::Int(0), Value::Array(vec![Value::Int(2)]))])]),
                )]),
            ]),
            Value::Array(vec![Value::Int(0), Value::Int(0), s("title"), s("fake client test")]),
            Value::Array(vec![
                Value::Int(0),
                Value::Int(2),
                s("controls"),
                Value::Array(vec![Value::map(vec![
                    ("_internals", Value::map(vec![("host_expanded", Value::Bool(true))])),
                    ("_i", Value::Int(12)),
                    ("_c", s("Column")),
                    (
                        "controls",
                        Value::Array(vec![
                            Value::map(vec![("value", s("押す前")), ("_i", Value::Int(9)), ("_c", s("Text"))]),
                            Value::map(vec![
                                ("content", s("押す")),
                                ("on_click", Value::Bool(true)),
                                ("_i", Value::Int(10)),
                                ("_c", s("Button")),
                            ]),
                            Value::map(vec![("label", s("名前")), ("_i", Value::Int(11)), ("_c", s("TextField"))]),
                        ]),
                    ),
                ])]),
            ]),
        ])
    }

    fn patch_after_click() -> Value {
        // [[0, {"views": [1, {0: [2, {"controls": [3, {0: [4, {"controls": [5, {0: [6]}]}]}]}]}]}], [0, 6, "value", "押した"]]
        let idx = Value::Array(vec![
            Value::Int(0),
            Value::Map(vec![(
                s("views"),
                Value::Array(vec![
                    Value::Int(1),
                    Value::Map(vec![(
                        Value::Int(0),
                        Value::Array(vec![
                            Value::Int(2),
                            Value::Map(vec![(
                                s("controls"),
                                Value::Array(vec![
                                    Value::Int(3),
                                    Value::Map(vec![(
                                        Value::Int(0),
                                        Value::Array(vec![
                                            Value::Int(4),
                                            Value::Map(vec![(
                                                s("controls"),
                                                Value::Array(vec![
                                                    Value::Int(5),
                                                    Value::Map(vec![(Value::Int(0), Value::Array(vec![Value::Int(6)]))]),
                                                ]),
                                            )]),
                                        ]),
                                    )]),
                                ]),
                            )]),
                        ]),
                    )]),
                ]),
            )]),
        ]);
        Value::Array(vec![idx, Value::Array(vec![Value::Int(0), Value::Int(6), s("value"), s("押した")])])
    }

    #[test]
    fn the_registration_page_becomes_a_tree() {
        let mut t = Tree::new();
        let root = t.set_root(&page()).unwrap();
        assert_eq!(root, 1);
        assert_eq!(t.get(1).unwrap().ctype, "Page");
        assert_eq!(t.get(0).unwrap().ctype, "Window");
        assert_eq!(t.get(0).unwrap().parent, Some(1));
        assert_eq!(t.children(1, "views"), vec![3]);
        assert_eq!(t.get(3).unwrap().get_str("route"), Some("/"));
    }

    #[test]
    fn the_real_patches_add_controls_and_change_a_value() {
        let mut t = Tree::new();
        t.set_root(&page()).unwrap();
        let changed = t.apply_patch(1, &patch_add_controls()).unwrap();
        assert_eq!(changed, vec![1, 3]);
        assert_eq!(t.get(1).unwrap().get_str("title"), Some("fake client test"));
        assert_eq!(t.children(3, "controls"), vec![12]);
        assert_eq!(t.children(12, "controls"), vec![9, 10, 11]);
        assert_eq!(t.get(12).unwrap().parent, Some(3));
        assert_eq!(t.get(9).unwrap().parent, Some(12));
        assert!(t.get(10).unwrap().has_event("click"));
        assert!(!t.get(10).unwrap().has_event("hover"));
        assert_eq!(t.get(9).unwrap().get_str("value"), Some("押す前"));

        let changed = t.apply_patch(1, &patch_after_click()).unwrap();
        assert_eq!(changed, vec![9]);
        assert_eq!(t.get(9).unwrap().get_str("value"), Some("押した"));
    }

    #[test]
    fn add_remove_and_move_work_on_lists() {
        let mut t = Tree::new();
        t.set_root(&page()).unwrap();
        t.apply_patch(1, &patch_add_controls()).unwrap();
        // index 0 = Page, 1 = views, 2 = View(3), 3 = controls, 4 = Column(12), 5 = its controls
        let idx = || {
            Value::Array(vec![
                Value::Int(0),
                Value::Map(vec![(
                    s("views"),
                    Value::Array(vec![
                        Value::Int(1),
                        Value::Map(vec![(
                            Value::Int(0),
                            Value::Array(vec![
                                Value::Int(2),
                                Value::Map(vec![(
                                    s("controls"),
                                    Value::Array(vec![
                                        Value::Int(3),
                                        Value::Map(vec![(
                                            Value::Int(0),
                                            Value::Array(vec![Value::Int(4), Value::Map(vec![(s("controls"), Value::Array(vec![Value::Int(5)]))])]),
                                        )]),
                                    ]),
                                )]),
                            ]),
                        )]),
                    ]),
                )]),
            ])
        };
        // add a Text at the end of the Column
        let add = Value::Array(vec![
            idx(),
            Value::Array(vec![
                Value::Int(1),
                Value::Int(5),
                Value::Int(3),
                Value::map(vec![("_c", s("Text")), ("_i", Value::Int(20)), ("value", s("四"))]),
            ]),
        ]);
        assert_eq!(t.apply_patch(1, &add).unwrap(), vec![12]);
        assert_eq!(t.children(12, "controls"), vec![9, 10, 11, 20]);
        // move it to the front, then remove the button
        let mv = Value::Array(vec![idx(), Value::Array(vec![Value::Int(3), Value::Int(5), Value::Int(3), Value::Int(5), Value::Int(0)])]);
        t.apply_patch(1, &mv).unwrap();
        assert_eq!(t.children(12, "controls"), vec![20, 9, 10, 11]);
        let rm = Value::Array(vec![idx(), Value::Array(vec![Value::Int(2), Value::Int(5), Value::Int(2)])]);
        t.apply_patch(1, &rm).unwrap();
        assert_eq!(t.children(12, "controls"), vec![20, 9, 11]);
        // hiding a child names its parent as changed too
        let hide_idx = Value::Array(vec![
            Value::Int(0),
            Value::Map(vec![(
                s("views"),
                Value::Array(vec![
                    Value::Int(1),
                    Value::Map(vec![(
                        Value::Int(0),
                        Value::Array(vec![
                            Value::Int(2),
                            Value::Map(vec![(
                                s("controls"),
                                Value::Array(vec![
                                    Value::Int(3),
                                    Value::Map(vec![(
                                        Value::Int(0),
                                        Value::Array(vec![
                                            Value::Int(4),
                                            Value::Map(vec![(
                                                s("controls"),
                                                Value::Array(vec![Value::Int(5), Value::Map(vec![(Value::Int(1), Value::Array(vec![Value::Int(6)]))])]),
                                            )]),
                                        ]),
                                    )]),
                                ]),
                            )]),
                        ]),
                    )]),
                ]),
            )]),
        ]);
        let hide = Value::Array(vec![hide_idx, Value::Array(vec![Value::Int(0), Value::Int(6), s("visible"), Value::Bool(false)])]);
        assert_eq!(t.apply_patch(1, &hide).unwrap(), vec![9, 12]);
        assert_eq!(t.children(12, "controls"), vec![20, 11]);
    }

    #[test]
    fn a_component_is_drawn_through_its_body() {
        let mut t = Tree::new();
        let v = Value::map(vec![
            ("_c", s("Page")),
            ("_i", Value::Int(1)),
            (
                "controls",
                Value::Array(vec![Value::map(vec![
                    ("_c", s("C")),
                    ("_i", Value::Int(2)),
                    ("_b", Value::map(vec![("_c", s("Text")), ("_i", Value::Int(3)), ("value", s("x"))])),
                ])]),
            ),
        ]);
        t.set_root(&v).unwrap();
        assert_eq!(t.children(1, "controls"), vec![3]);
    }

    #[test]
    fn update_merges_a_window_and_replaces_a_scalar() {
        let mut t = Tree::new();
        t.set_root(&page()).unwrap();
        let changed = t
            .update(
                1,
                &Value::map(vec![
                    ("width", Value::Int(1000)),
                    ("window", Value::map(vec![("_c", s("Window")), ("_i", Value::Int(0)), ("width", Value::Int(1000)), ("top", Value::Int(5))])),
                ]),
            )
            .unwrap();
        assert_eq!(changed, vec![1, 0]);
        assert_eq!(t.get(1).unwrap().get_i64("width"), Some(1000));
        assert_eq!(t.get(0).unwrap().get_i64("width"), Some(1000));
        assert_eq!(t.get(0).unwrap().get_i64("top"), Some(5));
    }
}
