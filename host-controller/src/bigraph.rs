use petgraph::graphmap::{Neighbors, UnGraphMap};
use std::{hash::Hash, iter::FlatMap};

/// An undirected bipartite graph.
pub struct BiGraph<L, R> {
    graph: UnGraphMap<Either<L, R>, ()>,
}

impl<L, R> BiGraph<L, R>
where
    L: Copy + Ord + Hash,
    R: Copy + Ord + Hash,
{
    pub fn new() -> Self {
        Self {
            graph: UnGraphMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.graph.clear()
    }

    pub fn add_left(&mut self, left: L) {
        self.graph.add_node(Either::Left(left));
    }

    pub fn add_right(&mut self, right: R) {
        self.graph.add_node(Either::Right(right));
    }

    pub fn remove_left(&mut self, left: L) -> bool {
        self.graph.remove_node(Either::Left(left))
    }

    pub fn remove_right(&mut self, right: R) -> bool {
        self.graph.remove_node(Either::Right(right))
    }

    pub fn contains_left(&self, left: L) -> bool {
        self.graph.contains_node(Either::Left(left))
    }

    pub fn contains_right(&self, right: R) -> bool {
        self.graph.contains_node(Either::Right(right))
    }

    pub fn add_edge(&mut self, left: L, right: R) -> bool {
        self.graph
            .add_edge(Either::Left(left), Either::Right(right), ())
            .is_none()
    }

    pub fn remove_edge(&mut self, left: L, right: R) -> bool {
        self.graph
            .remove_edge(Either::Left(left), Either::Right(right))
            .is_some()
    }

    pub fn contains_edge(&self, left: L, right: R) -> bool {
        self.graph
            .contains_edge(Either::Left(left), Either::Right(right))
    }

    pub fn neighbors_of_left(&self, left: L) -> NeighborsOfLeft<L, R> {
        NeighborsOfLeft {
            inner: self
                .graph
                .neighbors(Either::Left(left))
                .flat_map(Either::right),
        }
    }

    pub fn neighbors_of_right(&self, right: R) -> NeighborsOfRight<L, R> {
        NeighborsOfRight {
            inner: self
                .graph
                .neighbors(Either::Right(right))
                .flat_map(Either::left),
        }
    }
}

pub struct NeighborsOfLeft<'a, L, R> {
    inner: FlatMap<Neighbors<'a, Either<L, R>>, Option<R>, fn(Either<L, R>) -> Option<R>>,
}

impl<'a, L, R> Iterator for NeighborsOfLeft<'a, L, R>
where
    L: Copy + Ord + Hash,
    R: Copy + Ord + Hash,
{
    type Item = R;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

pub struct NeighborsOfRight<'a, L, R> {
    inner: FlatMap<Neighbors<'a, Either<L, R>>, Option<L>, fn(Either<L, R>) -> Option<L>>,
}

impl<'a, L, R> Iterator for NeighborsOfRight<'a, L, R>
where
    L: Copy + Ord + Hash,
    R: Copy + Ord + Hash,
{
    type Item = L;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Either<L, R> {
    Left(L),
    Right(R),
}

impl<L, R> Either<L, R> {
    fn left(self) -> Option<L> {
        match self {
            Self::Left(left) => Some(left),
            _ => None,
        }
    }

    fn right(self) -> Option<R> {
        match self {
            Self::Right(right) => Some(right),
            _ => None,
        }
    }
}
