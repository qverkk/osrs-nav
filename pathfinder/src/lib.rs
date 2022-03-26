use std::collections::{BinaryHeap, VecDeque};

use serde::{Deserialize, Serialize};

use model::{Coordinate, Edge, NavGrid};
use model::constants::*;
use model::definitions::{EdgeDefinition, GameState};
use model::util::RegionCache;

#[derive(Clone, Copy)]
struct DijkstraCacheState<'a> {
    cost: u32,
    prev: u32,
    edge: Option<&'a Edge>
}

#[derive(Eq, PartialEq, Clone, Copy, Debug)]
struct DijkstraQueueState {
    cost: u32,
    index: u32,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum Step {
    Edge(EdgeDefinition),
    Step(Coordinate),
}

impl Ord for DijkstraQueueState {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.cost.cmp(&self.cost).then_with(|| self.index.cmp(&other.index))
    }
}

impl PartialOrd for DijkstraQueueState {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

pub fn dijkstra(nav_grid: &NavGrid, start: &Coordinate, end: &Coordinate, game_state: &GameState) -> Option<Vec<Step>> {
    let start_index = start.index();
    let end_index = end.index();
    if nav_grid.vertices[start_index as usize].get_group() != nav_grid.vertices[end_index as usize].get_group() {
        return None;
    }
    let mut queue = BinaryHeap::new();
    let mut cache = RegionCache::new(DijkstraCacheState { cost: u32::MAX, prev: u32::MAX, edge: None });
    cache.get_mut(start_index).cost = 0;
    queue.push(DijkstraQueueState { cost: 0, index: start_index});
    for teleport in &nav_grid.teleports {
        if teleport.requirements.iter().all(|req| req.is_met(game_state)) {
            let dest = cache.get_mut(teleport.destination.index());
            if teleport.cost < dest.cost {
                println!("{:?}", teleport);
                dest.cost = teleport.cost;
                //dest.prev = start_index;
                dest.edge = Some(teleport);
                queue.push(DijkstraQueueState { cost: dest.cost, index: teleport.destination.index() });
            }
        }
    }
    while let Some(DijkstraQueueState { cost, mut index }) = queue.pop() {
        if index == end_index {
            let mut path = Vec::new();
            while index != u32::MAX {
                let state = cache.get_mut(index);
                if let Some(edge) = state.edge {
                    path.push(Step::Edge(edge.definition.clone()));
                } else {
                    path.push(Step::Step(Coordinate::from_index(index)));
                }
                index = state.prev;
            }
            path.reverse();
            return Some(path);
        }
        let v = &nav_grid.vertices[index as usize];
        for (flag, dx, dy) in &DIRECTIONS {
            if (v.flags & flag) != 0 {
                let adj_index = index + (WIDTH * *dy as u32) + *dx as u32;
                let adj = cache.get_mut(adj_index);
                if cost + 1 < adj.cost {
                    adj.cost = cost + 1;
                    adj.prev = index;
                    adj.edge = None;
                    queue.push(DijkstraQueueState { cost: adj.cost, index: adj_index });
                }
            }
        }
        if v.has_extra_edges() {
            for edge in nav_grid.edges.get_vec(&index).unwrap() {
                if edge.requirements.iter().all(|req| req.is_met(game_state)) {
                    let adj = cache.get_mut(edge.destination.index());
                    if cost + edge.cost < adj.cost {
                        adj.cost = cost + edge.cost;
                        adj.prev = index;
                        adj.edge = Some(edge);
                        queue.push(DijkstraQueueState { cost: adj.cost, index: edge.destination.index() });
                    }
                }
            }
        }
    }
    None
}

pub fn flood<F>(nav_grid: &NavGrid, start: &Coordinate, mut visit_vertex: F) where F: FnMut(u32) -> bool {
    let mut queue = VecDeque::new();
    let mut cache = RegionCache::new(false);
    queue.push_back(start.index());
    *cache.get_mut(start.index()) = true;
    while let Some(index) = queue.pop_front() {
        let v = &nav_grid.vertices[index as usize];
        if !visit_vertex(index) {
            continue;
        }
        for (flag, dx, dy) in &DIRECTIONS {
            if (v.flags & flag) != 0 {
                let adj_index = index + (WIDTH * *dy as u32) + *dx as u32;
                let visited = cache.get_mut(adj_index);
                if !*visited {
                    queue.push_back(adj_index);
                    *visited = true;
                }
            }
        }
        if v.has_extra_edges() {
            for edge in nav_grid.edges.get_vec(&index).unwrap() {
                let visited = cache.get_mut(edge.destination.index());
                if !*visited {
                    queue.push_back(edge.destination.index());
                    *visited = true;
                }
            }
        }
    }
}
