use std::collections::VecDeque;

pub struct Lookahead<I: Iterator> {
    iter: I,
    buffer: VecDeque<<I as Iterator>::Item>,
}

impl<I: Iterator> Lookahead<I> {
    pub fn new(mut iter: I, n: usize) -> Self {
        let mut buffer = VecDeque::with_capacity(n+1);

        {
            let it = &mut iter;
            for x in it.take(n+1) {
                buffer.push_back(x);
            }
        }

        Self {
            iter: iter,
            buffer: buffer,
        }
    }

    pub fn ahead(&self, i: usize) -> Option<&<I as Iterator>::Item> {
        self.buffer.get(i)
    }

    pub fn into_inner(self) -> I {
        self.iter
    }
}

impl<I: Iterator> Iterator for Lookahead<I> {
    type Item = <I as Iterator>::Item;

    fn next(&mut self) -> Option<Self::Item> {
        let ret = self.buffer.pop_front();

        if let Some(x) = self.iter.next() {
            self.buffer.push_back(x);
        }

        ret
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn lookahead() {
        let v = vec![0, 1, 2, 3];
        let mut v_it = v.into_iter();
        let mut it = Lookahead::new(&mut v_it, 1);

        assert_eq!(it.ahead(0), Some(&0));
        assert_eq!(it.ahead(1), Some(&1));
        assert_eq!(it.next(), Some(0));
        assert_eq!(it.ahead(0), Some(&1));
        assert_eq!(it.ahead(1), Some(&2));
        assert_eq!(it.next(), Some(1));
        assert_eq!(it.ahead(0), Some(&2));
        assert_eq!(it.ahead(1), Some(&3));
        assert_eq!(it.next(), Some(2));
        assert_eq!(it.ahead(0), Some(&3));
        assert_eq!(it.ahead(1), None);
        assert_eq!(it.next(), Some(3));
        assert_eq!(it.ahead(0), None);
        assert_eq!(it.next(), None);
    }
}
