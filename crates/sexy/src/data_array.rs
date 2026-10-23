//! `DataArray<T>`: the original's object pool. Items are referred to by IDs
//! that become invalid once the item is freed (a generational slot map), so a
//! stale ID can't reach a newer item in the same slot.

use std::marker::PhantomData;

pub struct Id<T> {
    index: u32,
    generation: u32,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Id<T> {}
impl<T> PartialEq for Id<T> {
    fn eq(&self, o: &Self) -> bool {
        (self.index, self.generation) == (o.index, o.generation)
    }
}
impl<T> Eq for Id<T> {}
impl<T> std::hash::Hash for Id<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (self.index, self.generation).hash(state);
    }
}
impl<T> std::fmt::Debug for Id<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Id({}v{})", self.index, self.generation)
    }
}

struct Slot<T> {
    generation: u32,
    item: Option<T>,
}

pub struct DataArray<T> {
    slots: Vec<Slot<T>>,
    free: Vec<u32>,
}

impl<T> Default for DataArray<T> {
    fn default() -> Self {
        Self { slots: Vec::new(), free: Vec::new() }
    }
}

impl<T> DataArray<T> {
    /// How many items are allocated (`mSize`).
    pub fn len(&self) -> usize {
        self.slots.iter().filter(|s| s.item.is_some()).count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn alloc(&mut self, item: T) -> Id<T> {
        let index = match self.free.pop() {
            Some(i) => i,
            None => {
                self.slots.push(Slot { generation: 0, item: None });
                (self.slots.len() - 1) as u32
            }
        };
        let slot = &mut self.slots[index as usize];
        slot.item = Some(item);
        Id { index, generation: slot.generation, _marker: PhantomData }
    }

    fn slot(&self, id: Id<T>) -> Option<&Slot<T>> {
        self.slots.get(id.index as usize).filter(|s| s.generation == id.generation)
    }

    pub fn get(&self, id: Id<T>) -> Option<&T> {
        self.slot(id).and_then(|s| s.item.as_ref())
    }

    pub fn get_mut(&mut self, id: Id<T>) -> Option<&mut T> {
        self.slots
            .get_mut(id.index as usize)
            .filter(|s| s.generation == id.generation)
            .and_then(|s| s.item.as_mut())
    }

    pub fn free(&mut self, id: Id<T>) -> Option<T> {
        let slot = self.slots.get_mut(id.index as usize).filter(|s| s.generation == id.generation)?;
        let item = slot.item.take()?;
        slot.generation = slot.generation.wrapping_add(1);
        self.free.push(id.index);
        Some(item)
    }

    pub fn clear(&mut self) {
        let ids: Vec<Id<T>> = self.iter().map(|(id, _)| id).collect();
        for id in ids {
            self.free(id);
        }
    }

    /// Live items in slot order.
    pub fn iter(&self) -> impl Iterator<Item = (Id<T>, &T)> {
        self.slots.iter().enumerate().filter_map(|(i, s)| {
            s.item.as_ref().map(|item| (Id { index: i as u32, generation: s.generation, _marker: PhantomData }, item))
        })
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Id<T>, &mut T)> {
        self.slots.iter_mut().enumerate().filter_map(|(i, s)| {
            let generation = s.generation;
            s.item.as_mut().map(|item| (Id { index: i as u32, generation, _marker: PhantomData }, item))
        })
    }

    /// Frees every item for which `keep` returns false.
    pub fn retain(&mut self, mut keep: impl FnMut(&T) -> bool) {
        let dead: Vec<Id<T>> = self.iter().filter(|(_, item)| !keep(item)).map(|(id, _)| id).collect();
        for id in dead {
            self.free(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_ids_miss_reused_slots() {
        let mut a = DataArray::default();
        let x = a.alloc("x");
        assert_eq!(a.free(x), Some("x"));
        let y = a.alloc("y");
        assert_eq!(a.get(x), None);
        assert_eq!(a.get(y), Some(&"y"));
        assert_ne!(x, y);
    }

    #[test]
    fn retain_frees_matching_items() {
        let mut a = DataArray::default();
        let ids: Vec<_> = (0..4).map(|i| a.alloc(i)).collect();
        a.retain(|&i| i % 2 == 0);
        assert_eq!(a.iter().map(|(_, &i)| i).collect::<Vec<_>>(), [0, 2]);
        assert!(a.get(ids[1]).is_none());
    }
}
