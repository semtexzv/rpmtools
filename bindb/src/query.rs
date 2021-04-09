use crate::{Database, Table};


pub struct Query<T, F> {
    iter: crate::Iter<T>,
    filter: F,
}

impl Database {
    pub fn query<T, F>(&self, filter: F) -> Query<T, F>
        where T: Table, F: FnMut(&T) -> bool
    {
        Query {
            iter: self.scan(),
            filter,
        }
    }
}

impl<T, F> Iterator for Query<T, F>
    where T: Table, F: FnMut(&T) -> bool
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.iter.next() {
                Some(i) if (self.filter)(&i) => return Some(i),
                Some(_) => {}
                None => return None
            }
        }
    }
}
