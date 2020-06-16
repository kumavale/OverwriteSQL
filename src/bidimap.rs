use std::collections::HashMap;
use std::hash::Hash;
use std::rc::Rc;
use std::ops::Deref;

pub struct BidiMap<A, B> {
    key_value: HashMap<Rc<A>, Rc<B>>,
    value_key: HashMap<Rc<B>, Rc<A>>,
}

impl<A, B> BidiMap<A, B>
where
    A: Eq + Hash,
    B: Eq + Hash,
{
    pub fn new() -> Self {
        Self {
            key_value: HashMap::new(),
            value_key: HashMap::new(),
        }
    }

    pub fn entry_or_insert(&mut self, a: A, b: B) {
        if !self.key_value.contains_key(&a) {
            let a = Rc::new(a);
            let b = Rc::new(b);
            self.key_value.insert(a.clone(), b.clone());
            self.value_key.insert(b, a);
        }
    }

    pub fn get(&self, key: &A) -> Option<&B> {
        self.key_value.get(key).map(Deref::deref)
    }

    pub fn get_reverse(&self, value: &B) -> Option<&A> {
        self.value_key.get(value).map(Deref::deref)
    }
}

